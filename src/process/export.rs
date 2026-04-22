//! Export process — orchestrates entity fetching and assembles domain data.
//!
//! Defines traits for I/O operations and coordinates between data fetching
//! (via traits) and result assembly. Returns domain types — serialization to
//! external formats happen in the adapters layer.
//!
//! Orchestration only — no I/O, no framework dependencies.

use std::collections::{HashMap, HashSet};
use std::future::Future;

use hexforge::HexforgeError;

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow};
use crate::domain::{Author, BibItem, Institution, Journal, Keyword, Publisher, School, Series};
use crate::logic::export::{BibitemExportRequest, ExportFormat};

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
// Result types — assembled domain data, ready for serialization by adapters
// =============================================================================

/// All domain data needed to render a bibitem export, in either format.
/// Serialization to external formats happens in the adapter layer.
pub enum BibitemExportData {
    Ids {
        bibitems: Vec<BibItem>,
        author_rows: Vec<BibitemAuthorsRow>,
        keyword_rows: Vec<BibitemKeywordsRow>,
    },
    Expanded(Box<BibitemExpandedData>),
}

pub struct BibitemExpandedData {
    pub bibitems: Vec<BibItem>,
    pub author_rows: Vec<BibitemAuthorsRow>,
    pub keyword_rows: Vec<BibitemKeywordsRow>,
    pub authors_map: HashMap<String, Author>,
    pub journals_map: HashMap<String, Journal>,
    pub publishers_map: HashMap<String, Publisher>,
    pub institutions_map: HashMap<String, Institution>,
    pub schools_map: HashMap<String, School>,
    pub series_map: HashMap<String, Series>,
    pub crossrefs_map: HashMap<String, BibItem>,
    pub keywords_map: HashMap<String, Keyword>,
}

// =============================================================================
// Traits — contracts for I/O operations that adapters implement
// =============================================================================

/// Contract for fetching keyed entities (authors, journals, publishers, etc.)
/// by all, IDs, or key column values.
pub trait KeyedEntityFetcher<T>: Send + Sync {
    fn fetch_all(&self) -> impl Future<Output = Result<Vec<T>, ExportError>> + Send;
    fn fetch_by_ids(&self, ids: &[i64])
    -> impl Future<Output = Result<Vec<T>, ExportError>> + Send;
    fn fetch_by_keys(
        &self,
        keys: &[String],
    ) -> impl Future<Output = Result<Vec<T>, ExportError>> + Send;
}

/// Contract for fetching a map of entities keyed by their business key.
pub trait EntityBatchFetcher<T>: Send + Sync {
    fn fetch_map(
        &self,
        keys: &HashSet<String>,
    ) -> impl Future<Output = Result<HashMap<String, T>, ExportError>> + Send;
}

