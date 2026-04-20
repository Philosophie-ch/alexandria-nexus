//! Export process — orchestrates entity fetching and CSV generation.
//!
//! Defines traits for I/O operations and coordinates between data fetching
//! (via traits) and CSV formatting (via pure logic functions).
//! No AppState, no PgPool, no sqlx, no SQL — only abstract contracts.
//!
//! **Architecture:** This module defines WHAT operations are needed via traits.
//! Concrete I/O implementations live in `crate::adapters::export`.

use std::collections::{HashMap, HashSet};
use std::future::Future;

use hexforge::HexforgeError;

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow};
use crate::domain::{Author, BibItem, Institution, Journal, Keyword, Publisher, School, Series};
use crate::logic::export::{
    BibitemExportRequest, ExportFormat, build_author_rows, build_bibitem_expanded_rows,
    build_bibitem_id_rows, build_institution_rows, build_journal_rows, build_keyword_rows,
    build_publisher_rows, build_school_rows, build_series_rows,
};

// =============================================================================
// Error type
// =============================================================================

/// Export error that the handler layer converts into HTTP responses.
#[derive(Debug)]
pub enum ExportError {
    MissingIds(Vec<i64>),
    MissingKeys(Vec<String>),
    MissingBibkeys(Vec<String>),
    BadRequest,
    Internal(HexforgeError),
}

impl From<HexforgeError> for ExportError {
    fn from(e: HexforgeError) -> Self {
        ExportError::Internal(e)
    }
}

// =============================================================================
// Traits — contracts for I/O operations that adapters implement
// =============================================================================

/// Contract for fetching keyed entities (authors, journals, publishers, etc.)
/// by all, IDs, or key column values.
pub trait KeyedEntityFetcher<T>: Send + Sync {
    /// Fetch all entities.
    fn fetch_all(&self) -> impl Future<Output = Result<Vec<T>, ExportError>> + Send;

    /// Fetch entities by their IDs.
    fn fetch_by_ids(&self, ids: &[i64])
    -> impl Future<Output = Result<Vec<T>, ExportError>> + Send;

    /// Fetch entities by their key column values (e.g., author_key, journal_key).
    fn fetch_by_keys(
        &self,
        keys: &[String],
    ) -> impl Future<Output = Result<Vec<T>, ExportError>> + Send;
}

/// Contract for fetching entities by IDs into a HashMap keyed by entity ID.
pub trait EntityBatchFetcher<T>: Send + Sync {
    /// Batch-fetch entities by IDs, returning a map of id -> entity.
    fn fetch_map(
        &self,
        ids: &HashSet<i64>,
    ) -> impl Future<Output = Result<HashMap<i64, T>, ExportError>> + Send;
}

