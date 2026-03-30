//! Import logic — CSV parsing, row validation, reference checking, and entity insertion.
//!
//! Contains all business logic for importing entities and bibitems from CSV data.
//! No HTTP types — takes raw CSV bytes, returns structured results.

use std::collections::HashSet;

use hexforge::db_exports::{FromRow, query, query_as};
use hexforge::{DataSource, HexforgeError, ValidationError};
use serde::Serialize;

use crate::domain::{
    AuthorRole, CreateAuthor, CreateBibItem, CreateInstitution, CreateJournal, CreateKeyword,
    CreatePublisher, CreateSchool, CreateSeries, EntryType, create_author_transform,
    create_bib_item_transform, create_institution_transform, create_journal_transform,
    create_keyword_transform, create_publisher_transform, create_school_transform,
    create_series_transform,
};
use crate::state::AppState;
use crate::validation::{
    validate_create_author, validate_create_bibitem, validate_create_institution,
    validate_create_journal, validate_create_keyword, validate_create_publisher,
    validate_create_school, validate_create_series,
};

// =============================================================================
// Response types
// =============================================================================

/// Import result with counts and any errors.
#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub imported: usize,
    pub failed: usize,
    pub errors: Vec<ImportRowError>,
}

/// Single import error with row and message.
#[derive(Debug, Serialize)]
pub struct ImportRowError {
    pub row: usize,
    pub identifier: String,
    pub error: String,
}

/// Error response for missing referenced IDs during bibitem import.
#[derive(Debug, Serialize)]
pub struct MissingReferencesError {
    pub error: &'static str,
    pub message: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_author_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_journal_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_publisher_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_institution_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_school_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_series_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_keyword_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_crossref_ids: Vec<i64>,
}

impl MissingReferencesError {
    pub fn has_missing(&self) -> bool {
        !self.missing_author_ids.is_empty()
            || !self.missing_journal_ids.is_empty()
            || !self.missing_publisher_ids.is_empty()
            || !self.missing_institution_ids.is_empty()
            || !self.missing_school_ids.is_empty()
            || !self.missing_series_ids.is_empty()
            || !self.missing_keyword_ids.is_empty()
            || !self.missing_crossref_ids.is_empty()
    }
}

/// Result of bibitem import: either success or missing references.
pub enum BibitemImportResult {
    /// Import completed (may contain per-row errors).
    Success(ImportResponse),
    /// Pre-flight reference check failed.
    MissingReferences(MissingReferencesError),
}

// =============================================================================
// CSV field helpers (pure functions)
// =============================================================================

