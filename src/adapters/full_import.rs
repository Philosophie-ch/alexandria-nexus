//! Postgres implementations of full CSV import/export store traits.
//!
//! These adapters implement the contracts defined in `crate::process::full_import`
//! using raw SQL against PostgreSQL.

use std::collections::{HashMap, HashSet};

use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgPool, query, query_as, query_scalar};

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow, BibitemRefsRow};
use crate::domain::{AuthorRole, BibItem, RefType};
use crate::logic::full_import::{
    AuthorLookupResult, AuthorNameKey, VariantInfo, parse_variant_to_keys,
};
use crate::process::full_import::{
    AuthorLookup, AuthorNameFetcher, BibitemByBibkeyDeleter, BibitemDeleter, BibkeyLookup,
    EntityLookup, FullCsvBibitemFetcher, FullCsvJunctionFetcher, FullImportAuthorJunctionStore,
    FullImportKeywordJunctionStore, FullImportRefStore, KeywordLookup, KeywordNameFetcher,
    ReverseNameMapFetcher,
};

// =============================================================================
// Row types for sqlx (adapter-only)
// =============================================================================

#[derive(FromRow)]
struct AuthorRow {
    id: i64,
    family_name_latex: Option<String>,
    given_name_latex: Option<String>,
    mononym_latex: Option<String>,
    name_variants_latex: Option<Vec<String>>,
    name_variants_unicode: Option<Vec<String>>,
}

#[derive(FromRow)]
struct NameIdRow {
    id: i64,
    name_latex: String,
}

#[derive(FromRow)]
struct KeywordRow {
    id: i64,
    name: String,
    level: i16,
}

#[derive(FromRow)]
struct BibkeyRow {
    bibkey: String,
}

// =============================================================================
// PgFullImportStore — single struct implementing all full import traits
// =============================================================================

/// Postgres implementation of all full CSV import/export store traits.
///
/// Uses raw SQL to perform batch lookups, junction inserts, and entity deletions.
pub struct PgFullImportStore<'a> {
    pool: &'a PgPool,
}

impl<'a> PgFullImportStore<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

// =============================================================================
// AuthorLookup
// =============================================================================

impl AuthorLookup for PgFullImportStore<'_> {
    async fn batch_lookup_authors(&self) -> Result<AuthorLookupResult, HexforgeError> {
        let rows: Vec<AuthorRow> = query_as(
            "SELECT id, family_name_latex, given_name_latex, mononym_latex, name_variants_latex, name_variants_unicode FROM authors",
        )
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;

        let mut id_map: HashMap<AuthorNameKey, Vec<i64>> = HashMap::new();
        let mut variant_map: HashMap<AuthorNameKey, VariantInfo> = HashMap::new();

        for row in &rows {
            // Primary name key
            let key = if let Some(mononym) = &row.mononym_latex {
                AuthorNameKey::Mononym(mononym.clone())
            } else if let Some(family) = &row.family_name_latex {
                AuthorNameKey::Named {
                    family_name: family.clone(),
                    given_name: row.given_name_latex.clone(),
                }
            } else {
                continue;
            };
            id_map.entry(key).or_default().push(row.id);

            // LaTeX name variants
            if let Some(variants) = &row.name_variants_latex {
                for variant in variants {
                    let keys = parse_variant_to_keys(variant);
                    for variant_key in keys {
                        id_map.entry(variant_key.clone()).or_default().push(row.id);
                        variant_map
                            .entry(variant_key)
                            .or_insert_with(|| VariantInfo {
                                variant_latex: Some(variant.clone()),
                                variant_unicode: None,
                            });
                    }
                }
            }

            // Unicode name variants
            if let Some(variants) = &row.name_variants_unicode {
                for variant in variants {
                    let keys = parse_variant_to_keys(variant);
                    for variant_key in keys {
                        id_map.entry(variant_key.clone()).or_default().push(row.id);
                        variant_map
                            .entry(variant_key)
                            .or_insert_with(|| VariantInfo {
                                variant_latex: None,
                                variant_unicode: Some(variant.clone()),
                            });
                    }
                }
            }
        }
        Ok(AuthorLookupResult {
            id_map,
            variant_map,
        })
    }
}

// =============================================================================
// EntityLookup
// =============================================================================

