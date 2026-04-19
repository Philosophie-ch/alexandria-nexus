//! Render process — orchestrates bibitem resolution, data fetching, and context building.
//!
//! Defines traits for I/O operations and coordinates between data fetching
//! (via traits) and pure logic functions (from `crate::logic::render`).
//! No AppState, no PgPool, no sqlx, no SQL — only abstract contracts.

use std::collections::{HashMap, HashSet};
use std::future::Future;

use hexforge::HexforgeError;

use crate::domain::junctions::BibitemAuthorsRow;
use crate::domain::{Author, AuthorRole, BibItem};
use crate::logic::render::{
    RenderContext, author_sort_key, extract_role_authors, render_bibliography,
};

// =============================================================================
// Traits — contracts for I/O operations that adapters implement
// =============================================================================

/// Contract for resolving bibitems by IDs or bibkeys.
pub trait BibitemResolver: Send + Sync {
    fn find_by_ids(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibItem>, HexforgeError>> + Send;

    fn find_by_bibkeys(
        &self,
        bibkeys: &[String],
    ) -> impl Future<Output = Result<Vec<BibItem>, HexforgeError>> + Send;
}

/// Contract for batch-fetching related entity names for rendering.
///
/// Each method returns a map of entity ID → display name (unicode).
/// The crossref method returns ID → bibkey instead.
pub trait RenderEntityFetcher: Send + Sync {
    fn fetch_journal_names(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<HashMap<i64, String>, HexforgeError>> + Send;

    fn fetch_publisher_names(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<HashMap<i64, String>, HexforgeError>> + Send;

    fn fetch_institution_names(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<HashMap<i64, String>, HexforgeError>> + Send;

    fn fetch_school_names(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<HashMap<i64, String>, HexforgeError>> + Send;

    fn fetch_series_names(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<HashMap<i64, String>, HexforgeError>> + Send;

    fn fetch_crossref_bibkeys(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<HashMap<i64, String>, HexforgeError>> + Send;
}

/// Contract for fetching transitive further-ref dep IDs for a set of source IDs.
pub trait TransitiveDepsResolver: Send + Sync {
    fn fetch_further_ref_ids(
        &self,
        source_ids: &[i64],
    ) -> impl Future<Output = Result<Vec<i64>, HexforgeError>> + Send;
}

/// Contract for batch-fetching author junction data and author entities.
pub trait RenderAuthorFetcher: Send + Sync {
    fn fetch_bibitem_authors(
        &self,
        bibitem_ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemAuthorsRow>, HexforgeError>> + Send;

    fn fetch_authors_by_ids(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<HashMap<i64, Author>, HexforgeError>> + Send;
}

// =============================================================================
// Render request — what the process layer receives
// =============================================================================

/// Selection criteria for bibitems to render.
pub enum RenderSelection {
    ByIds(Vec<i64>),
    ByBibkeys(Vec<String>),
}

/// Rendered output from the render pipeline.
pub struct RenderResponse {
    pub main_html: String,
    pub further_refs_html: Option<String>,
}

/// Maximum number of items per render request.
pub const MAX_RENDER_ITEMS: usize = 1000;

// =============================================================================
// Render errors — typed errors for the handler to map to HTTP
// =============================================================================

/// Errors from the render pipeline.
///
/// The handler maps these to HTTP status codes and response bodies.
pub enum RenderPipelineError {
    TooManyItems { requested: usize, max: usize },
    MissingIds(Vec<i64>),
    MissingBibkeys(Vec<String>),
    Internal(HexforgeError),
}

impl From<HexforgeError> for RenderPipelineError {
    fn from(e: HexforgeError) -> Self {
        RenderPipelineError::Internal(e)
    }
}

// =============================================================================
// Orchestration
// =============================================================================

/// Full render pipeline: validate → resolve → fetch → build contexts → sort → render.
///
/// Returns a `RenderResponse` or a typed error for the handler to map to HTTP.
pub async fn render_pipeline(
    resolver: &impl BibitemResolver,
    entity_fetcher: &impl RenderEntityFetcher,
    author_fetcher: &impl RenderAuthorFetcher,
    deps_resolver: &impl TransitiveDepsResolver,
    selection: RenderSelection,
    include_further_refs: bool,
) -> Result<RenderResponse, RenderPipelineError> {
    // Validate count
    let count = match &selection {
        RenderSelection::ByIds(ids) => ids.len(),
        RenderSelection::ByBibkeys(keys) => keys.len(),
    };
    if count > MAX_RENDER_ITEMS {
        return Err(RenderPipelineError::TooManyItems {
            requested: count,
            max: MAX_RENDER_ITEMS,
        });
    }
    if count == 0 {
        return Ok(RenderResponse {
            main_html: String::new(),
            further_refs_html: None,
        });
    }

    // Resolve bibitems
    let bibitems = match selection {
        RenderSelection::ByIds(ids) => {
            let found = resolver.find_by_ids(&ids).await?;
            let found_ids: HashSet<i64> = found.iter().map(|b| b.id).collect();
            let missing: Vec<i64> = ids
                .iter()
                .filter(|id| !found_ids.contains(id))
                .copied()
                .collect();
            if !missing.is_empty() {
                return Err(RenderPipelineError::MissingIds(missing));
            }
            found
        }
        RenderSelection::ByBibkeys(bibkeys) => {
            let found = resolver.find_by_bibkeys(&bibkeys).await?;
            let found_keys: HashSet<&str> = found.iter().map(|b| b.bibkey.as_str()).collect();
            let missing: Vec<String> = bibkeys
                .iter()
                .filter(|k| !found_keys.contains(k.as_str()))
                .cloned()
                .collect();
            if !missing.is_empty() {
                return Err(RenderPipelineError::MissingBibkeys(missing));
            }
            found
        }
    };

    let main_ids: Vec<i64> = bibitems.iter().map(|b| b.id).collect();
    let main_html = fetch_and_render(entity_fetcher, author_fetcher, bibitems).await?;

    let further_refs_html = if include_further_refs {
        let main_id_set: HashSet<i64> = main_ids.iter().copied().collect();
        let dep_ids = deps_resolver.fetch_further_ref_ids(&main_ids).await?;
        let novel_dep_ids: Vec<i64> = dep_ids
            .into_iter()
            .filter(|id| !main_id_set.contains(id))
            .collect();
        if novel_dep_ids.is_empty() {
            Some(String::new())
        } else {
            let dep_bibitems = resolver.find_by_ids(&novel_dep_ids).await?;
            Some(fetch_and_render(entity_fetcher, author_fetcher, dep_bibitems).await?)
        }
    } else {
        None
    };

    Ok(RenderResponse {
        main_html,
        further_refs_html,
    })
}

// =============================================================================
// Internal helpers
// =============================================================================

/// Fetch all related data, build RenderContexts, sort, and render to HTML.
async fn fetch_and_render(
    entity_fetcher: &impl RenderEntityFetcher,
    author_fetcher: &impl RenderAuthorFetcher,
    bibitems: Vec<BibItem>,
) -> Result<String, HexforgeError> {
    let bibitem_ids: Vec<i64> = bibitems.iter().map(|b| b.id).collect();

    // Collect unique FK IDs
    let mut journal_ids = Vec::new();
    let mut publisher_ids = Vec::new();
    let mut institution_ids = Vec::new();
    let mut school_ids = Vec::new();
    let mut series_ids = Vec::new();
    let mut crossref_ids = Vec::new();

    for bib in &bibitems {
        if let Some(id) = bib.journal_id {
            journal_ids.push(id);
        }
        if let Some(id) = bib.publisher_id {
            publisher_ids.push(id);
        }
        if let Some(id) = bib.institution_id {
            institution_ids.push(id);
        }
        if let Some(id) = bib.school_id {
            school_ids.push(id);
        }
        if let Some(id) = bib.series_id {
            series_ids.push(id);
        }
        if let Some(id) = bib.crossref_id {
            crossref_ids.push(id);
        }
    }

    // Batch-fetch entity names and author junction data concurrently (all independent).
    let (
        journals_map,
        publishers_map,
        institutions_map,
        schools_map,
        series_map,
        crossrefs_map,
        author_rows,
    ) = tokio::try_join!(
        entity_fetcher.fetch_journal_names(&journal_ids),
        entity_fetcher.fetch_publisher_names(&publisher_ids),
        entity_fetcher.fetch_institution_names(&institution_ids),
        entity_fetcher.fetch_school_names(&school_ids),
        entity_fetcher.fetch_series_names(&series_ids),
        entity_fetcher.fetch_crossref_bibkeys(&crossref_ids),
        author_fetcher.fetch_bibitem_authors(&bibitem_ids),
    )?;

    // Batch-fetch author entities
    let all_author_ids: Vec<i64> = author_rows
        .iter()
        .map(|r| r.author_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let authors_map = author_fetcher.fetch_authors_by_ids(&all_author_ids).await?;

    // Group junction data by bibitem_id
    let mut authors_by_bibitem: HashMap<i64, Vec<&BibitemAuthorsRow>> = HashMap::new();
    for row in &author_rows {
        authors_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    // Build RenderContext for each bibitem
    let mut items_with_ctx: Vec<(BibItem, RenderContext)> = bibitems
        .into_iter()
        .map(|bib| {
            let bib_authors = authors_by_bibitem.get(&bib.id);

            let authors = extract_role_authors(bib_authors, AuthorRole::Author, &authors_map);
            let editors = extract_role_authors(bib_authors, AuthorRole::Editor, &authors_map);
            let guesteditors =
                extract_role_authors(bib_authors, AuthorRole::Guesteditor, &authors_map);

            let ctx = RenderContext {
                authors,
                editors,
                guesteditors,
                journal_name: bib.journal_id.and_then(|id| journals_map.get(&id).cloned()),
                publisher_name: bib
                    .publisher_id
                    .and_then(|id| publishers_map.get(&id).cloned()),
                series_name: bib.series_id.and_then(|id| series_map.get(&id).cloned()),
                institution_name: bib
                    .institution_id
                    .and_then(|id| institutions_map.get(&id).cloned()),
                school_name: bib.school_id.and_then(|id| schools_map.get(&id).cloned()),
                crossref_bibkey: bib
                    .crossref_id
                    .and_then(|id| crossrefs_map.get(&id).cloned()),
                suppress_author: false,
            };
            (bib, ctx)
        })
        .collect();

    // Sort by author family name -> year -> bibkey
    items_with_ctx.sort_by(|(a, ctx_a), (b, ctx_b)| {
        let key_a = author_sort_key(&ctx_a.authors);
        let key_b = author_sort_key(&ctx_b.authors);
        key_a
            .cmp(&key_b)
            .then_with(|| a.date_year.cmp(&b.date_year))
            .then_with(|| a.bibkey.cmp(&b.bibkey))
    });

    Ok(render_bibliography(&items_with_ctx))
}