/// Get a trimmed, non-empty string from a CSV record at the given index.
fn get_field(record: &csv::StringRecord, idx: usize) -> Option<String> {
    record
        .get(idx)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Parse an i16 from a CSV field.
fn parse_i16_field(record: &csv::StringRecord, idx: usize) -> Option<i16> {
    get_field(record, idx).and_then(|s| s.parse().ok())
}

/// Parse an i64 from a CSV field.
fn parse_i64_field(record: &csv::StringRecord, idx: usize) -> Option<i64> {
    get_field(record, idx).and_then(|s| s.parse().ok())
}

/// Parse a bool from a CSV field.
fn parse_bool_field(record: &csv::StringRecord, idx: usize) -> Option<bool> {
    get_field(record, idx)
        .map(|s| matches!(s.to_lowercase().as_str(), "true" | "1" | "yes" | "y" | "x"))
}

/// Parse comma-separated i64 IDs from a CSV field.
fn parse_id_list(record: &csv::StringRecord, idx: usize) -> Vec<i64> {
    get_field(record, idx)
        .map(|s| {
            s.split(',')
                .filter_map(|part| part.trim().parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

// =============================================================================
// Column mapping helpers
// =============================================================================

/// Build a column index map from CSV headers.
fn column_index(headers: &csv::StringRecord, name: &str) -> Option<usize> {
    headers.iter().position(|h| h.trim() == name)
}

/// Require a column, returning a validation error if not found.
fn require_column(headers: &csv::StringRecord, name: &str) -> Result<usize, HexforgeError> {
    column_index(headers, name).ok_or_else(|| {
        HexforgeError::Validation(ValidationError::custom(format!(
            "Missing required column: {name}"
        )))
    })
}

// =============================================================================
// Author import
// =============================================================================

/// Import authors from CSV bytes.
pub async fn import_authors_from_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    let col_author_key = require_column(&headers, "author_key")?;
    let col_given_name_latex = column_index(&headers, "given_name_latex");
    let col_given_name_unicode = column_index(&headers, "given_name_unicode");
    let col_given_name_simplified = column_index(&headers, "given_name_simplified");
    let col_family_name_latex = column_index(&headers, "family_name_latex");
    let col_family_name_unicode = column_index(&headers, "family_name_unicode");
    let col_family_name_simplified = column_index(&headers, "family_name_simplified");
    let col_mononym_latex = column_index(&headers, "mononym_latex");
    let col_mononym_unicode = column_index(&headers, "mononym_unicode");
    let col_mononym_simplified = column_index(&headers, "mononym_simplified");
    let col_shorthand_latex = column_index(&headers, "shorthand_latex");
    let col_shorthand_unicode = column_index(&headers, "shorthand_unicode");
    let col_shorthand_simplified = column_index(&headers, "shorthand_simplified");
    let col_famous_name_latex = column_index(&headers, "famous_name_latex");
    let col_famous_name_unicode = column_index(&headers, "famous_name_unicode");
    let col_famous_name_simplified = column_index(&headers, "famous_name_simplified");

    let mut imported = 0usize;
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

        let dto = CreateAuthor {
            author_key: author_key.clone(),
            given_name_latex: col_given_name_latex.and_then(|i| get_field(&record, i)),
            given_name_unicode: col_given_name_unicode.and_then(|i| get_field(&record, i)),
            given_name_simplified: col_given_name_simplified.and_then(|i| get_field(&record, i)),
            family_name_latex: col_family_name_latex.and_then(|i| get_field(&record, i)),
            family_name_unicode: col_family_name_unicode.and_then(|i| get_field(&record, i)),
            family_name_simplified: col_family_name_simplified.and_then(|i| get_field(&record, i)),
            mononym_latex: col_mononym_latex.and_then(|i| get_field(&record, i)),
            mononym_unicode: col_mononym_unicode.and_then(|i| get_field(&record, i)),
            mononym_simplified: col_mononym_simplified.and_then(|i| get_field(&record, i)),
            shorthand_latex: col_shorthand_latex.and_then(|i| get_field(&record, i)),
            shorthand_unicode: col_shorthand_unicode.and_then(|i| get_field(&record, i)),
            shorthand_simplified: col_shorthand_simplified.and_then(|i| get_field(&record, i)),
            famous_name_latex: col_famous_name_latex.and_then(|i| get_field(&record, i)),
            famous_name_unicode: col_famous_name_unicode.and_then(|i| get_field(&record, i)),
            famous_name_simplified: col_famous_name_simplified.and_then(|i| get_field(&record, i)),
        };

        if let Err(e) = validate_create_author(&dto) {
            errors.push(ImportRowError {
                row: row_num,
                identifier: author_key,
                error: e.to_string(),
            });
            continue;
        }

        let entity = create_author_transform(dto);
        match state.author_ds.insert(entity).await {
            Ok(_) => imported += 1,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: author_key,
                    error: format_insert_error(e),
                });
            }
        }
    }

    Ok(ImportResponse {
        imported,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Journal import
// =============================================================================

/// Import journals from CSV bytes.
pub async fn import_journals_from_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    let col_journal_key = require_column(&headers, "journal_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_name_simplified = column_index(&headers, "name_simplified");
    let col_issn_print = column_index(&headers, "issn_print");
    let col_issn_electronic = column_index(&headers, "issn_electronic");

    let mut imported = 0usize;
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

        let dto = CreateJournal {
            journal_key: journal_key.clone(),
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
            name_simplified: col_name_simplified.and_then(|i| get_field(&record, i)),
            issn_print: col_issn_print.and_then(|i| get_field(&record, i)),
            issn_electronic: col_issn_electronic.and_then(|i| get_field(&record, i)),
        };

        if let Err(e) = validate_create_journal(&dto) {
            errors.push(ImportRowError {
                row: row_num,
                identifier: journal_key,
                error: e.to_string(),
            });
            continue;
        }

        let entity = create_journal_transform(dto);
        match state.journal_ds.insert(entity).await {
            Ok(_) => imported += 1,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: journal_key,
                    error: format_insert_error(e),
                });
            }
        }
    }

    Ok(ImportResponse {
        imported,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Publisher import
// =============================================================================

/// Import publishers from CSV bytes.
pub async fn import_publishers_from_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    let col_publisher_key = require_column(&headers, "publisher_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_name_simplified = column_index(&headers, "name_simplified");
    let col_default_address = column_index(&headers, "default_address");

    let mut imported = 0usize;
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

        let dto = CreatePublisher {
            publisher_key: publisher_key.clone(),
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
            name_simplified: col_name_simplified.and_then(|i| get_field(&record, i)),
            default_address: col_default_address.and_then(|i| get_field(&record, i)),
        };

        if let Err(e) = validate_create_publisher(&dto) {
            errors.push(ImportRowError {
                row: row_num,
                identifier: publisher_key,
                error: e.to_string(),
            });
            continue;
        }

        let entity = create_publisher_transform(dto);
        match state.publisher_ds.insert(entity).await {
            Ok(_) => imported += 1,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: publisher_key,
                    error: format_insert_error(e),
                });
            }
        }
    }

    Ok(ImportResponse {
        imported,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Institution import
// =============================================================================

/// Import institutions from CSV bytes.
pub async fn import_institutions_from_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    let col_institution_key = require_column(&headers, "institution_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_name_simplified = column_index(&headers, "name_simplified");
    let col_default_address = column_index(&headers, "default_address");

    let mut imported = 0usize;
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

        let dto = CreateInstitution {
            institution_key: institution_key.clone(),
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
            name_simplified: col_name_simplified.and_then(|i| get_field(&record, i)),
            default_address: col_default_address.and_then(|i| get_field(&record, i)),
        };

        if let Err(e) = validate_create_institution(&dto) {
            errors.push(ImportRowError {
                row: row_num,
                identifier: institution_key,
                error: e.to_string(),
            });
            continue;
        }

        let entity = create_institution_transform(dto);
        match state.institution_ds.insert(entity).await {
            Ok(_) => imported += 1,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: institution_key,
                    error: format_insert_error(e),
                });
            }
        }
    }

    Ok(ImportResponse {
        imported,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// School import
// =============================================================================

/// Import schools from CSV bytes.
pub async fn import_schools_from_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    let col_school_key = require_column(&headers, "school_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_name_simplified = column_index(&headers, "name_simplified");
    let col_default_address = column_index(&headers, "default_address");

    let mut imported = 0usize;
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

        let dto = CreateSchool {
            school_key: school_key.clone(),
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
            name_simplified: col_name_simplified.and_then(|i| get_field(&record, i)),
            default_address: col_default_address.and_then(|i| get_field(&record, i)),
        };

        if let Err(e) = validate_create_school(&dto) {
            errors.push(ImportRowError {
                row: row_num,
                identifier: school_key,
                error: e.to_string(),
            });
            continue;
        }

        let entity = create_school_transform(dto);
        match state.school_ds.insert(entity).await {
            Ok(_) => imported += 1,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: school_key,
                    error: format_insert_error(e),
                });
            }
        }
    }

    Ok(ImportResponse {
        imported,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Series import
// =============================================================================

/// Import series from CSV bytes.
pub async fn import_series_from_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    let col_series_key = require_column(&headers, "series_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_name_simplified = column_index(&headers, "name_simplified");

    let mut imported = 0usize;
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

        let dto = CreateSeries {
            series_key: series_key.clone(),
            name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
            name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
            name_simplified: col_name_simplified.and_then(|i| get_field(&record, i)),
        };

        if let Err(e) = validate_create_series(&dto) {
            errors.push(ImportRowError {
                row: row_num,
                identifier: series_key,
                error: e.to_string(),
            });
            continue;
        }

        let entity = create_series_transform(dto);
        match state.series_ds.insert(entity).await {
            Ok(_) => imported += 1,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: series_key,
                    error: format_insert_error(e),
                });
            }
        }
    }

    Ok(ImportResponse {
        imported,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Keyword import
// =============================================================================

/// Import keywords from CSV bytes.
pub async fn import_keywords_from_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    let col_name = require_column(&headers, "name")?;
    let col_level = require_column(&headers, "level")?;

    let mut imported = 0usize;
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

        let dto = CreateKeyword {
            name: name.clone(),
            level,
        };

        if let Err(e) = validate_create_keyword(&dto) {
            errors.push(ImportRowError {
                row: row_num,
                identifier: name,
                error: e.to_string(),
            });
            continue;
        }

        let entity = create_keyword_transform(dto);
        match state.keyword_ds.insert(entity).await {
            Ok(_) => imported += 1,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: name,
                    error: format_insert_error(e),
                });
            }
        }
    }

    Ok(ImportResponse {
        imported,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Bibitem import (IDs format)
// =============================================================================

/// Parsed bibitem row with author/keyword junction data.
struct ParsedBibitemRow {
    row_num: usize,
    bibkey: String,
    dto: CreateBibItem,
    author_ids: Vec<i64>,
    editor_ids: Vec<i64>,
    guesteditor_ids: Vec<i64>,
    keyword_ids: Vec<i64>,
}

/// Import bibitems from CSV bytes (IDs format).
///
/// Before inserting, validates ALL referenced IDs exist. If any are missing,
/// returns all missing IDs in a `MissingReferences` result and inserts nothing.
pub async fn import_bibitems_from_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<BibitemImportResult, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    // Map column names to indices
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
    let col_title_simplified = column_index(&headers, "title_simplified");
    let col_booktitle_latex = column_index(&headers, "booktitle_latex");
    let col_booktitle_unicode = column_index(&headers, "booktitle_unicode");
    let col_booktitle_simplified = column_index(&headers, "booktitle_simplified");
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

    // Phase 1: Parse all rows, collect referenced IDs
    let mut parsed_rows: Vec<ParsedBibitemRow> = Vec::new();
    let mut parse_errors: Vec<ImportRowError> = Vec::new();

    let mut all_author_ids: HashSet<i64> = HashSet::new();
    let mut all_journal_ids: HashSet<i64> = HashSet::new();
    let mut all_publisher_ids: HashSet<i64> = HashSet::new();
    let mut all_institution_ids: HashSet<i64> = HashSet::new();
    let mut all_school_ids: HashSet<i64> = HashSet::new();
    let mut all_series_ids: HashSet<i64> = HashSet::new();
    let mut all_keyword_ids: HashSet<i64> = HashSet::new();
    let mut all_crossref_ids: HashSet<i64> = HashSet::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                parse_errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: format!("CSV parse error: {e}"),
                });
                continue;
            }
        };

        let bibkey = match get_field(&record, col_bibkey) {
            Some(k) => k,
            None => {
                parse_errors.push(ImportRowError {
                    row: row_num,
                    identifier: String::new(),
                    error: "Missing bibkey".to_string(),
                });
                continue;
            }
        };

        let entry_type_str = get_field(&record, col_entry_type).unwrap_or_default();
        let entry_type: EntryType = entry_type_str.parse().unwrap_or(EntryType::Unknown);

        let title_latex = col_title_latex
            .and_then(|i| get_field(&record, i))
            .unwrap_or_default();
        let title_unicode = col_title_unicode
            .and_then(|i| get_field(&record, i))
            .unwrap_or_else(|| title_latex.clone());
        let title_simplified = col_title_simplified
            .and_then(|i| get_field(&record, i))
            .unwrap_or_else(|| title_latex.clone());

        let author_ids_list = col_author_ids
            .map(|i| parse_id_list(&record, i))
            .unwrap_or_default();
        let editor_ids_list = col_editor_ids
            .map(|i| parse_id_list(&record, i))
            .unwrap_or_default();
        let guesteditor_ids_list = col_guesteditor_ids
            .map(|i| parse_id_list(&record, i))
            .unwrap_or_default();
        let keyword_ids_list = col_keyword_ids
            .map(|i| parse_id_list(&record, i))
            .unwrap_or_default();

        // Collect all referenced IDs for batch validation
        all_author_ids.extend(&author_ids_list);
        all_author_ids.extend(&editor_ids_list);
        all_author_ids.extend(&guesteditor_ids_list);
        all_keyword_ids.extend(&keyword_ids_list);

        let journal_id = col_journal_id.and_then(|i| parse_i64_field(&record, i));
        let publisher_id = col_publisher_id.and_then(|i| parse_i64_field(&record, i));
        let institution_id = col_institution_id.and_then(|i| parse_i64_field(&record, i));
        let school_id = col_school_id.and_then(|i| parse_i64_field(&record, i));
        let series_id = col_series_id.and_then(|i| parse_i64_field(&record, i));
        let crossref_id = col_crossref_id.and_then(|i| parse_i64_field(&record, i));

        if let Some(id) = journal_id {
            all_journal_ids.insert(id);
        }
        if let Some(id) = publisher_id {
            all_publisher_ids.insert(id);
        }
        if let Some(id) = institution_id {
            all_institution_ids.insert(id);
        }
        if let Some(id) = school_id {
            all_school_ids.insert(id);
        }
        if let Some(id) = series_id {
            all_series_ids.insert(id);
        }
        if let Some(id) = crossref_id {
            all_crossref_ids.insert(id);
        }

        let dto = CreateBibItem {
            bibkey: bibkey.clone(),
            entry_type,
            date_year: col_date_year.and_then(|i| parse_i16_field(&record, i)),
            date_year_2_hyphen: None,
            date_year_2_slash: None,
            date_month: col_date_month.and_then(|i| parse_i16_field(&record, i)),
            date_day: col_date_day.and_then(|i| parse_i16_field(&record, i)),
            date_is_no_date: None,
            pubstate: col_pubstate
                .and_then(|i| get_field(&record, i))
                .and_then(|s| s.parse().ok()),
            title_latex,
            title_unicode,
            title_simplified,
            booktitle_latex: col_booktitle_latex.and_then(|i| get_field(&record, i)),
            booktitle_unicode: col_booktitle_unicode.and_then(|i| get_field(&record, i)),
            booktitle_simplified: col_booktitle_simplified.and_then(|i| get_field(&record, i)),
            journal_id,
            publisher_id,
            address: col_address.and_then(|i| get_field(&record, i)),
            volume: col_volume.and_then(|i| get_field(&record, i)),
            number: col_number.and_then(|i| get_field(&record, i)),
            pages: col_pages.and_then(|i| get_field(&record, i)),
            eid: col_eid.and_then(|i| get_field(&record, i)),
            series_id,
            edition: col_edition.and_then(|i| get_field(&record, i)),
            institution_id,
            school_id,
            type_field: col_type_field.and_then(|i| get_field(&record, i)),
            doi: col_doi.and_then(|i| get_field(&record, i)),
            url: col_url.and_then(|i| get_field(&record, i)),
            eprint: col_eprint.and_then(|i| get_field(&record, i)),
            urn: col_urn.and_then(|i| get_field(&record, i)),
            crossref_id,
            issuetitle_latex: col_issuetitle_latex.and_then(|i| get_field(&record, i)),
            issuetitle_unicode: col_issuetitle_unicode.and_then(|i| get_field(&record, i)),
            note_latex: col_note_latex.and_then(|i| get_field(&record, i)),
            note_unicode: col_note_unicode.and_then(|i| get_field(&record, i)),
            extra_note_latex: col_extra_note_latex.and_then(|i| get_field(&record, i)),
            extra_note_unicode: col_extra_note_unicode.and_then(|i| get_field(&record, i)),
            langid: col_langid
                .and_then(|i| get_field(&record, i))
                .and_then(|s| s.parse().ok()),
            is_translation: col_is_translation.and_then(|i| parse_bool_field(&record, i)),
            epoch: col_epoch
                .and_then(|i| get_field(&record, i))
                .and_then(|s| s.parse().ok()),
            options: col_options.and_then(|i| get_field(&record, i)),
            shorthand: col_shorthand.and_then(|i| get_field(&record, i)),
            person_id: None,
            has_fulltext: None,
            fulltext_path: None,
        };

        // Validate the DTO
        if let Err(e) = validate_create_bibitem(&dto) {
            parse_errors.push(ImportRowError {
                row: row_num,
                identifier: bibkey,
                error: e.to_string(),
            });
            continue;
        }

        parsed_rows.push(ParsedBibitemRow {
            row_num,
            bibkey,
            dto,
            author_ids: author_ids_list,
            editor_ids: editor_ids_list,
            guesteditor_ids: guesteditor_ids_list,
            keyword_ids: keyword_ids_list,
        });
    }

    // If there were parse errors, return them without inserting
    if !parse_errors.is_empty() {
        return Ok(BibitemImportResult::Success(ImportResponse {
            imported: 0,
            failed: parse_errors.len(),
            errors: parse_errors,
        }));
    }

    if parsed_rows.is_empty() {
        return Ok(BibitemImportResult::Success(ImportResponse {
            imported: 0,
            failed: 0,
            errors: vec![],
        }));
    }

    // Phase 2: Batch-check all referenced IDs exist
    let missing = check_all_references(
        state,
        &all_author_ids,
        &all_journal_ids,
        &all_publisher_ids,
        &all_institution_ids,
        &all_school_ids,
        &all_series_ids,
        &all_keyword_ids,
        &all_crossref_ids,
    )
    .await?;

    if missing.has_missing() {
        return Ok(BibitemImportResult::MissingReferences(missing));
    }

    // Phase 3: Insert all bibitems and their junction data
    let mut imported = 0usize;
    let mut insert_errors = Vec::new();

    for row in &parsed_rows {
        let bibitem = create_bib_item_transform(row.dto.clone());
        match state.bibitem_ds.insert(bibitem).await {
            Ok(inserted) => {
                // Insert junction data for authors
                if let Err(e) =
                    insert_bibitem_authors(state, inserted.id, &row.author_ids, AuthorRole::Author)
                        .await
                {
                    insert_errors.push(ImportRowError {
                        row: row.row_num,
                        identifier: row.bibkey.clone(),
                        error: format!("Failed to link authors: {e}"),
                    });
                    continue;
                }
                if let Err(e) =
                    insert_bibitem_authors(state, inserted.id, &row.editor_ids, AuthorRole::Editor)
                        .await
                {
                    insert_errors.push(ImportRowError {
                        row: row.row_num,
                        identifier: row.bibkey.clone(),
                        error: format!("Failed to link editors: {e}"),
                    });
                    continue;
                }
                if let Err(e) = insert_bibitem_authors(
                    state,
                    inserted.id,
                    &row.guesteditor_ids,
                    AuthorRole::Guesteditor,
                )
                .await
                {
                    insert_errors.push(ImportRowError {
                        row: row.row_num,
                        identifier: row.bibkey.clone(),
                        error: format!("Failed to link guesteditors: {e}"),
                    });
                    continue;
                }

                // Insert junction data for keywords
                if let Err(e) = insert_bibitem_keywords(state, inserted.id, &row.keyword_ids).await
                {
                    insert_errors.push(ImportRowError {
                        row: row.row_num,
                        identifier: row.bibkey.clone(),
                        error: format!("Failed to link keywords: {e}"),
                    });
                    continue;
                }

                imported += 1;
            }
            Err(e) => {
                insert_errors.push(ImportRowError {
                    row: row.row_num,
                    identifier: row.bibkey.clone(),
                    error: format_insert_error(e),
                });
            }
        }
    }

    Ok(BibitemImportResult::Success(ImportResponse {
        imported,
        failed: insert_errors.len(),
        errors: insert_errors,
    }))
}