impl EntityLookup for PgFullImportStore<'_> {
    async fn batch_lookup_by_name_latex(
        &self,
        table: &str,
    ) -> Result<HashMap<String, i64>, HexforgeError> {
        let sql = format!("SELECT id, name_latex FROM {table}");
        let rows: Vec<NameIdRow> = query_as(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;

        Ok(rows.into_iter().map(|r| (r.name_latex, r.id)).collect())
    }
}

// =============================================================================
// KeywordLookup
// =============================================================================

impl KeywordLookup for PgFullImportStore<'_> {
    async fn batch_lookup_keywords(&self) -> Result<HashMap<(String, i16), i64>, HexforgeError> {
        let rows: Vec<KeywordRow> = query_as("SELECT id, name, level FROM keywords")
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;

        Ok(rows
            .into_iter()
            .map(|r| ((r.name, r.level), r.id))
            .collect())
    }
}

// =============================================================================
// BibkeyLookup
// =============================================================================

impl BibkeyLookup for PgFullImportStore<'_> {
    async fn fetch_all_bibkeys(&self) -> Result<HashSet<String>, HexforgeError> {
        let rows: Vec<BibkeyRow> = query_as("SELECT bibkey FROM bibitems")
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;

        Ok(rows.into_iter().map(|r| r.bibkey).collect())
    }
}

// =============================================================================
// FullImportAuthorJunctionStore
// =============================================================================

impl FullImportAuthorJunctionStore for PgFullImportStore<'_> {
    async fn insert_author_junction(
        &self,
        bibitem_id: i64,
        author_id: i64,
        role: &AuthorRole,
        position: i16,
        variant_latex: Option<&str>,
        variant_unicode: Option<&str>,
    ) -> Result<(), String> {
        let role_str = role.to_string();
        query(
            "INSERT INTO bibitem_authors (bibitem_id, author_id, role, position, name_variant_latex, name_variant_unicode) \
             VALUES ($1, $2, $3::author_role, $4, $5, $6) \
             ON CONFLICT (bibitem_id, author_id, role) DO UPDATE SET position = $4, name_variant_latex = $5, name_variant_unicode = $6",
        )
        .bind(bibitem_id)
        .bind(author_id)
        .bind(&role_str)
        .bind(position)
        .bind(variant_latex)
        .bind(variant_unicode)
        .execute(self.pool)
        .await
        .map_err(|e| format!("failed to link author: {e}"))?;
        Ok(())
    }
}

// =============================================================================
// FullImportKeywordJunctionStore
// =============================================================================

impl FullImportKeywordJunctionStore for PgFullImportStore<'_> {
    async fn insert_keyword_junction(
        &self,
        bibitem_id: i64,
        keyword_id: i64,
        keyword_level: i16,
    ) -> Result<(), String> {
        query(
            "INSERT INTO bibitem_keywords (bibitem_id, keyword_id, keyword_level) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (bibitem_id, keyword_id) DO NOTHING",
        )
        .bind(bibitem_id)
        .bind(keyword_id)
        .bind(keyword_level)
        .execute(self.pool)
        .await
        .map_err(|e| format!("failed to link keyword: {e}"))?;
        Ok(())
    }
}

// =============================================================================
// FullImportRefStore
// =============================================================================

impl FullImportRefStore for PgFullImportStore<'_> {
    async fn insert_bibitem_ref(
        &self,
        source_id: i64,
        target_bibkey: &str,
        ref_type: &RefType,
    ) -> Result<(), String> {
        let ref_type_str = ref_type.to_string();
        let target_id: Option<i64> = query_scalar("SELECT id FROM bibitems WHERE bibkey = $1")
            .bind(target_bibkey)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| format!("failed to resolve bibkey '{target_bibkey}': {e}"))?;

        let target_id = match target_id {
            Some(id) => id,
            None => return Ok(()), // target not found -- skip silently (same behavior as before)
        };

        query(
            "INSERT INTO bibitem_refs (source_id, target_id, ref_type) \
             VALUES ($1, $2, $3::ref_type) \
             ON CONFLICT (source_id, target_id, ref_type) DO NOTHING",
        )
        .bind(source_id)
        .bind(target_id)
        .bind(&ref_type_str)
        .execute(self.pool)
        .await
        .map_err(|e| format!("failed to insert ref: {e}"))?;
        Ok(())
    }
}

// =============================================================================
// BibitemDeleter
// =============================================================================

