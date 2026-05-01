//! Postgres implementation of SnapshotFetcher.

use hexforge::db_exports::{PgPool, query_as};
use hexforge::{DataStore, HexforgeError, SortOrder};

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow, BibitemRefsRow};
use crate::domain::{
    Author, BibItem, BibitemNotes, Institution, Journal, Keyword, Publisher, School, Series,
};
use crate::process::snapshot::SnapshotFetcher;

// =============================================================================
// PgSnapshotFetcher
// =============================================================================

pub struct PgSnapshotFetcher {
    pool: PgPool,
}

impl PgSnapshotFetcher {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl SnapshotFetcher for PgSnapshotFetcher {
    fn fetch_authors(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Author>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            DataStore::<Author>::new(pool)
                .fetch_all(&())
                .await
                .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_journals(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Journal>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            DataStore::<Journal>::new(pool)
                .fetch_all(&())
                .await
                .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_publishers(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Publisher>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            DataStore::<Publisher>::new(pool)
                .fetch_all(&())
                .await
                .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_institutions(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Institution>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            DataStore::<Institution>::new(pool)
                .fetch_all(&())
                .await
                .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_schools(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<School>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            DataStore::<School>::new(pool)
                .fetch_all(&())
                .await
                .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_series(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Series>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            DataStore::<Series>::new(pool)
                .fetch_all(&())
                .await
                .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_keywords(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Keyword>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            DataStore::<Keyword>::new(pool)
                .fetch_all(&())
                .await
                .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_bibitems(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<BibItem>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            DataStore::<BibItem>::new(pool)
                .fetch_all(&())
                .await
                .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_bibitem_notes(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<BibitemNotes>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            DataStore::<BibitemNotes>::new(pool)
                .fetch_all_sorted(&(), &SortOrder::by("bibkey"))
                .await
                .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_bibitem_authors(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<BibitemAuthorsRow>, HexforgeError>> + Send
    {
        let pool = self.pool.clone();
        async move {
            query_as::<_, BibitemAuthorsRow>(
                "SELECT * FROM bibitem_authors ORDER BY bibkey, position",
            )
            .fetch_all(&pool)
            .await
            .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_bibitem_keywords(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<BibitemKeywordsRow>, HexforgeError>> + Send
    {
        let pool = self.pool.clone();
        async move {
            query_as::<_, BibitemKeywordsRow>(
                "SELECT * FROM bibitem_keywords ORDER BY bibkey, keyword_key",
            )
            .fetch_all(&pool)
            .await
            .map_err(HexforgeError::data_source)
        }
    }

    fn fetch_bibitem_refs(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<BibitemRefsRow>, HexforgeError>> + Send {
        let pool = self.pool.clone();
        async move {
            query_as::<_, BibitemRefsRow>(
                "SELECT * FROM bibitem_refs ORDER BY source_key, target_key",
            )
            .fetch_all(&pool)
            .await
            .map_err(HexforgeError::data_source)
        }
    }
}