// =============================================================================
// Reference checking
// =============================================================================

/// Check all referenced IDs exist, returning any missing ones.
#[allow(clippy::too_many_arguments)]
async fn check_all_references(
    state: &AppState,
    author_ids: &HashSet<i64>,
    journal_ids: &HashSet<i64>,
    publisher_ids: &HashSet<i64>,
    institution_ids: &HashSet<i64>,
    school_ids: &HashSet<i64>,
    series_ids: &HashSet<i64>,
    keyword_ids: &HashSet<i64>,
    crossref_ids: &HashSet<i64>,
) -> Result<MissingReferencesError, HexforgeError> {
    let pool = state.pool.pool();

    let missing_authors = find_missing_in_table(pool, "authors", author_ids).await?;
    let missing_journals = find_missing_in_table(pool, "journals", journal_ids).await?;
    let missing_publishers = find_missing_in_table(pool, "publishers", publisher_ids).await?;
    let missing_institutions = find_missing_in_table(pool, "institutions", institution_ids).await?;
    let missing_schools = find_missing_in_table(pool, "schools", school_ids).await?;
    let missing_series = find_missing_in_table(pool, "series", series_ids).await?;
    let missing_keywords = find_missing_in_table(pool, "keywords", keyword_ids).await?;
    let missing_crossrefs = find_missing_in_table(pool, "bibitems", crossref_ids).await?;

    Ok(MissingReferencesError {
        error: "missing_references",
        message: "Some referenced entities were not found",
        missing_author_ids: missing_authors,
        missing_journal_ids: missing_journals,
        missing_publisher_ids: missing_publishers,
        missing_institution_ids: missing_institutions,
        missing_school_ids: missing_schools,
        missing_series_ids: missing_series,
        missing_keyword_ids: missing_keywords,
        missing_crossref_ids: missing_crossrefs,
    })
}

