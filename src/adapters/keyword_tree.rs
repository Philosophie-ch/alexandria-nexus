//! Postgres implementation of keyword fetching by level.
//!
//! Implements `KeywordFetcher` from the process layer. All SQL, sqlx types,
//! and Postgres-specific logic lives here.

use hexforge::HexforgeError;
use hexforge::db_exports::{PgPool, query_as};

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
    async fn fetch_by_level(&self, level: i16) -> Result<Vec<Keyword>, HexforgeError> {
        query_as("SELECT * FROM keywords WHERE level = $1 ORDER BY name")
            .bind(level)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)
    }
}
