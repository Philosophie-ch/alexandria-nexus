//! OpenAPI schema implementations for logic-layer types.
//!
//! Keeps `utoipa::ToSchema` out of `src/logic/` by providing the impls
//! externally via `impl_to_schema!`, analogous to how `impl_db_enum!`
//! keeps sqlx out of `src/domain/`.

use hexforge::impl_to_schema;

// =============================================================================
// Full import response types
// =============================================================================

impl_to_schema! {
    struct crate::logic::full_import::FieldError => "FieldError" {
        field: String,
        error: String,
    }
}

impl_to_schema! {
    struct crate::logic::full_import::ValidationReport => "ValidationReport" {
        total_rows: usize,
        valid_rows: usize,
        errors: Vec<crate::logic::full_import::RowError>,
        duplicate_bibkeys: Vec<crate::logic::full_import::DuplicateBibkey>,
        missing_authors: Vec<String>,
        ambiguous_authors: Vec<crate::logic::full_import::AmbiguousAuthor>,
        missing_journals: Vec<String>,
        missing_publishers: Vec<String>,
        missing_institutions: Vec<String>,
        missing_schools: Vec<String>,
        missing_series: Vec<String>,
        missing_keywords: crate::logic::full_import::MissingKeywords,
        missing_crossrefs: Vec<String>,
        missing_further_refs: Vec<String>,
        missing_depends_on: Vec<String>,
        stale_bibitems: Vec<String>,
    }
}

impl_to_schema! {
    struct crate::logic::full_import::RowError => "RowError" {
        row: usize,
        bibkey: Option<String>,
        errors: Vec<crate::logic::full_import::FieldError>,
    }
}

impl_to_schema! {
    struct crate::logic::full_import::AmbiguousAuthor => "AmbiguousAuthor" {
        name: String,
        matching_ids: Vec<i64>,
    }
}

impl_to_schema! {
    struct crate::logic::full_import::DuplicateBibkey => "DuplicateBibkey" {
        bibkey: String,
        rows: Vec<usize>,
    }
}

impl_to_schema! {
    struct crate::logic::full_import::MissingKeywords => "MissingKeywords" {
        level_1: Vec<String>,
        level_2: Vec<String>,
        level_3: Vec<String>,
    }
}

// =============================================================================
// Entity import report types
// =============================================================================

impl_to_schema! {
    struct crate::logic::full_import::EntityImportReport => "EntityImportReport" {
        created_institutions: usize,
        created_schools: usize,
        created_series: usize,
        created_keywords: usize,
        errors: Vec<crate::logic::full_import::EntityImportError>,
    }
}

impl_to_schema! {
    struct crate::logic::full_import::EntityImportError => "EntityImportError" {
        entity_type: crate::logic::full_import::NamedEntityKind,
        name: String,
        error: String,
    }
}

// =============================================================================
// LaTeX conversion report types
// =============================================================================

impl_to_schema! {
    struct crate::logic::full_import::LatexConvertReport => "LatexConvertReport" {
        columns: Vec<crate::logic::full_import::ColumnConvertResult>,
        total_updated: usize,
        errors: Vec<crate::logic::full_import::LatexConvertError>,
        missing_citation_keys: Vec<String>,
    }
}

impl_to_schema! {
    struct crate::logic::full_import::ColumnConvertResult => "ColumnConvertResult" {
        entity: &'static str,
        field: &'static str,
        updated: usize,
    }
}

impl_to_schema! {
    struct crate::logic::full_import::LatexConvertError => "LatexConvertError" {
        entity: &'static str,
        field: &'static str,
        id: i64,
        error: String,
    }
}

// =============================================================================
// Full import report
// =============================================================================

impl_to_schema! {
    struct crate::logic::full_import::FullImportReport => "FullImportReport" {
        imported: usize,
        updated: usize,
        deleted: usize,
        failed: usize,
        skipped: usize,
        errors: Vec<crate::logic::full_import::RowError>,
    }
}

// =============================================================================
// Named entity kind enum
// =============================================================================

impl_to_schema! {
    crate::logic::full_import::NamedEntityKind => "NamedEntityKind", rename_all = "lowercase" {
        Institution,
        School,
        Series,
        Keyword,
    }
}

// =============================================================================
// Search types
// =============================================================================

impl_to_schema! {
    struct crate::logic::search::SearchRequest => "SearchRequest" {
        query: String,
        entry_type: Option<crate::domain::EntryType>,
        year_from: Option<i16>,
        year_to: Option<i16>,
        author_id: Option<i64>,
        journal_id: Option<i64>,
        epoch: Option<crate::domain::Epoch>,
        limit: i64,
        offset: i64,
    }
}

impl_to_schema! {
    struct crate::logic::search::SearchResponse => "SearchResponse" {
        results: Vec<crate::domain::BibItem>,
        total: i64,
        limit: i64,
        offset: i64,
    }
}

// =============================================================================
// Import response types
// =============================================================================

impl_to_schema! {
    struct crate::logic::import::ImportRowError => "ImportRowError" {
        row: usize,
        identifier: String,
        error: String,
    }
}

impl_to_schema! {
    struct crate::logic::import::ImportResponse => "ImportResponse" {
        imported: usize,
        updated: usize,
        failed: usize,
        errors: Vec<crate::logic::import::ImportRowError>,
    }
}

impl_to_schema! {
    struct crate::logic::import::MissingReferencesError => "MissingReferencesError" {
        error: &'static str,
        message: &'static str,
        missing_author_keys: Vec<String>,
        missing_journal_keys: Vec<String>,
        missing_publisher_keys: Vec<String>,
        missing_institution_keys: Vec<String>,
        missing_school_keys: Vec<String>,
        missing_series_keys: Vec<String>,
        missing_keyword_keys: Vec<String>,
        missing_crossref_keys: Vec<String>,
    }
}

// =============================================================================
// Export request types
// =============================================================================

impl_to_schema! {
    crate::logic::export::ExportFormat => "ExportFormat", rename_all = "lowercase" {
        Expanded,
        Ids,
    }
}

impl_to_schema! {
    struct crate::logic::export::EntityExportRequest => "EntityExportRequest" {
        all: bool,
        ids: Option<Vec<i64>>,
        keys: Option<Vec<String>>,
    }
}

impl_to_schema! {
    struct crate::logic::export::BibitemExportRequest => "BibitemExportRequest" {
        format: crate::logic::export::ExportFormat,
        all: bool,
        ids: Option<Vec<i64>>,
        bibkeys: Option<Vec<String>>,
    }
}

// =============================================================================
// Compute start pages report (process layer)
// =============================================================================

impl_to_schema! {
    struct crate::process::compute_start_pages::ComputeStartPagesReport => "ComputeStartPagesReport" {
        updated: usize,
    }
}
