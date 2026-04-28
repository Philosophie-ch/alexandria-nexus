//! Postgres implementation of keyword fetching by level.
//!
//! Implements `KeywordFetcher` from the process layer.

use hexforge::db_exports::PgPool;
use hexforge::{DataStore, HexforgeError, SortOrder};

use crate::domain::Keyword;
use crate::process::keyword_tree::KeywordFetcher;

/// Postgres-backed keyword fetcher.
pub struct PgKeywordFetcher<'a> {
    pool: &'a PgPool,
}

impl<'a> PgKeywordFetcher<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl KeywordFetcher for PgKeywordFetcher<'_> {
    async fn fetch_all(&self) -> Result<Vec<Keyword>, HexforgeError> {
        DataStore::<Keyword>::new(self.pool.clone())
            .fetch_all_sorted(&(), &SortOrder::by("level").then("name"))
            .await
            .map_err(HexforgeError::data_source)
    }
}
