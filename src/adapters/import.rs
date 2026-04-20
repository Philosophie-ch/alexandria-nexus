//! Postgres implementations of import store traits, plus CSV parsing functions.
//!
//! These adapters implement the contracts defined in `crate::process::import`
//! using raw SQL against PostgreSQL.
//!
//! CSV parsing (wire format) lives here, not in the logic layer.

use std::collections::HashSet;

use hexforge::db_exports::{FromRow, PgPool, query, query_as, query_scalar};
use hexforge::{DataStore, HexforgeError, ValidationError};

use crate::domain::{AuthorRole, CreateBibItem, EntryType};
use crate::logic::import::{
    ImportRowError, NameVariantType, ParsedAuthorRow, ParsedBibitemNotesRow, ParsedBibitemRefRow,
    ParsedBibitemRow, ParsedInstitutionRow, ParsedJournalRow, ParsedKeywordRow,
    ParsedNameVariantRow, ParsedPublisherRow, ParsedSchoolRow, ParsedSeriesRow,
};
use crate::process::import::{
    BibitemJunctionStore, BibitemNotesData, BibitemNotesStore, BibitemRefsStore, EntityBatchLookup,
    NameVariantStore, ReferenceStore, SequenceSyncer,
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
        notes: &BibitemNotesData<'_>,
    ) -> Result<(), HexforgeError> {
        query(
            "INSERT INTO bibitem_notes \
             (bibitem_id, note_perso, note_stock, note_missing, change_request, dltc_copyediting_note, todo_general) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (bibitem_id) DO UPDATE SET \
             note_perso = EXCLUDED.note_perso, \
             note_stock = EXCLUDED.note_stock, \
             note_missing = EXCLUDED.note_missing, \
             change_request = EXCLUDED.change_request, \
             dltc_copyediting_note = EXCLUDED.dltc_copyediting_note, \
             todo_general = EXCLUDED.todo_general, \
             updated_at = NOW()",
        )
        .bind(bibitem_id)
        .bind(notes.note_perso)
        .bind(notes.note_stock)
        .bind(notes.note_missing)
        .bind(notes.change_request)
        .bind(notes.dltc_copyediting_note)
        .bind(notes.todo_general)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;
        Ok(())
    }
}

// =============================================================================
// PgEntityBatchLookup — batch fetch entities by IDs for import
// =============================================================================

/// Postgres implementation of [`EntityBatchLookup`].
pub struct PgEntityBatchLookup<'a, T: hexforge::PgEntity, Q> {
    ds: &'a DataStore<T, Q>,
}

impl<'a, T: hexforge::PgEntity, Q> PgEntityBatchLookup<'a, T, Q> {
    pub fn new(ds: &'a DataStore<T, Q>) -> Self {
        Self { ds }
    }
}

impl<T, Q> EntityBatchLookup<T> for PgEntityBatchLookup<'_, T, Q>
where
    T: hexforge::PgEntity + Clone + Send + Sync + Unpin,
    Q: hexforge::PgQuery + 'static,
{
    async fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<T>, HexforgeError> {
        self.ds
            .find_by_ids(ids)
            .await
            .map_err(HexforgeError::data_source)
    }
}

// =============================================================================
// CSV field helpers (adapter-only — depend on csv::StringRecord)
// =============================================================================

