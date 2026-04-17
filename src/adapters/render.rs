//! Postgres implementations of render process traits.
//!
//! Implements `BibitemResolver`, `RenderEntityFetcher`, and `RenderAuthorFetcher`
//! from the process layer.

use std::collections::HashMap;

use hexforge::db_exports::{FromRow, PgPool, query_as};
use hexforge::{DataStore, HexforgeError, WhereClause};

use crate::adapters::db::queries::junctions::fetch_bibitem_authors_batch;
use crate::adapters::db::queries::{AuthorQuery, BibItemQuery};
use crate::domain::junctions::BibitemAuthorsRow;
use crate::domain::{Author, BibItem};
use crate::process::render::{BibitemResolver, RenderAuthorFetcher, RenderEntityFetcher};

// =============================================================================
// Shared SQL helper
// =============================================================================

/// Row type for batch name lookups.
#[derive(Debug, FromRow)]
struct IdNameRow {
    id: i64,
    name: String,
}

/// Batch-fetch a single column from a table for a set of IDs.
async fn batch_fetch_names(
    pool: &PgPool,
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
        .fetch_all(pool)
        .await
        .map_err(HexforgeError::data_source)?;
    Ok(rows.into_iter().map(|r| (r.id, r.name)).collect())
}

// =============================================================================
// BibitemResolver
// =============================================================================

/// Concrete resolver that uses DataStore to find bibitems.
pub struct PgBibitemResolver<'a> {
    bibitem_ds: &'a DataStore<BibItem, BibItemQuery>,
}

impl<'a> PgBibitemResolver<'a> {
    pub fn new(bibitem_ds: &'a DataStore<BibItem, BibItemQuery>) -> Self {
        Self { bibitem_ds }
    }
}

impl BibitemResolver for PgBibitemResolver<'_> {
    async fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<BibItem>, HexforgeError> {
        self.bibitem_ds
            .find_by_ids(ids)
            .await
            .map_err(HexforgeError::data_source)
    }

    async fn find_by_bibkeys(&self, bibkeys: &[String]) -> Result<Vec<BibItem>, HexforgeError> {
        let mut results = Vec::new();
        for bibkey in bibkeys {
            let found = self
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

// =============================================================================
// RenderEntityFetcher
// =============================================================================

/// Concrete fetcher for entity names using raw SQL.
pub struct PgRenderEntityFetcher<'a> {
    pool: &'a PgPool,
}

impl<'a> PgRenderEntityFetcher<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl RenderEntityFetcher for PgRenderEntityFetcher<'_> {
    async fn fetch_journal_names(
        &self,
        ids: &[i64],
    ) -> Result<HashMap<i64, String>, HexforgeError> {
        batch_fetch_names(self.pool, "journals", "name_unicode", ids).await
    }

    async fn fetch_publisher_names(
        &self,
        ids: &[i64],
    ) -> Result<HashMap<i64, String>, HexforgeError> {
        batch_fetch_names(self.pool, "publishers", "name_unicode", ids).await
    }

    async fn fetch_institution_names(
        &self,
        ids: &[i64],
    ) -> Result<HashMap<i64, String>, HexforgeError> {
        batch_fetch_names(self.pool, "institutions", "name_unicode", ids).await
    }

    async fn fetch_school_names(&self, ids: &[i64]) -> Result<HashMap<i64, String>, HexforgeError> {
        batch_fetch_names(self.pool, "schools", "name_unicode", ids).await
    }

    async fn fetch_series_names(&self, ids: &[i64]) -> Result<HashMap<i64, String>, HexforgeError> {
        batch_fetch_names(self.pool, "series", "name_unicode", ids).await
    }

    async fn fetch_crossref_bibkeys(
        &self,
        ids: &[i64],
    ) -> Result<HashMap<i64, String>, HexforgeError> {
        batch_fetch_names(self.pool, "bibitems", "bibkey", ids).await
    }
}

// =============================================================================
// RenderAuthorFetcher
// =============================================================================

/// Concrete fetcher for author junction data and author entities.
pub struct PgRenderAuthorFetcher<'a> {
    pool: &'a PgPool,
    author_ds: &'a DataStore<Author, AuthorQuery>,
}

impl<'a> PgRenderAuthorFetcher<'a> {
    pub fn new(pool: &'a PgPool, author_ds: &'a DataStore<Author, AuthorQuery>) -> Self {
        Self { pool, author_ds }
    }
}

impl RenderAuthorFetcher for PgRenderAuthorFetcher<'_> {
    async fn fetch_bibitem_authors(
        &self,
        bibitem_ids: &[i64],
    ) -> Result<Vec<BibitemAuthorsRow>, HexforgeError> {
        fetch_bibitem_authors_batch(self.pool, bibitem_ids).await
    }

    async fn fetch_authors_by_ids(
        &self,
        ids: &[i64],
    ) -> Result<HashMap<i64, Author>, HexforgeError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let authors = self
            .author_ds
            .find_by_ids(ids)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(authors.into_iter().map(|a| (a.id, a)).collect())
    }
}
