//! Postgres implementations of export store traits.
//!
//! These adapters implement the contracts defined in `crate::process::export`
//! using DataStore methods and raw SQL against PostgreSQL.

use std::collections::{HashMap, HashSet};

use hexforge::db_exports::{PgPool, query_as};
use hexforge::{DataStore, HexforgeError, WhereClause};

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow};
use crate::domain::{Author, BibItem, Institution, Journal, Keyword, Publisher, School, Series};
use crate::logic::export::ExportError;
use crate::process::export::{
    BibitemFetcher, EntityBatchFetcher, ExportJunctionFetcher, KeyedEntityFetcher,
};

// =============================================================================
// Generic keyed entity fetcher for DataStore-backed entities
// =============================================================================

/// Trait to extract entity id and key for generic entity operations.
trait EntityWithKey {
    fn entity_id(&self) -> i64;
    fn key_value(&self) -> &str;
}

impl EntityWithKey for Author {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.author_key
    }
}

impl EntityWithKey for Journal {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.journal_key
    }
}

impl EntityWithKey for Publisher {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.publisher_key
    }
}

impl EntityWithKey for Institution {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.institution_key
    }
}

impl EntityWithKey for School {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.school_key
    }
}

impl EntityWithKey for Series {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.series_key
    }
}

/// Trait to extract the id for batch fetching into a HashMap.
trait EntityWithId {
    fn entity_id(&self) -> i64;
}

impl EntityWithId for Author {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Journal {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Publisher {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Institution {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for School {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Series {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Keyword {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for BibItem {
    fn entity_id(&self) -> i64 {
        self.id
    }
}

// =============================================================================
// PgKeyedEntityFetcher — keyed entity export via DataStore
// =============================================================================

/// Postgres implementation of [`KeyedEntityFetcher`] for entities with a unique key column.
pub struct PgKeyedEntityFetcher<'a, T: hexforge::PgEntity, Q> {
    ds: &'a DataStore<T, Q>,
    key_column: &'static str,
    table: &'static str,
    pool: &'a PgPool,
}

impl<'a, T: hexforge::PgEntity, Q> PgKeyedEntityFetcher<'a, T, Q> {
    pub fn new(
        ds: &'a DataStore<T, Q>,
        key_column: &'static str,
        table: &'static str,
        pool: &'a PgPool,
    ) -> Self {
        Self {
            ds,
            key_column,
            table,
            pool,
        }
    }
}

impl<T, Q> KeyedEntityFetcher<T> for PgKeyedEntityFetcher<'_, T, Q>
where
    T: hexforge::PgEntity
        + Clone
        + EntityWithKey
        + Send
        + Sync
        + Unpin
        + for<'r> sqlx::FromRow<'r, hexforge::db_exports::PgRow>,
    Q: hexforge::PgQuery + 'static,
{
    async fn fetch_all(&self) -> Result<Vec<T>, ExportError> {
        let sql = format!("SELECT * FROM {} ORDER BY id", self.table);
        query_as::<_, T>(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(|e| ExportError::Internal(HexforgeError::data_source(e)))
    }

    async fn fetch_by_ids(&self, ids: &[i64]) -> Result<Vec<T>, ExportError> {
        let found = self
            .ds
            .find_by_ids(ids)
            .await
            .map_err(HexforgeError::data_source)?;
        let found_ids: HashSet<i64> = found.iter().map(|e| e.entity_id()).collect();
        let missing: Vec<i64> = ids
            .iter()
            .filter(|id| !found_ids.contains(id))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(ExportError::MissingIds(missing));
        }
        Ok(found)
    }

    async fn fetch_by_keys(&self, keys: &[String]) -> Result<Vec<T>, ExportError> {
        let key_column = self.key_column;
        let mut all_found = Vec::new();
        for key in keys {
            let clause = WhereClause::new(format!("{key_column} = $1")).bind(key.clone());
            let found = self
                .ds
                .find_one(clause)
                .await
                .map_err(HexforgeError::data_source)?;
            if let Some(entity) = found {
                all_found.push(entity);
            }
        }
        let found_keys: HashSet<&str> = all_found.iter().map(|e| e.key_value()).collect();
        let missing: Vec<String> = keys
            .iter()
            .filter(|k| !found_keys.contains(k.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(ExportError::MissingKeys(missing));
        }
        Ok(all_found)
    }
}

// =============================================================================
// PgKeywordFetcher — keyword export (no unique key column)
// =============================================================================

/// Postgres implementation of [`KeyedEntityFetcher`] for keywords.
///
/// Keywords don't have a unique "key" column — identity is (name, level).
/// We treat `keys` as keyword names for filtering purposes.
pub struct PgKeywordFetcher<'a, Q> {
    ds: &'a DataStore<Keyword, Q>,
    pool: &'a PgPool,
}

impl<'a, Q> PgKeywordFetcher<'a, Q> {
    pub fn new(ds: &'a DataStore<Keyword, Q>, pool: &'a PgPool) -> Self {
        Self { ds, pool }
    }
}

impl<Q> KeyedEntityFetcher<Keyword> for PgKeywordFetcher<'_, Q>
where
    Q: hexforge::PgQuery + 'static,
{
    async fn fetch_all(&self) -> Result<Vec<Keyword>, ExportError> {
        let sql = "SELECT * FROM keywords ORDER BY id";
        query_as::<_, Keyword>(sql)
            .fetch_all(self.pool)
            .await
            .map_err(|e| ExportError::Internal(HexforgeError::data_source(e)))
    }

    async fn fetch_by_ids(&self, ids: &[i64]) -> Result<Vec<Keyword>, ExportError> {
        let found = self
            .ds
            .find_by_ids(ids)
            .await
            .map_err(HexforgeError::data_source)?;
        let found_ids: HashSet<i64> = found.iter().map(|k| k.id).collect();
        let missing: Vec<i64> = ids
            .iter()
            .filter(|id| !found_ids.contains(id))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(ExportError::MissingIds(missing));
        }
        Ok(found)
    }