fn get_field(record: &csv::StringRecord, idx: usize) -> Option<String> {
    record
        .get(idx)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_i16_field(record: &csv::StringRecord, idx: usize) -> Option<i16> {
    get_field(record, idx).and_then(|s| s.parse().ok())
}

fn parse_i64_field(record: &csv::StringRecord, idx: usize) -> Option<i64> {
    get_field(record, idx).and_then(|s| s.parse().ok())
}

fn parse_id_list(record: &csv::StringRecord, idx: usize) -> Vec<i64> {
    get_field(record, idx)
        .map(|s| {
            s.split(',')
                .filter_map(|part| part.trim().parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn column_index(headers: &csv::StringRecord, name: &str) -> Option<usize> {
    headers.iter().position(|h| h.trim() == name)
}

fn require_column(headers: &csv::StringRecord, name: &str) -> Result<usize, HexforgeError> {
    column_index(headers, name).ok_or_else(|| {
        HexforgeError::Validation(ValidationError::custom(format!(
            "Missing required column: {name}"
        )))
    })
}

fn csv_reader(data: &[u8]) -> csv::Reader<std::io::Cursor<&[u8]>> {
    csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data))
}

fn parse_csv_headers(
    reader: &mut csv::Reader<std::io::Cursor<&[u8]>>,
) -> Result<csv::StringRecord, HexforgeError> {
    reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))
        .cloned()
}

// =============================================================================
// CSV parse functions
// =============================================================================

pub fn parse_authors_csv(
    data: &[u8],
) -> Result<(Vec<ParsedAuthorRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_id = column_index(&headers, "id");
    let col_author_key = require_column(&headers, "author_key")?;
    let col_given_name_latex = column_index(&headers, "given_name_latex");
    let col_given_name_unicode = column_index(&headers, "given_name_unicode");
    let col_family_name_latex = column_index(&headers, "family_name_latex");
    let col_family_name_unicode = column_index(&headers, "family_name_unicode");
    let col_mononym_latex = column_index(&headers, "mononym_latex");
    let col_mononym_unicode = column_index(&headers, "mononym_unicode");
    let col_shorthand_latex = column_index(&headers, "shorthand_latex");
    let col_shorthand_unicode = column_index(&headers, "shorthand_unicode");
    let col_famous_name_latex = column_index(&headers, "famous_name_latex");
    let col_famous_name_unicode = column_index(&headers, "famous_name_unicode");
    let col_name_variants_latex = column_index(&headers, "name_variants_latex");
    let col_name_variants_unicode = column_index(&headers, "name_variants_unicode");

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let author_key = match get_field(&record, col_author_key) {
            Some(k) => k,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing author_key".to_string(),
                });
                continue;
            }
        };

        let parse_variants = |col: Option<usize>| -> Option<Vec<String>> {
            col.and_then(|i| {
                get_field(&record, i).map(|s| {
                    s.split(';')
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                        .collect()
                })
            })
        };

        rows.push(ParsedAuthorRow {
            row_num,
            source_id: col_id.and_then(|i| parse_i64_field(&record, i)),
            author_key,
            given_name_latex: col_given_name_latex.and_then(|i| get_field(&record, i)),
            given_name_unicode: col_given_name_unicode.and_then(|i| get_field(&record, i)),
            family_name_latex: col_family_name_latex.and_then(|i| get_field(&record, i)),
            family_name_unicode: col_family_name_unicode.and_then(|i| get_field(&record, i)),
            mononym_latex: col_mononym_latex.and_then(|i| get_field(&record, i)),
            mononym_unicode: col_mononym_unicode.and_then(|i| get_field(&record, i)),
            shorthand_latex: col_shorthand_latex.and_then(|i| get_field(&record, i)),
            shorthand_unicode: col_shorthand_unicode.and_then(|i| get_field(&record, i)),
            famous_name_latex: col_famous_name_latex.and_then(|i| get_field(&record, i)),
            famous_name_unicode: col_famous_name_unicode.and_then(|i| get_field(&record, i)),
            name_variants_latex: parse_variants(col_name_variants_latex),
            name_variants_unicode: parse_variants(col_name_variants_unicode),
        });
    }

    Ok((rows, errors))
}

pub fn parse_journals_csv(
    data: &[u8],
) -> Result<(Vec<ParsedJournalRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_id = column_index(&headers, "id");
    let col_journal_key = require_column(&headers, "journal_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_issn_print = column_index(&headers, "issn_print");
    let col_issn_electronic = column_index(&headers, "issn_electronic");

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let journal_key = match get_field(&record, col_journal_key) {
            Some(k) => k,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing journal_key".to_string(),
                });
                continue;
            }
        };

        rows.push(ParsedJournalRow {
            row_num,
            source_id: col_id.and_then(|i| parse_i64_field(&record, i)),
            journal_key,
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
            issn_print: col_issn_print.and_then(|i| get_field(&record, i)),
            issn_electronic: col_issn_electronic.and_then(|i| get_field(&record, i)),
        });
    }

    Ok((rows, errors))
}

