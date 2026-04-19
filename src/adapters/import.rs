//! Postgres implementations of import store traits.
//!
//! These adapters implement the contracts defined in `crate::process::import`
//! using raw SQL against PostgreSQL.

use std::collections::HashSet;

use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgPool, query, query_as, query_scalar};

use crate::domain::AuthorRole;
use crate::logic::import::NameVariantType;
use crate::process::import::{
    BibitemJunctionStore, BibitemNotesStore, BibitemRefsStore, NameVariantStore, ReferenceStore,
    SequenceSyncer,
};

// =============================================================================
// PgNameVariantStore
// =============================================================================

/// Postgres implementation of [`NameVariantStore`].
///
/// Uses `array_append` to atomically add variants to an author's name arrays.
pub struct PgNameVariantStore<'a> {
    pool: &'a PgPool,
}

impl<'a> PgNameVariantStore<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl NameVariantStore for PgNameVariantStore<'_> {
    async fn append_variant(
        &self,
        author_id: i64,
        variant: &str,
        variant_type: &NameVariantType,
    ) -> Result<(), HexforgeError> {
        let column = variant_type.column_name();
        let sql = format!(
            "UPDATE authors SET {column} = array_append(COALESCE({column}, ARRAY[]::TEXT[]), $1) WHERE id = $2"
        );
        query(&sql)
            .bind(variant)
            .bind(author_id)
            .execute(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(())
    }
}

// =============================================================================
// PgSequenceSyncer
// =============================================================================

/// Postgres implementation of [`SequenceSyncer`].
///
/// Advances the sequence for a given table to at least `MAX(id)`, preventing
/// ID collisions after bulk inserts with explicit IDs.
pub struct PgSequenceSyncer<'a> {
    pool: &'a PgPool,
}

impl<'a> PgSequenceSyncer<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl SequenceSyncer for PgSequenceSyncer<'_> {
    async fn sync_sequence(&self, table: &'static str) -> Result<(), HexforgeError> {
        let sql = match table {
            "authors" => {
                "SELECT setval(pg_get_serial_sequence('authors', 'id'), COALESCE(MAX(id), 1)) FROM authors"
            }
            "journals" => {
                "SELECT setval(pg_get_serial_sequence('journals', 'id'), COALESCE(MAX(id), 1)) FROM journals"
            }
            "publishers" => {
                "SELECT setval(pg_get_serial_sequence('publishers', 'id'), COALESCE(MAX(id), 1)) FROM publishers"
            }
            "institutions" => {
                "SELECT setval(pg_get_serial_sequence('institutions', 'id'), COALESCE(MAX(id), 1)) FROM institutions"
            }
            "schools" => {
                "SELECT setval(pg_get_serial_sequence('schools', 'id'), COALESCE(MAX(id), 1)) FROM schools"
            }
            "series" => {
                "SELECT setval(pg_get_serial_sequence('series', 'id'), COALESCE(MAX(id), 1)) FROM series"
            }
            "keywords" => {
                "SELECT setval(pg_get_serial_sequence('keywords', 'id'), COALESCE(MAX(id), 1)) FROM keywords"
            }
            "bibitems" => {
                "SELECT setval(pg_get_serial_sequence('bibitems', 'id'), COALESCE(MAX(id), 1)) FROM bibitems"
            }
            _ => {
                return Err(HexforgeError::internal(format!(
                    "Unknown table for sequence sync: {table}"
                )));
            }
        };
        query(sql)
            .execute(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(())
    }
}

// =============================================================================
// PgBibitemJunctionStore
// =============================================================================

/// Postgres implementation of [`BibitemJunctionStore`].
///
/// Inserts junction records for bibitem-author and bibitem-keyword relationships.
pub struct PgBibitemJunctionStore<'a> {
    pool: &'a PgPool,
}

impl<'a> PgBibitemJunctionStore<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl BibitemJunctionStore for PgBibitemJunctionStore<'_> {
    async fn insert_author_junction(
        &self,
        bibitem_id: i64,
        author_id: i64,
        role: &AuthorRole,
        position: i16,
    ) -> Result<(), HexforgeError> {
        let role_str = role.to_string();
        query(
            r#"
            INSERT INTO bibitem_authors (bibitem_id, author_id, role, position)
            VALUES ($1, $2, $3::author_role, $4)
            ON CONFLICT (bibitem_id, author_id, role) DO UPDATE SET position = $4
            "#,
        )
        .bind(bibitem_id)
        .bind(author_id)
        .bind(&role_str)
        .bind(position)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;
        Ok(())
    }

    async fn insert_keyword_junction(
        &self,
        bibitem_id: i64,
        keyword_id: i64,
        keyword_level: i16,
    ) -> Result<(), HexforgeError> {
        query(
            "INSERT INTO bibitem_keywords (bibitem_id, keyword_id, keyword_level) VALUES ($1, $2, $3) ON CONFLICT (bibitem_id, keyword_id) DO NOTHING",
        )
        .bind(bibitem_id)
        .bind(keyword_id)
        .bind(keyword_level)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;
        Ok(())
    }