/// Contract for fetching junction rows for a batch of bibitem IDs.
pub trait ExportJunctionFetcher: Send + Sync {
    fn fetch_bibitem_authors_batch(
        &self,
        bibitem_ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemAuthorsRow>, HexforgeError>> + Send;

    fn fetch_bibitem_keywords_batch(
        &self,
        bibitem_ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemKeywordsRow>, HexforgeError>> + Send;
}

/// Contract for fetching bibitems for export.
pub trait BibitemFetcher: Send + Sync {
    fn fetch_all(&self) -> impl Future<Output = Result<Vec<BibItem>, ExportError>> + Send;
    fn fetch_by_ids(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibItem>, ExportError>> + Send;
    fn fetch_by_bibkeys(
        &self,
        bibkeys: &[String],
    ) -> impl Future<Output = Result<Vec<BibItem>, ExportError>> + Send;
}

// =============================================================================
// Entity export orchestration — returns domain types
// =============================================================================

async fn fetch_entities<T>(
    fetcher: &impl KeyedEntityFetcher<T>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<T>, ExportError> {
    if all {
        fetcher.fetch_all().await
    } else if let Some(ref id_list) = ids {
        fetcher.fetch_by_ids(id_list).await
    } else if let Some(ref key_list) = keys {
        fetcher.fetch_by_keys(key_list).await
    } else {
        Err(ExportError::BadRequest)
    }
}

pub async fn export_authors(
    fetcher: &impl KeyedEntityFetcher<Author>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Author>, ExportError> {
    fetch_entities(fetcher, all, ids, keys).await
}

pub async fn export_journals(
    fetcher: &impl KeyedEntityFetcher<Journal>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Journal>, ExportError> {
    fetch_entities(fetcher, all, ids, keys).await
}

pub async fn export_publishers(
    fetcher: &impl KeyedEntityFetcher<Publisher>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Publisher>, ExportError> {
    fetch_entities(fetcher, all, ids, keys).await
}

pub async fn export_institutions(
    fetcher: &impl KeyedEntityFetcher<Institution>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Institution>, ExportError> {
    fetch_entities(fetcher, all, ids, keys).await
}

pub async fn export_schools(
    fetcher: &impl KeyedEntityFetcher<School>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<School>, ExportError> {
    fetch_entities(fetcher, all, ids, keys).await
}

pub async fn export_series(
    fetcher: &impl KeyedEntityFetcher<Series>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Series>, ExportError> {
    fetch_entities(fetcher, all, ids, keys).await
}

pub async fn export_keywords(
    fetcher: &impl KeyedEntityFetcher<Keyword>,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Vec<Keyword>, ExportError> {
    fetch_entities(fetcher, all, ids, keys).await
}

// =============================================================================
// Bibitem export orchestration — assembles all domain data
// =============================================================================

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
) -> Result<BibitemExportData, ExportError> {
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
        ExportFormat::Ids => assemble_ids_data(bibitems, junction_fetcher).await,
        ExportFormat::Expanded => {
            assemble_expanded_data(
                bibitems,
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

async fn assemble_ids_data(
    bibitems: Vec<BibItem>,
    junction_fetcher: &impl ExportJunctionFetcher,
) -> Result<BibitemExportData, ExportError> {
    if bibitems.is_empty() {
        return Ok(BibitemExportData::Ids {
            bibitems,
            author_rows: vec![],
            keyword_rows: vec![],
        });
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

    Ok(BibitemExportData::Ids {
        bibitems,
        author_rows,
        keyword_rows,
    })
}

#[allow(clippy::too_many_arguments)]
async fn assemble_expanded_data(
    bibitems: Vec<BibItem>,
    junction_fetcher: &impl ExportJunctionFetcher,
    author_batch: &impl EntityBatchFetcher<Author>,
    journal_batch: &impl EntityBatchFetcher<Journal>,
    publisher_batch: &impl EntityBatchFetcher<Publisher>,
    institution_batch: &impl EntityBatchFetcher<Institution>,
    school_batch: &impl EntityBatchFetcher<School>,
    series_batch: &impl EntityBatchFetcher<Series>,
    bibitem_batch: &impl EntityBatchFetcher<BibItem>,
    keyword_batch: &impl EntityBatchFetcher<Keyword>,
) -> Result<BibitemExportData, ExportError> {
    if bibitems.is_empty() {
        return Ok(BibitemExportData::Expanded(Box::new(BibitemExpandedData {
            bibitems,
            author_rows: vec![],
            keyword_rows: vec![],
            authors_map: HashMap::new(),
            journals_map: HashMap::new(),
            publishers_map: HashMap::new(),
            institutions_map: HashMap::new(),
            schools_map: HashMap::new(),
            series_map: HashMap::new(),
            crossrefs_map: HashMap::new(),
            keywords_map: HashMap::new(),
        })));
    }

    let bibitem_ids: Vec<i64> = bibitems.iter().map(|b| b.id).collect();

    let mut journal_keys = HashSet::new();
    let mut publisher_keys = HashSet::new();
    let mut institution_keys = HashSet::new();
    let mut school_keys = HashSet::new();
    let mut series_keys = HashSet::new();
    let mut crossref_keys = HashSet::new();

    for bib in &bibitems {
        if let Some(ref k) = bib.journal_key {
            journal_keys.insert(k.clone());
        }
        if let Some(ref k) = bib.publisher_key {
            publisher_keys.insert(k.clone());
        }
        if let Some(ref k) = bib.institution_key {
            institution_keys.insert(k.clone());
        }
        if let Some(ref k) = bib.school_key {
            school_keys.insert(k.clone());
        }
        if let Some(ref k) = bib.series_key {
            series_keys.insert(k.clone());
        }
        if let Some(ref k) = bib.crossref {
            crossref_keys.insert(k.clone());
        }
    }

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
        journal_batch.fetch_map(&journal_keys),
        publisher_batch.fetch_map(&publisher_keys),
        institution_batch.fetch_map(&institution_keys),
        school_batch.fetch_map(&school_keys),
        series_batch.fetch_map(&series_keys),
        bibitem_batch.fetch_map(&crossref_keys),
    )?;

    let all_author_keys: HashSet<String> =
        author_rows.iter().map(|r| r.author_key.clone()).collect();
    let all_keyword_keys: HashSet<String> =
        keyword_rows.iter().map(|r| r.keyword_key.clone()).collect();
    let (authors_map, keywords_map) = tokio::try_join!(
        author_batch.fetch_map(&all_author_keys),
        keyword_batch.fetch_map(&all_keyword_keys),
    )?;

    Ok(BibitemExportData::Expanded(Box::new(BibitemExpandedData {
        bibitems,
        author_rows,
        keyword_rows,
        authors_map,
        journals_map,
        publishers_map,
        institutions_map,
        schools_map,
        series_map,
        crossrefs_map,
        keywords_map,
    })))
}
