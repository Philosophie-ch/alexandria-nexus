//! Postgres implementations of render process traits.
//!
//! Implements `BibitemResolver`, `RenderEntityFetcher`, and `RenderAuthorFetcher`
//! from the process layer.

use std::collections::HashMap;

use hexforge::db_exports::{FromRow, PgPool, query_as};
use hexforge::{DataStore, HexforgeError, PgQuery, WhereClause};

use crate::domain::junctions::BibitemAuthorsRow;
use crate::domain::{Author, BibItem};
use crate::process::render::{
    BibitemResolver, RenderAuthorFetcher, RenderEntityFetcher, TransitiveDepsResolver,
};

// =============================================================================
// Shared SQL helper
// =============================================================================

/// Row type for batch name lookups.
#[derive(Debug, FromRow)]
struct KeyNameRow {
    key: String,
    name: String,
}

/// Batch-fetch a single column from a table for a set of entity keys.
async fn batch_fetch_names_by_key(
    pool: &PgPool,
    table: &str,
    key_column: &str,
    name_column: &str,
    keys: &[String],
) -> Result<HashMap<String, String>, HexforgeError> {
    if keys.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT {key_column} AS key, {name_column} AS name FROM {table} WHERE {key_column} = ANY($1)"
    );
    let rows: Vec<KeyNameRow> = query_as(&sql)
        .bind(keys)
        .fetch_all(pool)
        .await
        .map_err(HexforgeError::data_source)?;
    Ok(rows.into_iter().map(|r| (r.key, r.name)).collect())
}

// =============================================================================
// BibitemResolver
// =============================================================================

/// Concrete resolver that uses DataStore to find bibitems.
pub struct PgBibitemResolver<'a, Q> {
    bibitem_ds: &'a DataStore<BibItem, Q>,
}

impl<'a, Q> PgBibitemResolver<'a, Q> {
    pub fn new(bibitem_ds: &'a DataStore<BibItem, Q>) -> Self {
        Self { bibitem_ds }
    }
}

impl<Q: PgQuery + 'static> BibitemResolver for PgBibitemResolver<'_, Q> {
    async fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<BibItem>, HexforgeError> {
        self.bibitem_ds
            .find_by_ids(ids)
            .await
            .map_err(HexforgeError::data_source)
    }

    async fn find_by_bibkeys(&self, bibkeys: &[String]) -> Result<Vec<BibItem>, HexforgeError> {
        if bibkeys.is_empty() {
            return Ok(Vec::new());
        }
        self.bibitem_ds
            .find_many(
                WhereClause::new("bibkey = ANY($1)")
                    .bind(bibkeys.to_vec())
                    .map_err(HexforgeError::data_source)?,
            )
            .await
            .map_err(HexforgeError::data_source)
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
        keys: &[String],
    ) -> Result<HashMap<String, String>, HexforgeError> {
        batch_fetch_names_by_key(self.pool, "journals", "journal_key", "name_unicode", keys).await
    }

    async fn fetch_publisher_names(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, String>, HexforgeError> {
        batch_fetch_names_by_key(
            self.pool,
            "publishers",
            "publisher_key",
            "name_unicode",
            keys,
        )
        .await
    }

    async fn fetch_institution_names(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, String>, HexforgeError> {
        batch_fetch_names_by_key(
            self.pool,
            "institutions",
            "institution_key",
            "name_unicode",
            keys,
        )
        .await
    }

    async fn fetch_school_names(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, String>, HexforgeError> {
        batch_fetch_names_by_key(self.pool, "schools", "school_key", "name_unicode", keys).await
    }

    async fn fetch_series_names(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, String>, HexforgeError> {
        batch_fetch_names_by_key(self.pool, "series", "series_key", "name_unicode", keys).await
    }

    async fn fetch_crossref_bibkeys(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, String>, HexforgeError> {
        batch_fetch_names_by_key(self.pool, "bibitems", "bibkey", "bibkey", keys).await
    }
}

// =============================================================================
// RenderAuthorFetcher
// =============================================================================

/// Concrete fetcher for author junction data and author entities.
pub struct PgRenderAuthorFetcher<'a> {
    pool: &'a PgPool,
}

impl<'a> PgRenderAuthorFetcher<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl RenderAuthorFetcher for PgRenderAuthorFetcher<'_> {
    async fn fetch_bibitem_authors(
        &self,
        bibitem_ids: &[i64],
    ) -> Result<Vec<BibitemAuthorsRow>, HexforgeError> {
        if bibitem_ids.is_empty() {
            return Ok(Vec::new());
        }
        query_as::<_, BibitemAuthorsRow>(
            "SELECT j.bibkey, j.author_key, j.role, j.position, j.name_variant_latex, j.name_variant_unicode \
             FROM bibitem_authors j JOIN bibitems m ON m.bibkey = j.bibkey \
             WHERE m.id = ANY($1) ORDER BY j.bibkey, j.role, j.position"
        )
        .bind(bibitem_ids)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)
    }

    async fn fetch_authors_by_keys(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, Author>, HexforgeError> {
        if keys.is_empty() {
            return Ok(HashMap::new());
        }
        let sql = "SELECT * FROM authors WHERE author_key = ANY($1)";
        let authors: Vec<Author> = hexforge::db_exports::query_as(sql)
            .bind(keys)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(authors
            .into_iter()
            .map(|a| (a.author_key.clone(), a))
            .collect())
    }
}

// =============================================================================
// TransitiveDepsResolver
// =============================================================================

pub struct PgTransitiveDepsResolver<'a> {
    pool: &'a PgPool,
}

impl<'a> PgTransitiveDepsResolver<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl TransitiveDepsResolver for PgTransitiveDepsResolver<'_> {
    async fn fetch_further_ref_ids(&self, source_ids: &[i64]) -> Result<Vec<i64>, HexforgeError> {
        if source_ids.is_empty() {
            return Ok(Vec::new());
        }
        #[derive(FromRow)]
        struct DepRow {
            id: i64,
        }
        let rows: Vec<DepRow> =
            query_as("SELECT DISTINCT b.id FROM bibitem_further_refs bfr JOIN bibitems src ON src.bibkey = bfr.source_key JOIN bibitems b ON b.bibkey = bfr.dep_key WHERE src.id = ANY($1)")
                .bind(source_ids)
                .fetch_all(self.pool)
                .await
                .map_err(HexforgeError::data_source)?;
        Ok(rows.into_iter().map(|r| r.id).collect())
    }
}