/// Contract for fetching bibitem junction data (authors and keywords).
pub trait ExportJunctionFetcher: Send + Sync {
    /// Batch-fetch bibitem-author junction rows for the given bibitem IDs.
    fn fetch_bibitem_authors_batch(
        &self,
        bibitem_ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemAuthorsRow>, HexforgeError>> + Send;

    /// Batch-fetch bibitem-keyword junction rows for the given bibitem IDs.
    fn fetch_bibitem_keywords_batch(
        &self,
        bibitem_ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemKeywordsRow>, HexforgeError>> + Send;
}

/// Contract for fetching bibitems by all, IDs, or bibkeys.
pub trait BibitemFetcher: Send + Sync {
    /// Fetch all bibitems.
    fn fetch_all(&self) -> impl Future<Output = Result<Vec<BibItem>, ExportError>> + Send;

    /// Fetch bibitems by their IDs.
    fn fetch_by_ids(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibItem>, ExportError>> + Send;

    /// Fetch bibitems by their bibkeys.
    fn fetch_by_bibkeys(
        &self,
        bibkeys: &[String],
    ) -> impl Future<Output = Result<Vec<BibItem>, ExportError>> + Send;
}

// =============================================================================
// Entity export orchestration
// =============================================================================

/// Fetch entities by request criteria and build CSV rows.
async fn fetch_and_build<T>(
    fetcher: &impl KeyedEntityFetcher<T>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
    build_rows: impl FnOnce(&[T]) -> Vec<Vec<String>>,
) -> Result<Vec<Vec<String>>, ExportError> {
    let entities = if all {
        fetcher.fetch_all().await?
    } else if let Some(ref id_list) = ids {
        fetcher.fetch_by_ids(id_list).await?
    } else if let Some(ref key_list) = keys {
        fetcher.fetch_by_keys(key_list).await?
    } else {
        return Err(ExportError::BadRequest);
    };

    Ok(build_rows(&entities))
}

/// Export authors as CSV rows.
pub async fn export_authors(
    fetcher: &impl KeyedEntityFetcher<Author>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Vec<String>>, ExportError> {
    fetch_and_build(fetcher, all, ids, keys, build_author_rows).await
}

/// Export journals as CSV rows.
pub async fn export_journals(
    fetcher: &impl KeyedEntityFetcher<Journal>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Vec<String>>, ExportError> {
    fetch_and_build(fetcher, all, ids, keys, build_journal_rows).await
}

/// Export publishers as CSV rows.
pub async fn export_publishers(
    fetcher: &impl KeyedEntityFetcher<Publisher>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Vec<String>>, ExportError> {
    fetch_and_build(fetcher, all, ids, keys, build_publisher_rows).await
}

/// Export institutions as CSV rows.
pub async fn export_institutions(
    fetcher: &impl KeyedEntityFetcher<Institution>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Vec<String>>, ExportError> {
    fetch_and_build(fetcher, all, ids, keys, build_institution_rows).await
}

/// Export schools as CSV rows.
pub async fn export_schools(
    fetcher: &impl KeyedEntityFetcher<School>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Vec<String>>, ExportError> {
    fetch_and_build(fetcher, all, ids, keys, build_school_rows).await
}

/// Export series as CSV rows.
pub async fn export_series(
    fetcher: &impl KeyedEntityFetcher<Series>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Vec<String>>, ExportError> {
    fetch_and_build(fetcher, all, ids, keys, build_series_rows).await
}

/// Export keywords as CSV rows.
pub async fn export_keywords(
    fetcher: &impl KeyedEntityFetcher<Keyword>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Vec<String>>, ExportError> {
    fetch_and_build(fetcher, all, ids, keys, build_keyword_rows).await
}

// =============================================================================
// Bibitem export orchestration
// =============================================================================

/// Export bibitems as CSV rows.
///
/// Supports two formats:
/// - `Expanded`: human-readable with resolved names
/// - `Ids`: machine-readable with raw foreign key IDs
#[allow(clippy::too_many_arguments)]
pub async fn export_bibitems(
    bibitem_fetcher: &impl BibitemFetcher,
    junction_fetcher: &impl ExportJunctionFetcher,
    author_batch: &impl EntityBatchFetcher<Author>,
    journal_batch: &impl EntityBatchFetcher<Journal>,
    publisher_batch: &impl EntityBatchFetcher<Publisher>,
    institution_batch: &impl EntityBatchFetcher<Institution>,
    school_batch: &impl EntityBatchFetcher<School>,
    series_batch: &impl EntityBatchFetcher<Series>,
    bibitem_batch: &impl EntityBatchFetcher<BibItem>,
    keyword_batch: &impl EntityBatchFetcher<Keyword>,
    req: BibitemExportRequest,
) -> Result<Vec<Vec<String>>, ExportError> {
    // 1. Fetch bibitems based on selection criteria
    let bibitems = if req.all {
        bibitem_fetcher.fetch_all().await?
    } else if let Some(ref id_list) = req.ids {
        bibitem_fetcher.fetch_by_ids(id_list).await?
    } else if let Some(ref bibkey_list) = req.bibkeys {
        bibitem_fetcher.fetch_by_bibkeys(bibkey_list).await?
    } else {
        return Err(ExportError::BadRequest);
    };

    match req.format {
        ExportFormat::Ids => assemble_bibitem_id_rows(&bibitems, junction_fetcher).await,
        ExportFormat::Expanded => {
            assemble_bibitem_expanded_rows(
                &bibitems,
                junction_fetcher,
                author_batch,
                journal_batch,
                publisher_batch,
                institution_batch,
                school_batch,
                series_batch,
                bibitem_batch,
                keyword_batch,
            )
            .await
        }
    }
}

/// Fetch junction data and build bibitems IDs CSV rows.
async fn assemble_bibitem_id_rows(
    bibitems: &[BibItem],
    junction_fetcher: &impl ExportJunctionFetcher,
) -> Result<Vec<Vec<String>>, ExportError> {
    if bibitems.is_empty() {
        return Ok(build_bibitem_id_rows(bibitems, &[], &[]));
    }

    let bibitem_ids: Vec<i64> = bibitems.iter().map(|b| b.id).collect();

    let (author_rows, keyword_rows) = tokio::try_join!(
        async {
            junction_fetcher
                .fetch_bibitem_authors_batch(&bibitem_ids)
                .await
                .map_err(ExportError::from)
        },
        async {
            junction_fetcher
                .fetch_bibitem_keywords_batch(&bibitem_ids)
                .await
                .map_err(ExportError::from)
        },
    )?;

    Ok(build_bibitem_id_rows(bibitems, &author_rows, &keyword_rows))
}

/// Fetch all related data and build bibitems expanded CSV rows.
#[allow(clippy::too_many_arguments)]
async fn assemble_bibitem_expanded_rows(
    bibitems: &[BibItem],
    junction_fetcher: &impl ExportJunctionFetcher,
    author_batch: &impl EntityBatchFetcher<Author>,
    journal_batch: &impl EntityBatchFetcher<Journal>,
    publisher_batch: &impl EntityBatchFetcher<Publisher>,
    institution_batch: &impl EntityBatchFetcher<Institution>,
    school_batch: &impl EntityBatchFetcher<School>,
    series_batch: &impl EntityBatchFetcher<Series>,
    bibitem_batch: &impl EntityBatchFetcher<BibItem>,
    keyword_batch: &impl EntityBatchFetcher<Keyword>,
) -> Result<Vec<Vec<String>>, ExportError> {
    if bibitems.is_empty() {
        return Ok(build_bibitem_expanded_rows(
            bibitems,
            &[],
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        ));
    }

    let bibitem_ids: Vec<i64> = bibitems.iter().map(|b| b.id).collect();

    let mut journal_ids = HashSet::new();
    let mut publisher_ids = HashSet::new();
    let mut institution_ids = HashSet::new();
    let mut school_ids = HashSet::new();
    let mut series_ids = HashSet::new();
    let mut crossref_ids = HashSet::new();

    for bib in bibitems {
        if let Some(id) = bib.journal_id {
            journal_ids.insert(id);
        }
        if let Some(id) = bib.publisher_id {
            publisher_ids.insert(id);
        }
        if let Some(id) = bib.institution_id {
            institution_ids.insert(id);
        }
        if let Some(id) = bib.school_id {
            school_ids.insert(id);
        }
        if let Some(id) = bib.series_id {
            series_ids.insert(id);
        }
        if let Some(id) = bib.crossref_id {
            crossref_ids.insert(id);
        }
    }

    // Round 1: all independent fetches concurrently
    let (
        author_rows,
        keyword_rows,
        journals_map,
        publishers_map,
        institutions_map,
        schools_map,
        series_map,
        crossrefs_map,
    ) = tokio::try_join!(
        async {
            junction_fetcher
                .fetch_bibitem_authors_batch(&bibitem_ids)
                .await
                .map_err(ExportError::from)
        },
        async {
            junction_fetcher
                .fetch_bibitem_keywords_batch(&bibitem_ids)
                .await
                .map_err(ExportError::from)
        },
        journal_batch.fetch_map(&journal_ids),
        publisher_batch.fetch_map(&publisher_ids),
        institution_batch.fetch_map(&institution_ids),
        school_batch.fetch_map(&school_ids),
        series_batch.fetch_map(&series_ids),
        bibitem_batch.fetch_map(&crossref_ids),
    )?;

    // Round 2: author and keyword maps depend on junction data
    let all_author_ids: HashSet<i64> = author_rows.iter().map(|r| r.author_id).collect();
    let all_keyword_ids: HashSet<i64> = keyword_rows.iter().map(|r| r.keyword_id).collect();
    let (authors_map, keywords_map) = tokio::try_join!(
        author_batch.fetch_map(&all_author_ids),
        keyword_batch.fetch_map(&all_keyword_ids),
    )?;

    Ok(build_bibitem_expanded_rows(
        bibitems,
        &author_rows,
        &keyword_rows,
        &authors_map,
        &journals_map,
        &publishers_map,
        &institutions_map,
        &schools_map,
        &series_map,
        &crossrefs_map,
        &keywords_map,
    ))
}
