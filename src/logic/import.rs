//! Import types and pure helpers — CSV field parsing, column mapping, DTO construction.
//!
//! This module contains ONLY pure types and functions (no async, no database, no I/O).
//! Orchestration logic lives in `crate::process::import`.

use hexforge::{HexforgeError, ValidationError};
use serde::Serialize;

use crate::domain::{CreateBibItem, UpdateBibItem};

// =============================================================================
// Response types
// =============================================================================

/// Import result with counts and any errors.
#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub imported: usize,
    pub updated: usize,
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
// Name variant type (for author name variant import)
// =============================================================================

/// The type of author name variant: LaTeX or Unicode.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NameVariantType {
    Latex,
    Unicode,
}

impl NameVariantType {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "latex" => Some(Self::Latex),
            "unicode" => Some(Self::Unicode),
            _ => None,
        }
    }

    pub fn column_name(&self) -> &'static str {
        match self {
            Self::Latex => "name_variants_latex",
            Self::Unicode => "name_variants_unicode",
        }
    }
}

// =============================================================================
// CSV field helpers (pure functions)
// =============================================================================

/// Get a trimmed, non-empty string from a CSV record at the given index.
pub fn get_field(record: &csv::StringRecord, idx: usize) -> Option<String> {
    record
        .get(idx)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Parse an i16 from a CSV field.
pub fn parse_i16_field(record: &csv::StringRecord, idx: usize) -> Option<i16> {
    get_field(record, idx).and_then(|s| s.parse().ok())
}

/// Parse an i64 from a CSV field.
pub fn parse_i64_field(record: &csv::StringRecord, idx: usize) -> Option<i64> {
    get_field(record, idx).and_then(|s| s.parse().ok())
}

/// Parse comma-separated i64 IDs from a CSV field.
pub fn parse_id_list(record: &csv::StringRecord, idx: usize) -> Vec<i64> {
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
pub fn column_index(headers: &csv::StringRecord, name: &str) -> Option<usize> {
    headers.iter().position(|h| h.trim() == name)
}

/// Require a column, returning a validation error if not found.
pub fn require_column(headers: &csv::StringRecord, name: &str) -> Result<usize, HexforgeError> {
    column_index(headers, name).ok_or_else(|| {
        HexforgeError::Validation(ValidationError::custom(format!(
            "Missing required column: {name}"
        )))
    })
}

// =============================================================================
// DTO helpers
// =============================================================================

/// Build an `UpdateBibItem` from a `CreateBibItem` (for upsert logic).
pub fn build_bibitem_update_dto(create: &CreateBibItem) -> UpdateBibItem {
    UpdateBibItem {
        bibkey: Some(create.bibkey.clone()),
        entry_type: Some(create.entry_type),
        date_year: create.date_year,
        date_year_2_hyphen: create.date_year_2_hyphen,
        date_year_2_slash: create.date_year_2_slash,
        date_month: create.date_month,
        date_day: create.date_day,
        date_is_no_date: Some(create.date_is_no_date),
        pubstate: create.pubstate,
        title_latex: Some(create.title_latex.clone()),
        title_unicode: Some(create.title_unicode.clone()),
        booktitle_latex: create.booktitle_latex.clone(),
        booktitle_unicode: create.booktitle_unicode.clone(),
        journal_id: create.journal_id,
        publisher_id: create.publisher_id,
        address: create.address.clone(),
        volume: create.volume.clone(),
        number: create.number.clone(),
        pages: create.pages.clone(),
        eid: create.eid.clone(),
        series_id: create.series_id,
        edition: create.edition.clone(),
        institution_id: create.institution_id,
        school_id: create.school_id,
        type_field: create.type_field.clone(),
        doi: create.doi.clone(),
        url: create.url.clone(),
        eprint: create.eprint.clone(),
        urn: create.urn.clone(),
        crossref_id: create.crossref_id,
        issuetitle_latex: create.issuetitle_latex.clone(),
        issuetitle_unicode: create.issuetitle_unicode.clone(),
        note_latex: create.note_latex.clone(),
        note_unicode: create.note_unicode.clone(),
        extra_note_latex: create.extra_note_latex.clone(),
        extra_note_unicode: create.extra_note_unicode.clone(),
        langid: create.langid,
        is_translation: Some(create.is_translation),
        epoch: create.epoch,
        options: create.options.clone(),
        shorthand: create.shorthand.clone(),
        person_id: create.person_id,
        has_fulltext: Some(create.has_fulltext),
        fulltext_path: create.fulltext_path.clone(),
    }
}

/// Format a `DataSourceError` for user-facing import error messages.
pub fn format_insert_error(e: hexforge::DataSourceError) -> String {
    let msg = e.to_string();
    if msg.contains("duplicate key") || msg.contains("23505") {
        format!("Duplicate key: {msg}")
    } else {
        msg
    }
}
