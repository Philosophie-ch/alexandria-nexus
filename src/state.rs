//! Application state — holds all data sources and shared resources.

use hexforge::{DatabasePool, PostgresDataSource};

use crate::entities::{Author, BibItem, Institution, Journal, Keyword, Publisher, School, Series};

/// Shared application state.
///
/// Contains the database pool and typed data sources for every entity.
/// Data sources are cheap clones (Arc-wrapped pool reference).
#[derive(Clone)]
pub struct AppState {
    pub pool: DatabasePool,
    pub author_ds: PostgresDataSource<Author>,
    pub journal_ds: PostgresDataSource<Journal>,
    pub publisher_ds: PostgresDataSource<Publisher>,
    pub institution_ds: PostgresDataSource<Institution>,
    pub school_ds: PostgresDataSource<School>,
    pub series_ds: PostgresDataSource<Series>,
    pub keyword_ds: PostgresDataSource<Keyword>,
    pub bibitem_ds: PostgresDataSource<BibItem>,
}

impl AppState {
    pub fn new(pool: DatabasePool) -> Self {
        let author_ds = pool.data_source::<Author>();
        let journal_ds = pool.data_source::<Journal>();
        let publisher_ds = pool.data_source::<Publisher>();
        let institution_ds = pool.data_source::<Institution>();
        let school_ds = pool.data_source::<School>();
        let series_ds = pool.data_source::<Series>();
        let keyword_ds = pool.data_source::<Keyword>();
        let bibitem_ds = pool.data_source::<BibItem>();

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
