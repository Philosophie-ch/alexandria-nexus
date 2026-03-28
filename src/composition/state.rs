//! Application state — holds all data sources and shared resources.

use hexforge::{DataStore, DatabasePool};

use crate::adapters::db::queries::{
    AuthorQuery, BibItemQuery, InstitutionQuery, JournalQuery, KeywordQuery, PublisherQuery,
    SchoolQuery, SeriesQuery,
};
use crate::domain::{Author, BibItem, Institution, Journal, Keyword, Publisher, School, Series};

/// Shared application state.
///
/// Contains the database pool and typed data sources for every entity.
/// Data sources are cheap clones (Arc-wrapped pool reference).
/// Each data source is parameterized with its query filter type so that
/// list endpoints can accept typed filter parameters (e.g., `?family_name=kant`).
#[derive(Clone)]
pub struct AppState {
    pub pool: DatabasePool,
    pub author_ds: DataStore<Author, AuthorQuery>,
    pub journal_ds: DataStore<Journal, JournalQuery>,
    pub publisher_ds: DataStore<Publisher, PublisherQuery>,
    pub institution_ds: DataStore<Institution, InstitutionQuery>,
    pub school_ds: DataStore<School, SchoolQuery>,
    pub series_ds: DataStore<Series, SeriesQuery>,
    pub keyword_ds: DataStore<Keyword, KeywordQuery>,
    pub bibitem_ds: DataStore<BibItem, BibItemQuery>,
}

impl AppState {
    pub fn new(pool: DatabasePool) -> Self {
        let author_ds = DataStore::new(pool.clone());
        let journal_ds = DataStore::new(pool.clone());
        let publisher_ds = DataStore::new(pool.clone());
        let institution_ds = DataStore::new(pool.clone());
        let school_ds = DataStore::new(pool.clone());
        let series_ds = DataStore::new(pool.clone());
        let keyword_ds = DataStore::new(pool.clone());
        let bibitem_ds = DataStore::new(pool.clone());

        Self {
            pool,
            author_ds,
            journal_ds,
            publisher_ds,
            institution_ds,
            school_ds,
            series_ds,
            keyword_ds,
            bibitem_ds,
        }
    }
}