    async fn fetch_by_keys(&self, keys: &[String]) -> Result<Vec<Keyword>, ExportError> {
        let mut all_found = Vec::new();
        for name in keys {
            let found: Vec<Keyword> = self
                .ds
                .find_many(WhereClause::new("name = $1").bind(name.clone()))
                .await
                .map_err(HexforgeError::data_source)?;
            all_found.extend(found);
        }
        let found_names: HashSet<&str> = all_found.iter().map(|k| k.name.as_str()).collect();
        let missing: Vec<String> = keys
            .iter()
            .filter(|k| !found_names.contains(k.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(ExportError::MissingKeys(missing));
        }
        Ok(all_found)
    }
}

// =============================================================================
// PgEntityBatchFetcher — batch fetch by IDs into HashMap
// =============================================================================

/// Postgres implementation of [`EntityBatchFetcher`].
pub struct PgEntityBatchFetcher<'a, T: hexforge::PgEntity, Q> {
    ds: &'a DataStore<T, Q>,
}

impl<'a, T: hexforge::PgEntity, Q> PgEntityBatchFetcher<'a, T, Q> {
    pub fn new(ds: &'a DataStore<T, Q>) -> Self {
        Self { ds }
    }
}

impl<T, Q> EntityBatchFetcher<T> for PgEntityBatchFetcher<'_, T, Q>
where
    T: hexforge::PgEntity + Clone + EntityWithId + Send + Sync + Unpin,
    Q: hexforge::PgQuery + 'static,
{
    async fn fetch_map(&self, ids: &HashSet<i64>) -> Result<HashMap<i64, T>, ExportError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let id_vec: Vec<i64> = ids.iter().copied().collect();
        let entities = self
            .ds
            .find_by_ids(&id_vec)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(entities.into_iter().map(|e| (e.entity_id(), e)).collect())
    }
}

// =============================================================================
// PgBibitemFetcher — bibitem export fetching
// =============================================================================

/// Postgres implementation of [`BibitemFetcher`].
pub struct PgBibitemFetcher<'a, Q> {
    ds: &'a DataStore<BibItem, Q>,
    pool: &'a PgPool,
}

impl<'a, Q> PgBibitemFetcher<'a, Q> {
    pub fn new(ds: &'a DataStore<BibItem, Q>, pool: &'a PgPool) -> Self {
        Self { ds, pool }
    }
}

impl<Q> BibitemFetcher for PgBibitemFetcher<'_, Q>
where
    Q: hexforge::PgQuery + 'static,
{
    async fn fetch_all(&self) -> Result<Vec<BibItem>, ExportError> {
        let sql = "SELECT * FROM bibitems ORDER BY id";
        query_as::<_, BibItem>(sql)
            .fetch_all(self.pool)
            .await
            .map_err(|e| ExportError::Internal(HexforgeError::data_source(e)))
    }

    async fn fetch_by_ids(&self, ids: &[i64]) -> Result<Vec<BibItem>, ExportError> {
        let found = self
            .ds
            .find_by_ids(ids)
            .await
            .map_err(HexforgeError::data_source)?;
        let found_ids: HashSet<i64> = found.iter().map(|b| b.id).collect();
        let missing: Vec<i64> = ids
            .iter()
            .filter(|id| !found_ids.contains(id))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(ExportError::MissingIds(missing));
        }
        Ok(found)
    }

    async fn fetch_by_bibkeys(&self, bibkeys: &[String]) -> Result<Vec<BibItem>, ExportError> {
        let mut all_found = Vec::new();
        for bibkey in bibkeys {
            let found = self
                .ds
                .find_one(WhereClause::new("bibkey = $1").bind(bibkey.clone()))
                .await
                .map_err(HexforgeError::data_source)?;
            if let Some(item) = found {
                all_found.push(item);
            }
        }
        let found_keys: HashSet<&str> = all_found.iter().map(|b| b.bibkey.as_str()).collect();
        let missing: Vec<String> = bibkeys
            .iter()
            .filter(|k| !found_keys.contains(k.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(ExportError::MissingBibkeys(missing));
        }
        Ok(all_found)
    }
}

// =============================================================================
// PgExportJunctionFetcher — junction batch-fetch
// =============================================================================

/// Postgres implementation of [`ExportJunctionFetcher`].
pub struct PgExportJunctionFetcher<'a> {
    pool: &'a PgPool,
}

impl<'a> PgExportJunctionFetcher<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl ExportJunctionFetcher for PgExportJunctionFetcher<'_> {
    async fn fetch_bibitem_authors_batch(
        &self,
        bibitem_ids: &[i64],
    ) -> Result<Vec<BibitemAuthorsRow>, HexforgeError> {
        query_as::<_, BibitemAuthorsRow>(
            "SELECT bibitem_id, author_id, role::text as role, position, name_variant_latex, name_variant_unicode FROM bibitem_authors WHERE bibitem_id = ANY($1) ORDER BY bibitem_id, role, position"
        )
        .bind(bibitem_ids)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)
    }

    async fn fetch_bibitem_keywords_batch(
        &self,
        bibitem_ids: &[i64],
    ) -> Result<Vec<BibitemKeywordsRow>, HexforgeError> {
        query_as::<_, BibitemKeywordsRow>(
            "SELECT bibitem_id, keyword_id, keyword_level FROM bibitem_keywords WHERE bibitem_id = ANY($1) ORDER BY bibitem_id"
        )
        .bind(bibitem_ids)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)
    }
}