impl BibitemDeleter for PgFullImportStore<'_> {
    async fn delete_bibitems_by_bibkeys(&self, bibkeys: &[String]) -> Result<usize, HexforgeError> {
        let result = query("DELETE FROM bibitems WHERE bibkey = ANY($1)")
            .bind(bibkeys)
            .execute(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(usize::try_from(result.rows_affected()).unwrap_or(0))
    }
}

// =============================================================================
// BibitemByBibkeyDeleter
// =============================================================================

impl BibitemByBibkeyDeleter for PgFullImportStore<'_> {
    async fn delete_by_bibkey(&self, bibkey: &str) -> Result<(), String> {
        query("DELETE FROM bibitems WHERE bibkey = $1")
            .bind(bibkey)
            .execute(self.pool)
            .await
            .map_err(|e| format!("failed to delete old bibitem: {e}"))?;
        Ok(())
    }
}

// =============================================================================
// FullCsvBibitemFetcher
// =============================================================================

impl FullCsvBibitemFetcher for PgFullImportStore<'_> {
    async fn fetch_all_bibitems(&self) -> Result<Vec<BibItem>, HexforgeError> {
        query_as("SELECT * FROM bibitems ORDER BY bibkey")
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)
    }
}

// =============================================================================
// AuthorNameFetcher
// =============================================================================

impl AuthorNameFetcher for PgFullImportStore<'_> {
    async fn fetch_author_names(&self) -> Result<HashMap<i64, String>, HexforgeError> {
        let rows: Vec<AuthorRow> = query_as(
            "SELECT id, family_name_latex, given_name_latex, mononym_latex, name_variants_latex, name_variants_unicode FROM authors",
        )
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;

        Ok(rows
            .into_iter()
            .map(|a| {
                let name = if let Some(m) = a.mononym_latex {
                    m
                } else {
                    match (a.family_name_latex, a.given_name_latex) {
                        (Some(f), Some(g)) => format!("{f}, {g}"),
                        (Some(f), None) => f,
                        _ => String::new(),
                    }
                };
                (a.id, name)
            })
            .collect())
    }
}

// =============================================================================
// ReverseNameMapFetcher
// =============================================================================

impl ReverseNameMapFetcher for PgFullImportStore<'_> {
    async fn fetch_reverse_name_map(
        &self,
        table: &str,
    ) -> Result<HashMap<i64, String>, HexforgeError> {
        let sql = format!("SELECT id, name_latex FROM {table}");
        let rows: Vec<NameIdRow> = query_as(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(rows.into_iter().map(|r| (r.id, r.name_latex)).collect())
    }
}

// =============================================================================
// KeywordNameFetcher
// =============================================================================

impl KeywordNameFetcher for PgFullImportStore<'_> {
    async fn fetch_keyword_names(&self) -> Result<HashMap<i64, (String, i16)>, HexforgeError> {
        let rows: Vec<KeywordRow> = query_as("SELECT id, name, level FROM keywords")
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(rows
            .into_iter()
            .map(|k| (k.id, (k.name, k.level)))
            .collect())
    }
}

// =============================================================================
// FullCsvJunctionFetcher
// =============================================================================

impl FullCsvJunctionFetcher for PgFullImportStore<'_> {
    async fn fetch_bibitem_authors_batch(
        &self,
        ids: &[i64],
    ) -> Result<Vec<BibitemAuthorsRow>, HexforgeError> {
        query_as::<_, BibitemAuthorsRow>(
            "SELECT bibitem_id, author_id, role::text as role, position, name_variant_latex, name_variant_unicode \
             FROM bibitem_authors WHERE bibitem_id = ANY($1) ORDER BY bibitem_id, role, position",
        )
        .bind(ids)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)
    }

    async fn fetch_bibitem_keywords_batch(
        &self,
        ids: &[i64],
    ) -> Result<Vec<BibitemKeywordsRow>, HexforgeError> {
        query_as::<_, BibitemKeywordsRow>(
            "SELECT bibitem_id, keyword_id, keyword_level \
             FROM bibitem_keywords WHERE bibitem_id = ANY($1) ORDER BY bibitem_id",
        )
        .bind(ids)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)
    }

    async fn fetch_bibitem_refs_batch(
        &self,
        ids: &[i64],
    ) -> Result<Vec<BibitemRefsRow>, HexforgeError> {
        query_as::<_, BibitemRefsRow>(
            "SELECT source_id, target_id, ref_type::text as ref_type \
             FROM bibitem_refs WHERE source_id = ANY($1) ORDER BY source_id, ref_type",
        )
        .bind(ids)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)
    }
}