pub fn parse_publishers_csv(
    data: &[u8],
) -> Result<(Vec<ParsedPublisherRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_id = column_index(&headers, "id");
    let col_publisher_key = require_column(&headers, "publisher_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_default_address = column_index(&headers, "default_address");

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let publisher_key = match get_field(&record, col_publisher_key) {
            Some(k) => k,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing publisher_key".to_string(),
                });
                continue;
            }
        };

        rows.push(ParsedPublisherRow {
            row_num,
            source_id: col_id.and_then(|i| parse_i64_field(&record, i)),
            publisher_key,
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
            default_address: col_default_address.and_then(|i| get_field(&record, i)),
        });
    }

    Ok((rows, errors))
}

pub fn parse_institutions_csv(
    data: &[u8],
) -> Result<(Vec<ParsedInstitutionRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_id = column_index(&headers, "id");
    let col_institution_key = require_column(&headers, "institution_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_default_address = column_index(&headers, "default_address");

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let institution_key = match get_field(&record, col_institution_key) {
            Some(k) => k,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing institution_key".to_string(),
                });
                continue;
            }
        };

        rows.push(ParsedInstitutionRow {
            row_num,
            source_id: col_id.and_then(|i| parse_i64_field(&record, i)),
            institution_key,
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
            default_address: col_default_address.and_then(|i| get_field(&record, i)),
        });
    }

    Ok((rows, errors))
}

pub fn parse_schools_csv(
    data: &[u8],
) -> Result<(Vec<ParsedSchoolRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_id = column_index(&headers, "id");
    let col_school_key = require_column(&headers, "school_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let school_key = match get_field(&record, col_school_key) {
            Some(k) => k,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing school_key".to_string(),
                });
                continue;
            }
        };

        rows.push(ParsedSchoolRow {
            row_num,
            source_id: col_id.and_then(|i| parse_i64_field(&record, i)),
            school_key,
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
        });
    }

    Ok((rows, errors))
}

pub fn parse_series_csv(
    data: &[u8],
) -> Result<(Vec<ParsedSeriesRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_id = column_index(&headers, "id");
    let col_series_key = require_column(&headers, "series_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let series_key = match get_field(&record, col_series_key) {
            Some(k) => k,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing series_key".to_string(),
                });
                continue;
            }
        };

        rows.push(ParsedSeriesRow {
            row_num,
            source_id: col_id.and_then(|i| parse_i64_field(&record, i)),
            series_key,
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
        });
    }

    Ok((rows, errors))
}

pub fn parse_keywords_csv(
    data: &[u8],
) -> Result<(Vec<ParsedKeywordRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_id = column_index(&headers, "id");
    let col_name = require_column(&headers, "name")?;
    let col_level = require_column(&headers, "level")?;

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let name = match get_field(&record, col_name) {
            Some(n) => n,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing name".to_string(),
                });
                continue;
            }
        };

        let level = match parse_i16_field(&record, col_level) {
            Some(l) => l,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: name,
                    error: "Missing or invalid level".to_string(),
                });
                continue;
            }
        };

        rows.push(ParsedKeywordRow {
            row_num,
            source_id: col_id.and_then(|i| parse_i64_field(&record, i)),
            name,
            level,
        });
    }

    Ok((rows, errors))
}