    async fn find_keyword_levels(
        &self,
        keyword_ids: &[i64],
    ) -> Result<Vec<(i64, i16)>, HexforgeError> {
        #[derive(Debug, FromRow)]
        struct KeywordLevelRow {
            id: i64,
            level: i16,
        }

        let rows: Vec<KeywordLevelRow> =
            query_as("SELECT id, level FROM keywords WHERE id = ANY($1)")
                .bind(keyword_ids)
                .fetch_all(self.pool)
                .await
                .map_err(HexforgeError::data_source)?;

        Ok(rows.into_iter().map(|r| (r.id, r.level)).collect())
    }
}

// =============================================================================
// PgReferenceStore
// =============================================================================

/// Postgres implementation of [`ReferenceStore`].
///
/// Checks for missing entity IDs across all referenced tables.
pub struct PgReferenceStore<'a> {
    pool: &'a PgPool,
}

impl<'a> PgReferenceStore<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl PgReferenceStore<'_> {
    /// Find IDs from the given set that don't exist in the specified table.
    async fn find_missing_in_table(
        &self,
        table: &str,
        ids: &HashSet<i64>,
    ) -> Result<Vec<i64>, HexforgeError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let id_vec: Vec<i64> = ids.iter().copied().collect();

        let sql = format!("SELECT id FROM {table} WHERE id = ANY($1)");
        let found_ids: Vec<i64> = query_scalar(&sql)
            .bind(&id_vec)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;

        let found_set: HashSet<i64> = found_ids.into_iter().collect();
        let mut missing: Vec<i64> = id_vec
            .into_iter()
            .filter(|id| !found_set.contains(id))
            .collect();
        missing.sort_unstable();
        Ok(missing)
    }
}

impl ReferenceStore for PgReferenceStore<'_> {
    async fn find_missing_author_ids(&self, ids: &HashSet<i64>) -> Result<Vec<i64>, HexforgeError> {
        self.find_missing_in_table("authors", ids).await
    }

    async fn find_missing_journal_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> Result<Vec<i64>, HexforgeError> {
        self.find_missing_in_table("journals", ids).await
    }

    async fn find_missing_publisher_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> Result<Vec<i64>, HexforgeError> {
        self.find_missing_in_table("publishers", ids).await
    }

    async fn find_missing_institution_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> Result<Vec<i64>, HexforgeError> {
        self.find_missing_in_table("institutions", ids).await
    }

    async fn find_missing_school_ids(&self, ids: &HashSet<i64>) -> Result<Vec<i64>, HexforgeError> {
        self.find_missing_in_table("schools", ids).await
    }

    async fn find_missing_series_ids(&self, ids: &HashSet<i64>) -> Result<Vec<i64>, HexforgeError> {
        self.find_missing_in_table("series", ids).await
    }

    async fn find_missing_keyword_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> Result<Vec<i64>, HexforgeError> {
        self.find_missing_in_table("keywords", ids).await
    }

    async fn find_missing_bibitem_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> Result<Vec<i64>, HexforgeError> {
        self.find_missing_in_table("bibitems", ids).await
    }
}

// =============================================================================
// PgBibitemRefsStore
// =============================================================================

pub struct PgBibitemRefsStore<'a> {
    pool: &'a PgPool,
}

impl<'a> PgBibitemRefsStore<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl BibitemRefsStore for PgBibitemRefsStore<'_> {
    async fn insert_bibitem_ref(
        &self,
        source_id: i64,
        target_id: i64,
        ref_type: &str,
    ) -> Result<(), HexforgeError> {
        query(
            "INSERT INTO bibitem_refs (source_id, target_id, ref_type) \
             VALUES ($1, $2, $3::ref_type) ON CONFLICT DO NOTHING",
        )
        .bind(source_id)
        .bind(target_id)
        .bind(ref_type)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;
        Ok(())
    }
}

// =============================================================================
// PgBibitemNotesStore
// =============================================================================

pub struct PgBibitemNotesStore<'a> {
    pool: &'a PgPool,
}

impl<'a> PgBibitemNotesStore<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl BibitemNotesStore for PgBibitemNotesStore<'_> {
    async fn upsert_bibitem_notes(
        &self,
        bibitem_id: i64,
        notes: &serde_json::Value,
    ) -> Result<(), HexforgeError> {
        query(
            "INSERT INTO bibitem_notes (bibitem_id, notes) VALUES ($1, $2) \
             ON CONFLICT (bibitem_id) DO UPDATE SET notes = EXCLUDED.notes, updated_at = NOW()",
        )
        .bind(bibitem_id)
        .bind(notes)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;
        Ok(())
    }
}