/// Find which IDs from the given set don't exist in the given table.
async fn find_missing_in_table(
    pool: &hexforge::db_exports::PgPool,
    table: &str,
    ids: &HashSet<i64>,
) -> Result<Vec<i64>, HexforgeError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let id_vec: Vec<i64> = ids.iter().copied().collect();

    // Query for IDs that DO exist, then diff
    let sql = format!("SELECT id FROM {table} WHERE id = ANY($1)");
    let found_ids: Vec<i64> = hexforge::db_exports::query_scalar(&sql)
        .bind(&id_vec)
        .fetch_all(pool)
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

// =============================================================================
// Junction table insertion helpers
// =============================================================================

/// Insert bibitem-author junction records.
async fn insert_bibitem_authors(
    state: &AppState,
    bibitem_id: i64,
    author_ids: &[i64],
    role: AuthorRole,
) -> Result<(), HexforgeError> {
    let role_str = role.to_string();
    for (position, &author_id) in author_ids.iter().enumerate() {
        let pos = i16::try_from(position).unwrap_or(0);
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
        .bind(pos)
        .execute(state.pool.pool())
        .await
        .map_err(HexforgeError::data_source)?;
    }
    Ok(())
}

/// Insert bibitem-keyword junction records.
/// Keywords are looked up to determine their level.
async fn insert_bibitem_keywords(
    state: &AppState,
    bibitem_id: i64,
    keyword_ids: &[i64],
) -> Result<(), HexforgeError> {
    if keyword_ids.is_empty() {
        return Ok(());
    }

    // Fetch keyword levels
    let kw_rows: Vec<KeywordLevelRow> =
        query_as("SELECT id, level FROM keywords WHERE id = ANY($1)")
            .bind(keyword_ids)
            .fetch_all(state.pool.pool())
            .await
            .map_err(HexforgeError::data_source)?;

    for kw in &kw_rows {
        query(
            "INSERT INTO bibitem_keywords (bibitem_id, keyword_id, keyword_level) VALUES ($1, $2, $3) ON CONFLICT (bibitem_id, keyword_id) DO NOTHING",
        )
        .bind(bibitem_id)
        .bind(kw.id)
        .bind(kw.level)
        .execute(state.pool.pool())
        .await
        .map_err(HexforgeError::data_source)?;
    }

    Ok(())
}

#[derive(Debug, FromRow)]
struct KeywordLevelRow {
    id: i64,
    level: i16,
}

// =============================================================================
// Helpers
// =============================================================================

fn format_insert_error(e: hexforge::DataSourceError) -> String {
    let msg = e.to_string();
    if msg.contains("duplicate key") || msg.contains("23505") {
        format!("Duplicate key: {msg}")
    } else {
        msg
    }
}