pub fn parse_name_variants_csv(
    data: &[u8],
) -> Result<(Vec<ParsedNameVariantRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_name_variant = require_column(&headers, "name_variant")?;
    let col_type = require_column(&headers, "type")?;
    let col_profile_id = require_column(&headers, "profile_id")?;

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let variant = match get_field(&record, col_name_variant) {
            Some(v) => v,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing name_variant".to_string(),
                });
                continue;
            }
        };

        let variant_type = match get_field(&record, col_type) {
            Some(t) => match NameVariantType::parse(&t) {
                Some(vt) => vt,
                None => {
                    errors.push(ImportRowError {
                        row: row_num,
                        identifier: variant,
                        error: format!("Invalid type '{t}', expected 'latex' or 'unicode'"),
                    });
                    continue;
                }
            },
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: variant,
                    error: "Missing type".to_string(),
                });
                continue;
            }
        };

        let profile_id = match parse_i64_field(&record, col_profile_id) {
            Some(id) => id,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: variant,
                    error: "Missing or invalid profile_id".to_string(),
                });
                continue;
            }
        };

        rows.push(ParsedNameVariantRow {
            row_num,
            profile_id,
            variant_type,
            variant,
        });
    }

    Ok((rows, errors))
}

pub fn parse_bibitems_csv(
    data: &[u8],
) -> Result<(Vec<ParsedBibitemRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_id = column_index(&headers, "id");
    let col_entry_type = require_column(&headers, "entry_type")?;
    let col_bibkey = require_column(&headers, "bibkey")?;
    let col_author_ids = column_index(&headers, "author_ids");
    let col_editor_ids = column_index(&headers, "editor_ids");
    let col_guesteditor_ids = column_index(&headers, "guesteditor_ids");
    let col_options = column_index(&headers, "options");
    let col_shorthand = column_index(&headers, "shorthand");
    let col_date_year = column_index(&headers, "date_year");
    let col_date_month = column_index(&headers, "date_month");
    let col_date_day = column_index(&headers, "date_day");
    let col_pubstate = column_index(&headers, "pubstate");
    let col_title_latex = column_index(&headers, "title_latex");
    let col_title_unicode = column_index(&headers, "title_unicode");
    let col_booktitle_latex = column_index(&headers, "booktitle_latex");
    let col_booktitle_unicode = column_index(&headers, "booktitle_unicode");
    let col_crossref_id = column_index(&headers, "crossref_id");
    let col_journal_id = column_index(&headers, "journal_id");
    let col_volume = column_index(&headers, "volume");
    let col_number = column_index(&headers, "number");
    let col_pages = column_index(&headers, "pages");
    let col_eid = column_index(&headers, "eid");
    let col_series_id = column_index(&headers, "series_id");
    let col_address = column_index(&headers, "address");
    let col_institution_id = column_index(&headers, "institution_id");
    let col_school_id = column_index(&headers, "school_id");
    let col_publisher_id = column_index(&headers, "publisher_id");
    let col_type_field = column_index(&headers, "type_field");
    let col_edition = column_index(&headers, "edition");
    let col_note_latex = column_index(&headers, "note_latex");
    let col_note_unicode = column_index(&headers, "note_unicode");
    let col_issuetitle_latex = column_index(&headers, "issuetitle_latex");
    let col_issuetitle_unicode = column_index(&headers, "issuetitle_unicode");
    let col_extra_note_latex = column_index(&headers, "extra_note_latex");
    let col_extra_note_unicode = column_index(&headers, "extra_note_unicode");
    let col_urn = column_index(&headers, "urn");
    let col_eprint = column_index(&headers, "eprint");
    let col_doi = column_index(&headers, "doi");
    let col_url = column_index(&headers, "url");
    let col_keyword_ids = column_index(&headers, "keyword_ids");
    let col_epoch = column_index(&headers, "epoch");
    let col_langid = column_index(&headers, "langid");
    let col_is_translation = column_index(&headers, "is_translation");

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let source_id = col_id.and_then(|i| parse_i64_field(&record, i));

        let bibkey = match get_field(&record, col_bibkey) {
            Some(k) => k,
            None => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing bibkey".to_string(),
                });
                continue;
            }
        };

        let entry_type_str = get_field(&record, col_entry_type).unwrap_or_default();
        let entry_type: EntryType = match entry_type_str.parse() {
            Ok(et) => et,
            Err(_) if entry_type_str.is_empty() => EntryType::Unknown,
            Err(_) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: bibkey,
                    error: format!("Invalid entry_type: '{entry_type_str}'"),
                });
                continue;
            }
        };

        let is_translation = match col_is_translation.and_then(|i| get_field(&record, i)) {
            None => false,
            Some(raw) => match raw.to_lowercase().as_str() {
                "true" | "1" | "yes" | "y" | "x" => true,
                "false" | "0" | "no" | "n" => false,
                _ => {
                    errors.push(ImportRowError {
                        row: row_num,
                        identifier: bibkey,
                        error: format!(
                            "Invalid is_translation value: '{raw}' (expected true/false/yes/no/1/0)"
                        ),
                    });
                    continue;
                }
            },
        };

        let title_latex = col_title_latex
            .and_then(|i| get_field(&record, i))
            .unwrap_or_default();
        let title_unicode = col_title_unicode
            .and_then(|i| get_field(&record, i))
            .unwrap_or_else(|| title_latex.clone());

        let dto = CreateBibItem {
            bibkey: bibkey.clone(),
            entry_type,
            date_year: col_date_year.and_then(|i| parse_i16_field(&record, i)),
            date_year_2_hyphen: None,
            date_year_2_slash: None,
            date_month: col_date_month.and_then(|i| parse_i16_field(&record, i)),
            date_day: col_date_day.and_then(|i| parse_i16_field(&record, i)),
            date_is_no_date: false,
            pubstate: col_pubstate
                .and_then(|i| get_field(&record, i))
                .and_then(|s| s.parse().ok()),
            title_latex,
            title_unicode,
            booktitle_latex: col_booktitle_latex.and_then(|i| get_field(&record, i)),
            booktitle_unicode: col_booktitle_unicode.and_then(|i| get_field(&record, i)),
            journal_id: col_journal_id.and_then(|i| parse_i64_field(&record, i)),
            publisher_id: col_publisher_id.and_then(|i| parse_i64_field(&record, i)),
            address: col_address.and_then(|i| get_field(&record, i)),
            volume: col_volume.and_then(|i| get_field(&record, i)),
            number: col_number.and_then(|i| get_field(&record, i)),
            pages: col_pages.and_then(|i| get_field(&record, i)),
            eid: col_eid.and_then(|i| get_field(&record, i)),
            series_id: col_series_id.and_then(|i| parse_i64_field(&record, i)),
            edition: col_edition.and_then(|i| get_field(&record, i)),
            institution_id: col_institution_id.and_then(|i| parse_i64_field(&record, i)),
            school_id: col_school_id.and_then(|i| parse_i64_field(&record, i)),
            type_field: col_type_field.and_then(|i| get_field(&record, i)),
            doi: col_doi.and_then(|i| get_field(&record, i)),
            url: col_url.and_then(|i| get_field(&record, i)),
            eprint: col_eprint.and_then(|i| get_field(&record, i)),
            urn: col_urn.and_then(|i| get_field(&record, i)),
            crossref_id: col_crossref_id.and_then(|i| parse_i64_field(&record, i)),
            issuetitle_latex: col_issuetitle_latex.and_then(|i| get_field(&record, i)),
            issuetitle_unicode: col_issuetitle_unicode.and_then(|i| get_field(&record, i)),
            note_latex: col_note_latex.and_then(|i| get_field(&record, i)),
            note_unicode: col_note_unicode.and_then(|i| get_field(&record, i)),
            extra_note_latex: col_extra_note_latex.and_then(|i| get_field(&record, i)),
            extra_note_unicode: col_extra_note_unicode.and_then(|i| get_field(&record, i)),
            langid: col_langid
                .and_then(|i| get_field(&record, i))
                .and_then(|s| s.parse().ok()),
            is_translation,
            epoch: col_epoch
                .and_then(|i| get_field(&record, i))
                .and_then(|s| s.parse().ok()),
            options: col_options.and_then(|i| get_field(&record, i)),
            shorthand: col_shorthand.and_then(|i| get_field(&record, i)),
            person_id: None,
            has_fulltext: false,
            fulltext_path: None,
        };

        rows.push(ParsedBibitemRow {
            row_num,
            source_id,
            bibkey,
            dto,
            author_ids: col_author_ids
                .map(|i| parse_id_list(&record, i))
                .unwrap_or_default(),
            editor_ids: col_editor_ids
                .map(|i| parse_id_list(&record, i))
                .unwrap_or_default(),
            guesteditor_ids: col_guesteditor_ids
                .map(|i| parse_id_list(&record, i))
                .unwrap_or_default(),
            keyword_ids: col_keyword_ids
                .map(|i| parse_id_list(&record, i))
                .unwrap_or_default(),
        });
    }

    Ok((rows, errors))
}

