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
// Render request resolution result
// =============================================================================

/// Result of resolving a render request — either resolved bibitems or an error
/// indicating which items were not found.
pub enum ResolveResult {
    Ok(Vec<BibItem>),
    MissingIds(Vec<i64>),
    MissingBibkeys(Vec<String>),
}

// =============================================================================
// Orchestration
// =============================================================================

/// Resolve bibitems by IDs, returning resolved items or missing IDs.
pub async fn resolve_by_ids(
    resolver: &impl BibitemResolver,
    ids: &[i64],
) -> Result<ResolveResult, HexforgeError> {
    let found = resolver.find_by_ids(ids).await?;
    let found_ids: HashSet<i64> = found.iter().map(|b| b.id).collect();
    let missing: Vec<i64> = ids
        .iter()
        .filter(|id| !found_ids.contains(id))
        .copied()
        .collect();
    if missing.is_empty() {
        Ok(ResolveResult::Ok(found))
    } else {
        Ok(ResolveResult::MissingIds(missing))
    }
}

/// Resolve bibitems by bibkeys, returning resolved items or missing bibkeys.
pub async fn resolve_by_bibkeys(
    resolver: &impl BibitemResolver,
    bibkeys: &[String],
) -> Result<ResolveResult, HexforgeError> {
    let found = resolver.find_by_bibkeys(bibkeys).await?;
    let found_keys: HashSet<&str> = found.iter().map(|b| b.bibkey.as_str()).collect();
    let missing: Vec<String> = bibkeys
        .iter()
        .filter(|k| !found_keys.contains(k.as_str()))
        .cloned()
        .collect();
    if missing.is_empty() {
        Ok(ResolveResult::Ok(found))
    } else {
        Ok(ResolveResult::MissingBibkeys(missing))
    }
}

/// Fetch all related data, build RenderContexts, sort, and render to HTML.
pub async fn render_bibitems_to_html(
    entity_fetcher: &impl RenderEntityFetcher,
    author_fetcher: &impl RenderAuthorFetcher,
    bibitems: Vec<BibItem>,
) -> Result<String, HexforgeError> {
    if bibitems.is_empty() {
        return Ok(String::new());
    }

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

    // Batch-fetch related entity names
    let journals_map = entity_fetcher.fetch_journal_names(&journal_ids).await?;
    let publishers_map = entity_fetcher.fetch_publisher_names(&publisher_ids).await?;
    let institutions_map = entity_fetcher
        .fetch_institution_names(&institution_ids)
        .await?;
    let schools_map = entity_fetcher.fetch_school_names(&school_ids).await?;
    let series_map = entity_fetcher.fetch_series_names(&series_ids).await?;
    let crossrefs_map = entity_fetcher.fetch_crossref_bibkeys(&crossref_ids).await?;

    // Batch-fetch author junction data
    let author_rows = author_fetcher.fetch_bibitem_authors(&bibitem_ids).await?;

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
