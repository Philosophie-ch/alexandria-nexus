//! Postgres implementations of render process traits.
//!
//! Implements `BibitemResolver`, `RenderNameFetcher`, and `RenderAuthorFetcher`
//! from the process layer.

use std::collections::HashMap;

use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgPool, query_as};

use crate::adapters::db::queries::junctions::fetch_bibitem_authors_batch;
use crate::domain::junctions::BibitemAuthorsRow;
use crate::domain::{Author, BibItem};
use crate::process::render::{BibitemResolver, RenderAuthorFetcher, RenderNameFetcher};
use crate::state::AppState;

/// Row type for batch name lookups.
#[derive(Debug, FromRow)]
struct IdNameRow {
    id: i64,
    name: String,
}

/// Concrete resolver that uses DataStore to find bibitems.
pub struct PgBibitemResolver<'a> {
    state: &'a AppState,
}

impl<'a> PgBibitemResolver<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }
}

impl BibitemResolver for PgBibitemResolver<'_> {
    async fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<BibItem>, HexforgeError> {
        self.state
            .bibitem_ds
            .find_by_ids(ids)
            .await
            .map_err(HexforgeError::data_source)
    }

    async fn find_by_bibkeys(&self, bibkeys: &[String]) -> Result<Vec<BibItem>, HexforgeError> {
        use hexforge::WhereClause;
        let mut results = Vec::new();
        for bibkey in bibkeys {
            let found = self
                .state
                .bibitem_ds
                .find_one(WhereClause::new("bibkey = $1").bind(bibkey.clone()))
                .await
                .map_err(HexforgeError::data_source)?;
            if let Some(item) = found {
                results.push(item);
            }
        }
        Ok(results)
    }
}

/// Concrete fetcher for entity names using raw SQL.
pub struct PgRenderNameFetcher<'a> {
    pool: &'a PgPool,
}

impl<'a> PgRenderNameFetcher<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl RenderNameFetcher for PgRenderNameFetcher<'_> {
    async fn fetch_names(
        &self,
        table: &str,
        name_column: &str,
        ids: &[i64],
    ) -> Result<HashMap<i64, String>, HexforgeError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let sql = format!("SELECT id, {name_column} AS name FROM {table} WHERE id = ANY($1)");
        let rows: Vec<IdNameRow> = query_as(&sql)
            .bind(ids)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(rows.into_iter().map(|r| (r.id, r.name)).collect())
    }
}

/// Concrete fetcher for author junction data and author entities.
pub struct PgRenderAuthorFetcher<'a> {
    state: &'a AppState,
}

impl<'a> PgRenderAuthorFetcher<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }
}

impl RenderAuthorFetcher for PgRenderAuthorFetcher<'_> {
    async fn fetch_bibitem_authors(
        &self,
        bibitem_ids: &[i64],
    ) -> Result<Vec<BibitemAuthorsRow>, HexforgeError> {
        fetch_bibitem_authors_batch(self.state.pool.pool(), bibitem_ids).await
    }

    async fn fetch_authors_by_ids(
        &self,
        ids: &[i64],
    ) -> Result<HashMap<i64, Author>, HexforgeError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let authors = self
            .state
            .author_ds
            .find_by_ids(ids)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(authors.into_iter().map(|a| (a.id, a)).collect())
    }
}