pub fn parse_bibitem_refs_csv(
    data: &[u8],
) -> Result<(Vec<ParsedBibitemRefRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_source_id = require_column(&headers, "source_id")?;
    let col_target_id = require_column(&headers, "target_id")?;
    let col_ref_type = require_column(&headers, "ref_type")?;

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, result) in (2usize..).zip(reader.records()) {
        let row_num = i;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: e.to_string(),
                });
                continue;
            }
        };

        let source_id = match parse_i64_field(&record, col_source_id) {
            Some(v) => v,
            None => {
                let raw = get_field(&record, col_source_id).unwrap_or_default();
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: raw.clone(),
                    error: format!("invalid source_id: {raw}"),
                });
                continue;
            }
        };

        let target_id = match parse_i64_field(&record, col_target_id) {
            Some(v) => v,
            None => {
                let raw = get_field(&record, col_target_id).unwrap_or_default();
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: format!("{source_id}->?"),
                    error: format!("invalid target_id: {raw}"),
                });
                continue;
            }
        };

        let ref_type = get_field(&record, col_ref_type).unwrap_or_default();
        if !matches!(ref_type.as_str(), "further_ref" | "depends_on") {
            errors.push(ImportRowError {
                row: row_num,
                identifier: format!("{source_id}->{target_id}"),
                error: format!("unknown ref_type: {ref_type}"),
            });
            continue;
        }

        rows.push(ParsedBibitemRefRow {
            source_id,
            target_id,
            ref_type,
        });
    }

    Ok((rows, errors))
}

pub fn parse_bibitem_notes_csv(
    data: &[u8],
) -> Result<(Vec<ParsedBibitemNotesRow>, Vec<ImportRowError>), HexforgeError> {
    let mut reader = csv_reader(data);
    let headers = parse_csv_headers(&mut reader)?;

    let col_bibitem_id = require_column(&headers, "bibitem_id")?;
    let col_note_perso = column_index(&headers, "note_perso");
    let col_note_stock = column_index(&headers, "note_stock");
    let col_note_missing = column_index(&headers, "note_missing");
    let col_change_request = column_index(&headers, "change_request");
    let col_dltc = column_index(&headers, "dltc_copyediting_note");
    let col_todo = column_index(&headers, "todo_general");

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, result) in (2usize..).zip(reader.records()) {
        let row_num = i;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: e.to_string(),
                });
                continue;
            }
        };

        let bibitem_id = match parse_i64_field(&record, col_bibitem_id) {
            Some(v) => v,
            None => {
                let raw = get_field(&record, col_bibitem_id).unwrap_or_default();
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: raw.clone(),
                    error: format!("invalid bibitem_id: {raw}"),
                });
                continue;
            }
        };

        rows.push(ParsedBibitemNotesRow {
            bibitem_id,
            note_perso: col_note_perso.and_then(|i| get_field(&record, i)),
            note_stock: col_note_stock.and_then(|i| get_field(&record, i)),
            note_missing: col_note_missing.and_then(|i| get_field(&record, i)),
            change_request: col_change_request.and_then(|i| get_field(&record, i)),
            dltc_copyediting_note: col_dltc.and_then(|i| get_field(&record, i)),
            todo_general: col_todo.and_then(|i| get_field(&record, i)),
        });
    }

    Ok((rows, errors))
}
