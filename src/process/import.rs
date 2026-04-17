//! Import process — orchestrates CSV parsing, validation, and entity insertion.
//!
//! Receives I/O dependencies as params (DataStore, trait impls). No AppState.
//! Pure types and helpers are imported from `crate::logic::import`.
//!
//! **Architecture:** This module defines WHAT operations are needed via traits.
//! Concrete I/O implementations live in `crate::adapters::import`.

use std::collections::HashSet;
use std::future::Future;

use crate::domain::{
    AuthorRole, CreateAuthor, CreateBibItem, CreateInstitution, CreateJournal, CreateKeyword,
    CreatePublisher, CreateSchool, CreateSeries, EntryType, UpdateAuthor, UpdateInstitution,
    UpdateJournal, UpdateKeyword, UpdatePublisher, UpdateSchool, UpdateSeries,
    create_author_transform, create_bib_item_transform, create_institution_transform,
    create_journal_transform, create_keyword_transform, create_publisher_transform,
    create_school_transform, create_series_transform, update_author_transform,
    update_bib_item_transform, update_institution_transform, update_journal_transform,
    update_keyword_transform, update_publisher_transform, update_school_transform,
    update_series_transform,
};
use crate::logic::import::{
    BibitemImportResult, ImportResponse, ImportRowError, MissingReferencesError, NameVariantType,
    build_bibitem_update_dto, column_index, format_insert_error, get_field, parse_i16_field,
    parse_i64_field, parse_id_list, require_column,
};
use crate::validation::{
    validate_create_author, validate_create_bibitem, validate_create_institution,
    validate_create_journal, validate_create_keyword, validate_create_publisher,
    validate_create_school, validate_create_series, validate_update_author,
    validate_update_institution, validate_update_journal, validate_update_keyword,
    validate_update_publisher, validate_update_school, validate_update_series,
};
use hexforge::{DataSource, HexforgeError, ValidationError};

// =============================================================================
// Traits — contracts for I/O operations that adapters implement
// =============================================================================

/// Contract for author name variant operations.
pub trait NameVariantStore: Send + Sync {
    /// Append a name variant to an author's variant array.
    fn append_variant(
        &self,
        author_id: i64,
        variant: &str,
        variant_type: &NameVariantType,
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;
}

/// Contract for bibitem junction operations (author and keyword links).
pub trait BibitemJunctionStore: Send + Sync {
    /// Insert or update a bibitem-author junction record.
    fn insert_author_junction(
        &self,
        bibitem_id: i64,
        author_id: i64,
        role: &AuthorRole,
        position: i16,
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;

    /// Insert a bibitem-keyword junction record (no-op on conflict).
    fn insert_keyword_junction(
        &self,
        bibitem_id: i64,
        keyword_id: i64,
        keyword_level: i16,
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;

    /// Look up keyword levels for the given keyword IDs.
    /// Returns pairs of (keyword_id, level).
    fn find_keyword_levels(
        &self,
        keyword_ids: &[i64],
    ) -> impl Future<Output = Result<Vec<(i64, i16)>, HexforgeError>> + Send;
}

/// Contract for reference validation (checking that referenced IDs exist).
pub trait ReferenceStore: Send + Sync {
    fn find_missing_author_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> impl Future<Output = Result<Vec<i64>, HexforgeError>> + Send;

    fn find_missing_journal_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> impl Future<Output = Result<Vec<i64>, HexforgeError>> + Send;

    fn find_missing_publisher_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> impl Future<Output = Result<Vec<i64>, HexforgeError>> + Send;

    fn find_missing_institution_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> impl Future<Output = Result<Vec<i64>, HexforgeError>> + Send;

    fn find_missing_school_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> impl Future<Output = Result<Vec<i64>, HexforgeError>> + Send;

    fn find_missing_series_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> impl Future<Output = Result<Vec<i64>, HexforgeError>> + Send;

    fn find_missing_keyword_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> impl Future<Output = Result<Vec<i64>, HexforgeError>> + Send;

    fn find_missing_bibitem_ids(
        &self,
        ids: &HashSet<i64>,
    ) -> impl Future<Output = Result<Vec<i64>, HexforgeError>> + Send;
}

// =============================================================================
// Author import
// =============================================================================

/// Import authors from CSV bytes.
pub async fn import_authors_from_csv(
    author_ds: &impl DataSource<crate::domain::Author, Id = i64, Error = hexforge::DataSourceError>,
    data: Vec<u8>,
    auto_assign_ids: bool,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

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

    let mut imported = 0usize;
    let mut updated = 0usize;
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

        let csv_id = col_id.and_then(|i| parse_i64_field(&record, i));

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
                        .collect::<Vec<_>>()
                })
            })
        };
        let name_variants_latex = parse_variants(col_name_variants_latex);
        let name_variants_unicode = parse_variants(col_name_variants_unicode);

        // Check if this is an update (CSV has ID that exists in DB)
        if let Some(id) = csv_id {
            match author_ds.find_by_id(&id).await {
                Ok(Some(existing)) => {
                    if existing.author_key != author_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', CSV has '{author_key}'",
                            existing.author_key
                        );
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: author_key,
                            error: msg,
                        });
                        continue;
                    }
                    let update_dto = UpdateAuthor {
                        author_key: Some(author_key.clone()),
                        given_name_latex: col_given_name_latex.and_then(|i| get_field(&record, i)),
                        given_name_unicode: col_given_name_unicode
                            .and_then(|i| get_field(&record, i)),
                        family_name_latex: col_family_name_latex
                            .and_then(|i| get_field(&record, i)),
                        family_name_unicode: col_family_name_unicode
                            .and_then(|i| get_field(&record, i)),
                        mononym_latex: col_mononym_latex.and_then(|i| get_field(&record, i)),
                        mononym_unicode: col_mononym_unicode.and_then(|i| get_field(&record, i)),
                        shorthand_latex: col_shorthand_latex.and_then(|i| get_field(&record, i)),
                        shorthand_unicode: col_shorthand_unicode
                            .and_then(|i| get_field(&record, i)),
                        famous_name_latex: col_famous_name_latex
                            .and_then(|i| get_field(&record, i)),
                        famous_name_unicode: col_famous_name_unicode
                            .and_then(|i| get_field(&record, i)),
                        name_variants_latex: name_variants_latex.clone(),
                        name_variants_unicode: name_variants_unicode.clone(),
                    };
                    if let Err(e) = validate_update_author(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: author_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_author_transform(update_dto, existing);
                    match author_ds.update(&id, merged).await {
                        Ok(_) => updated += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: author_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Ok(None) => {
                    // ID not in DB — create with this ID
                    let dto = CreateAuthor {
                        author_key: author_key.clone(),
                        given_name_latex: col_given_name_latex.and_then(|i| get_field(&record, i)),
                        given_name_unicode: col_given_name_unicode
                            .and_then(|i| get_field(&record, i)),
                        family_name_latex: col_family_name_latex
                            .and_then(|i| get_field(&record, i)),
                        family_name_unicode: col_family_name_unicode
                            .and_then(|i| get_field(&record, i)),
                        mononym_latex: col_mononym_latex.and_then(|i| get_field(&record, i)),
                        mononym_unicode: col_mononym_unicode.and_then(|i| get_field(&record, i)),
                        shorthand_latex: col_shorthand_latex.and_then(|i| get_field(&record, i)),
                        shorthand_unicode: col_shorthand_unicode
                            .and_then(|i| get_field(&record, i)),
                        famous_name_latex: col_famous_name_latex
                            .and_then(|i| get_field(&record, i)),
                        famous_name_unicode: col_famous_name_unicode
                            .and_then(|i| get_field(&record, i)),
                        name_variants_latex: name_variants_latex.clone(),
                        name_variants_unicode: name_variants_unicode.clone(),
                    };
                    if let Err(e) = validate_create_author(&dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: author_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let mut entity = create_author_transform(dto);
                    entity.id = id;
                    match author_ds.insert_with_id(entity).await {
                        Ok(_) => imported += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: author_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Err(e) => {
                    errors.push(ImportRowError {
                        row: row_num,
                        identifier: author_key,
                        error: format!("DB lookup error: {e}"),
                    });
                    continue;
                }
            }
        }

        // No ID
        if !auto_assign_ids {
            errors.push(ImportRowError {
                row: row_num,
                identifier: author_key,
                error: "Missing id (use ?auto_assign_ids=true to auto-assign)".to_string(),
            });
            continue;
        }

        let dto = CreateAuthor {
            author_key: author_key.clone(),
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
            name_variants_latex,
            name_variants_unicode,
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
        match author_ds.insert(entity).await {
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
        updated,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Journal import
// =============================================================================

/// Import journals from CSV bytes.
pub async fn import_journals_from_csv(
    journal_ds: &impl DataSource<crate::domain::Journal, Id = i64, Error = hexforge::DataSourceError>,
    data: Vec<u8>,
    auto_assign_ids: bool,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    let col_id = column_index(&headers, "id");
    let col_journal_key = require_column(&headers, "journal_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_issn_print = column_index(&headers, "issn_print");
    let col_issn_electronic = column_index(&headers, "issn_electronic");

    let mut imported = 0usize;
    let mut updated = 0usize;
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

        let csv_id = col_id.and_then(|i| parse_i64_field(&record, i));

        // Check if this is an update (CSV has ID that exists in DB)
        if let Some(id) = csv_id {
            match journal_ds.find_by_id(&id).await {
                Ok(Some(existing)) => {
                    if existing.journal_key != journal_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', CSV has '{journal_key}'",
                            existing.journal_key
                        );
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: journal_key,
                            error: msg,
                        });
                        continue;
                    }
                    let update_dto = UpdateJournal {
                        journal_key: Some(journal_key.clone()),
                        name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
                        name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
                        issn_print: col_issn_print.and_then(|i| get_field(&record, i)),
                        issn_electronic: col_issn_electronic.and_then(|i| get_field(&record, i)),
                    };
                    if let Err(e) = validate_update_journal(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: journal_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_journal_transform(update_dto, existing);
                    match journal_ds.update(&id, merged).await {
                        Ok(_) => updated += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: journal_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Ok(None) => {
                    // ID not in DB — create with this ID
                    let dto = CreateJournal {
                        journal_key: journal_key.clone(),
                        name_latex: col_name_latex
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
                        name_unicode: col_name_unicode
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
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
                    let mut entity = create_journal_transform(dto);
                    entity.id = id;
                    match journal_ds.insert_with_id(entity).await {
                        Ok(_) => imported += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: journal_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Err(e) => {
                    errors.push(ImportRowError {
                        row: row_num,
                        identifier: journal_key,
                        error: format!("DB lookup error: {e}"),
                    });
                    continue;
                }
            }
        }

        // No ID
        if !auto_assign_ids {
            errors.push(ImportRowError {
                row: row_num,
                identifier: journal_key,
                error: "Missing id (use ?auto_assign_ids=true to auto-assign)".to_string(),
            });
            continue;
        }

        let dto = CreateJournal {
            journal_key: journal_key.clone(),
            name_latex: col_name_latex
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
            name_unicode: col_name_unicode
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
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
        match journal_ds.insert(entity).await {
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
        updated,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Publisher import
// =============================================================================

/// Import publishers from CSV bytes.
pub async fn import_publishers_from_csv(
    publisher_ds: &impl DataSource<
        crate::domain::Publisher,
        Id = i64,
        Error = hexforge::DataSourceError,
    >,
    data: Vec<u8>,
    auto_assign_ids: bool,
) -> Result<ImportResponse, HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .clone();

    let col_id = column_index(&headers, "id");
    let col_publisher_key = require_column(&headers, "publisher_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_default_address = column_index(&headers, "default_address");

    let mut imported = 0usize;
    let mut updated = 0usize;
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

        let csv_id = col_id.and_then(|i| parse_i64_field(&record, i));

        // Check if this is an update (CSV has ID that exists in DB)
        if let Some(id) = csv_id {
            match publisher_ds.find_by_id(&id).await {
                Ok(Some(existing)) => {
                    if existing.publisher_key != publisher_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', CSV has '{publisher_key}'",
                            existing.publisher_key
                        );
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: publisher_key,
                            error: msg,
                        });
                        continue;
                    }
                    let update_dto = UpdatePublisher {
                        publisher_key: Some(publisher_key.clone()),
                        name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
                        name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
                        default_address: col_default_address.and_then(|i| get_field(&record, i)),
                    };
                    if let Err(e) = validate_update_publisher(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: publisher_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_publisher_transform(update_dto, existing);
                    match publisher_ds.update(&id, merged).await {
                        Ok(_) => updated += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: publisher_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Ok(None) => {
                    // ID not in DB — create with this ID
                    let dto = CreatePublisher {
                        publisher_key: publisher_key.clone(),
                        name_latex: col_name_latex
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
                        name_unicode: col_name_unicode
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
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
                    let mut entity = create_publisher_transform(dto);
                    entity.id = id;
                    match publisher_ds.insert_with_id(entity).await {
                        Ok(_) => imported += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: publisher_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Err(e) => {
                    errors.push(ImportRowError {
                        row: row_num,
                        identifier: publisher_key,
                        error: format!("DB lookup error: {e}"),
                    });
                    continue;
                }
            }
        }

        // No ID
        if !auto_assign_ids {
            errors.push(ImportRowError {
                row: row_num,
                identifier: publisher_key,
                error: "Missing id (use ?auto_assign_ids=true to auto-assign)".to_string(),
            });
            continue;
        }

        let dto = CreatePublisher {
            publisher_key: publisher_key.clone(),
            name_latex: col_name_latex
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
            name_unicode: col_name_unicode
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
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
        match publisher_ds.insert(entity).await {
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
        updated,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Institution import
// =============================================================================

/// Import institutions from CSV bytes.
pub async fn import_institutions_from_csv(
    institution_ds: &impl DataSource<
        crate::domain::Institution,
        Id = i64,
        Error = hexforge::DataSourceError,
    >,
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

    let col_id = column_index(&headers, "id");
    let col_institution_key = require_column(&headers, "institution_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_default_address = column_index(&headers, "default_address");

    let mut imported = 0usize;
    let mut updated = 0usize;
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

        let csv_id = col_id.and_then(|i| parse_i64_field(&record, i));

        // Check if this is an update (CSV has ID that exists in DB)
        if let Some(id) = csv_id {
            match institution_ds.find_by_id(&id).await {
                Ok(Some(existing)) => {
                    if existing.institution_key != institution_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', CSV has '{institution_key}'",
                            existing.institution_key
                        );
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: institution_key,
                            error: msg,
                        });
                        continue;
                    }
                    let update_dto = UpdateInstitution {
                        institution_key: Some(institution_key.clone()),
                        name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
                        name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
                        default_address: col_default_address.and_then(|i| get_field(&record, i)),
                    };
                    if let Err(e) = validate_update_institution(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: institution_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_institution_transform(update_dto, existing);
                    match institution_ds.update(&id, merged).await {
                        Ok(_) => updated += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: institution_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Ok(None) => {
                    // ID not in DB — create with this ID
                    let dto = CreateInstitution {
                        institution_key: institution_key.clone(),
                        name_latex: col_name_latex
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
                        name_unicode: col_name_unicode
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
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
                    let mut entity = create_institution_transform(dto);
                    entity.id = id;
                    match institution_ds.insert_with_id(entity).await {
                        Ok(_) => imported += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: institution_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Err(e) => {
                    errors.push(ImportRowError {
                        row: row_num,
                        identifier: institution_key,
                        error: format!("DB lookup error: {e}"),
                    });
                    continue;
                }
            }
        }

        // No ID — create normally
        let dto = CreateInstitution {
            institution_key: institution_key.clone(),
            name_latex: col_name_latex
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
            name_unicode: col_name_unicode
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
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
        match institution_ds.insert(entity).await {
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
        updated,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// School import
// =============================================================================

/// Import schools from CSV bytes.
pub async fn import_schools_from_csv(
    school_ds: &impl DataSource<crate::domain::School, Id = i64, Error = hexforge::DataSourceError>,
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

    let col_id = column_index(&headers, "id");
    let col_school_key = require_column(&headers, "school_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");
    let col_default_address = column_index(&headers, "default_address");

    let mut imported = 0usize;
    let mut updated = 0usize;
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

        let csv_id = col_id.and_then(|i| parse_i64_field(&record, i));

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

        // Check if this is an update (CSV has ID that exists in DB)
        if let Some(id) = csv_id {
            match school_ds.find_by_id(&id).await {
                Ok(Some(existing)) => {
                    if existing.school_key != school_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', CSV has '{school_key}'",
                            existing.school_key
                        );
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: school_key,
                            error: msg,
                        });
                        continue;
                    }
                    let update_dto = UpdateSchool {
                        school_key: Some(school_key.clone()),
                        name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
                        name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
                        default_address: col_default_address.and_then(|i| get_field(&record, i)),
                    };
                    if let Err(e) = validate_update_school(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: school_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_school_transform(update_dto, existing);
                    match school_ds.update(&id, merged).await {
                        Ok(_) => updated += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: school_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Ok(None) => {
                    // ID not in DB — create with this ID
                    let dto = CreateSchool {
                        school_key: school_key.clone(),
                        name_latex: col_name_latex
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
                        name_unicode: col_name_unicode
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
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
                    let mut entity = create_school_transform(dto);
                    entity.id = id;
                    match school_ds.insert_with_id(entity).await {
                        Ok(_) => imported += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: school_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Err(e) => {
                    errors.push(ImportRowError {
                        row: row_num,
                        identifier: school_key,
                        error: format!("DB lookup error: {e}"),
                    });
                    continue;
                }
            }
        }

        // No ID — create normally
        let dto = CreateSchool {
            school_key: school_key.clone(),
            name_latex: col_name_latex
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
            name_unicode: col_name_unicode
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
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
        match school_ds.insert(entity).await {
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
        updated,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Series import
// =============================================================================

/// Import series from CSV bytes.
pub async fn import_series_from_csv(
    series_ds: &impl DataSource<crate::domain::Series, Id = i64, Error = hexforge::DataSourceError>,
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

    let col_id = column_index(&headers, "id");
    let col_series_key = require_column(&headers, "series_key")?;
    let col_name_latex = column_index(&headers, "name_latex");
    let col_name_unicode = column_index(&headers, "name_unicode");

    let mut imported = 0usize;
    let mut updated = 0usize;
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

        let csv_id = col_id.and_then(|i| parse_i64_field(&record, i));

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

        // Check if this is an update (CSV has ID that exists in DB)
        if let Some(id) = csv_id {
            match series_ds.find_by_id(&id).await {
                Ok(Some(existing)) => {
                    if existing.series_key != series_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', CSV has '{series_key}'",
                            existing.series_key
                        );
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: series_key,
                            error: msg,
                        });
                        continue;
                    }
                    let update_dto = UpdateSeries {
                        series_key: Some(series_key.clone()),
                        name_latex: col_name_latex.and_then(|i| get_field(&record, i)),
                        name_unicode: col_name_unicode.and_then(|i| get_field(&record, i)),
                    };
                    if let Err(e) = validate_update_series(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: series_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_series_transform(update_dto, existing);
                    match series_ds.update(&id, merged).await {
                        Ok(_) => updated += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: series_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Ok(None) => {
                    // ID not in DB — create with this ID
                    let dto = CreateSeries {
                        series_key: series_key.clone(),
                        name_latex: col_name_latex
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
                        name_unicode: col_name_unicode
                            .and_then(|i| get_field(&record, i))
                            .unwrap_or_default(),
                    };
                    if let Err(e) = validate_create_series(&dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: series_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let mut entity = create_series_transform(dto);
                    entity.id = id;
                    match series_ds.insert_with_id(entity).await {
                        Ok(_) => imported += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: series_key,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Err(e) => {
                    errors.push(ImportRowError {
                        row: row_num,
                        identifier: series_key,
                        error: format!("DB lookup error: {e}"),
                    });
                    continue;
                }
            }
        }

        // No ID — create normally
        let dto = CreateSeries {
            series_key: series_key.clone(),
            name_latex: col_name_latex
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
            name_unicode: col_name_unicode
                .and_then(|i| get_field(&record, i))
                .unwrap_or_default(),
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
        match series_ds.insert(entity).await {
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
        updated,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Keyword import
// =============================================================================

/// Import keywords from CSV bytes.
pub async fn import_keywords_from_csv(
    keyword_ds: &impl DataSource<crate::domain::Keyword, Id = i64, Error = hexforge::DataSourceError>,
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

    let col_id = column_index(&headers, "id");
    let col_name = require_column(&headers, "name")?;
    let col_level = require_column(&headers, "level")?;

    let mut imported = 0usize;
    let mut updated = 0usize;
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

        let csv_id = col_id.and_then(|i| parse_i64_field(&record, i));

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

        // Check if this is an update (CSV has ID that exists in DB)
        if let Some(id) = csv_id {
            match keyword_ds.find_by_id(&id).await {
                Ok(Some(existing)) => {
                    if existing.name != name {
                        let msg = format!(
                            "ID {id} exists but has key '{}', CSV has '{name}'",
                            existing.name
                        );
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: name,
                            error: msg,
                        });
                        continue;
                    }
                    let update_dto = UpdateKeyword {
                        name: Some(name.clone()),
                        level: Some(level),
                    };
                    if let Err(e) = validate_update_keyword(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: name,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_keyword_transform(update_dto, existing);
                    match keyword_ds.update(&id, merged).await {
                        Ok(_) => updated += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: name,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Ok(None) => {
                    // ID not in DB — create with this ID
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
                    let mut entity = create_keyword_transform(dto);
                    entity.id = id;
                    match keyword_ds.insert_with_id(entity).await {
                        Ok(_) => imported += 1,
                        Err(e) => errors.push(ImportRowError {
                            row: row_num,
                            identifier: name,
                            error: format_insert_error(e),
                        }),
                    }
                    continue;
                }
                Err(e) => {
                    errors.push(ImportRowError {
                        row: row_num,
                        identifier: name,
                        error: format!("DB lookup error: {e}"),
                    });
                    continue;
                }
            }
        }

        // No ID — create normally
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
        match keyword_ds.insert(entity).await {
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
        updated,
        failed: errors.len(),
        errors,
    })
}

// =============================================================================
// Author name variants import
// =============================================================================

/// Import author name variants from CSV bytes.
///
/// CSV format: `name_variant,type,profile_id`
/// - `type` is `latex` or `unicode`
/// - `profile_id` is the author's ID
///
/// Appends each variant to the author's `name_variants_latex` or
/// `name_variants_unicode` array. Variants must be unique per author per type.
pub async fn import_author_name_variants_from_csv(
    author_ds: &impl DataSource<crate::domain::Author, Id = i64, Error = hexforge::DataSourceError>,
    variant_store: &impl NameVariantStore,
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

    let col_name_variant = require_column(&headers, "name_variant")?;
    let col_type = require_column(&headers, "type")?;
    let col_profile_id = require_column(&headers, "profile_id")?;

    let mut updated = 0usize;
    let mut errors = Vec::new();
    let mut seen: HashSet<(i64, NameVariantType, String)> = HashSet::new();

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

        // Deduplicate within this CSV
        if !seen.insert((profile_id, variant_type.clone(), variant.clone())) {
            continue;
        }

        // Look up the author
        let existing = match author_ds.find_by_id(&profile_id).await {
            Ok(Some(a)) => a,
            Ok(None) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: variant,
                    error: format!("Author with id {profile_id} not found"),
                });
                continue;
            }
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: variant,
                    error: format!("DB lookup error: {e}"),
                });
                continue;
            }
        };

        // Append to the correct variants list (skip if already present)
        let current = match variant_type {
            NameVariantType::Latex => &existing.name_variants_latex,
            NameVariantType::Unicode => &existing.name_variants_unicode,
        };

        let already_has = current.as_ref().is_some_and(|v| v.contains(&variant));

        if already_has {
            continue;
        }

        // Append via the variant store (atomic operation)
        match variant_store
            .append_variant(profile_id, &variant, &variant_type)
            .await
        {
            Ok(()) => updated += 1,
            Err(e) => {
                errors.push(ImportRowError {
                    row: row_num,
                    identifier: variant,
                    error: format!("Failed to update author {profile_id}: {e}"),
                });
            }
        }
    }

    Ok(ImportResponse {
        imported: 0,
        updated,
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
    csv_id: Option<i64>,
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
    bibitem_ds: &impl DataSource<crate::domain::BibItem, Id = i64, Error = hexforge::DataSourceError>,
    junction_store: &impl BibitemJunctionStore,
    ref_store: &impl ReferenceStore,
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

        let csv_id = col_id.and_then(|i| parse_i64_field(&record, i));

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
        let entry_type: EntryType = match entry_type_str.parse() {
            Ok(et) => et,
            Err(_) if entry_type_str.is_empty() => EntryType::Unknown,
            Err(_) => {
                parse_errors.push(ImportRowError {
                    row: row_num,
                    identifier: bibkey,
                    error: format!("Invalid entry_type: '{entry_type_str}'"),
                });
                continue;
            }
        };

        let title_latex = col_title_latex
            .and_then(|i| get_field(&record, i))
            .unwrap_or_default();
        let title_unicode = col_title_unicode
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

        // Validate is_translation: absent/empty -> false, known bool -> value, else error
        let is_translation = match col_is_translation.and_then(|i| get_field(&record, i)) {
            None => false,
            Some(raw) => {
                match raw.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "y" | "x" => true,
                    "false" | "0" | "no" | "n" => false,
                    _ => {
                        parse_errors.push(ImportRowError {
                        row: row_num,
                        identifier: bibkey,
                        error: format!("Invalid is_translation value: '{raw}' (expected true/false/yes/no/1/0)"),
                    });
                        continue;
                    }
                }
            }
        };

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
            csv_id,
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
            updated: 0,
            failed: parse_errors.len(),
            errors: parse_errors,
        }));
    }

    if parsed_rows.is_empty() {
        return Ok(BibitemImportResult::Success(ImportResponse {
            imported: 0,
            updated: 0,
            failed: 0,
            errors: vec![],
        }));
    }

    // Phase 2: Batch-check all referenced IDs exist
    let missing = check_all_references(
        ref_store,
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

    // Phase 3: Insert/update bibitems and their junction data
    let mut imported = 0usize;
    let mut updated = 0usize;
    let mut insert_errors = Vec::new();

    for row in &parsed_rows {
        // Determine the bibitem ID: update existing, insert-with-id, or insert new
        let (bibitem_id, is_update) = if let Some(id) = row.csv_id {
            match bibitem_ds.find_by_id(&id).await {
                Ok(Some(existing)) => {
                    if existing.bibkey != row.bibkey {
                        insert_errors.push(ImportRowError {
                            row: row.row_num,
                            identifier: row.bibkey.clone(),
                            error: format!(
                                "ID {id} exists but has bibkey '{}', CSV has '{}'",
                                existing.bibkey, row.bibkey
                            ),
                        });
                        continue;
                    }
                    // Update existing bibitem
                    let merged =
                        update_bib_item_transform(build_bibitem_update_dto(&row.dto), existing);
                    match bibitem_ds.update(&id, merged).await {
                        Ok(Some(_)) => (id, true),
                        Ok(None) => {
                            insert_errors.push(ImportRowError {
                                row: row.row_num,
                                identifier: row.bibkey.clone(),
                                error: format!("Failed to update bibitem {id}: not found"),
                            });
                            continue;
                        }
                        Err(e) => {
                            insert_errors.push(ImportRowError {
                                row: row.row_num,
                                identifier: row.bibkey.clone(),
                                error: format_insert_error(e),
                            });
                            continue;
                        }
                    }
                }
                Ok(None) => {
                    // ID not in DB — insert with this ID
                    let mut bibitem = create_bib_item_transform(row.dto.clone());
                    bibitem.id = id;
                    match bibitem_ds.insert_with_id(bibitem).await {
                        Ok(inserted) => (inserted.id, false),
                        Err(e) => {
                            insert_errors.push(ImportRowError {
                                row: row.row_num,
                                identifier: row.bibkey.clone(),
                                error: format_insert_error(e),
                            });
                            continue;
                        }
                    }
                }
                Err(e) => {
                    insert_errors.push(ImportRowError {
                        row: row.row_num,
                        identifier: row.bibkey.clone(),
                        error: format!("DB lookup error: {e}"),
                    });
                    continue;
                }
            }
        } else {
            // No CSV ID — insert normally
            let bibitem = create_bib_item_transform(row.dto.clone());
            match bibitem_ds.insert(bibitem).await {
                Ok(inserted) => (inserted.id, false),
                Err(e) => {
                    insert_errors.push(ImportRowError {
                        row: row.row_num,
                        identifier: row.bibkey.clone(),
                        error: format_insert_error(e),
                    });
                    continue;
                }
            }
        };

        // Insert/re-insert junction data
        if let Err(e) = insert_bibitem_authors(
            junction_store,
            bibitem_id,
            &row.author_ids,
            AuthorRole::Author,
        )
        .await
        {
            insert_errors.push(ImportRowError {
                row: row.row_num,
                identifier: row.bibkey.clone(),
                error: format!("Failed to link authors: {e}"),
            });
            continue;
        }
        if let Err(e) = insert_bibitem_authors(
            junction_store,
            bibitem_id,
            &row.editor_ids,
            AuthorRole::Editor,
        )
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
            junction_store,
            bibitem_id,
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
        if let Err(e) = insert_bibitem_keywords(junction_store, bibitem_id, &row.keyword_ids).await
        {
            insert_errors.push(ImportRowError {
                row: row.row_num,
                identifier: row.bibkey.clone(),
                error: format!("Failed to link keywords: {e}"),
            });
            continue;
        }

        if is_update {
            updated += 1;
        } else {
            imported += 1;
        }
    }

    Ok(BibitemImportResult::Success(ImportResponse {
        imported,
        updated,
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
    ref_store: &impl ReferenceStore,
    author_ids: &HashSet<i64>,
    journal_ids: &HashSet<i64>,
    publisher_ids: &HashSet<i64>,
    institution_ids: &HashSet<i64>,
    school_ids: &HashSet<i64>,
    series_ids: &HashSet<i64>,
    keyword_ids: &HashSet<i64>,
    crossref_ids: &HashSet<i64>,
) -> Result<MissingReferencesError, HexforgeError> {
    let missing_authors = ref_store.find_missing_author_ids(author_ids).await?;
    let missing_journals = ref_store.find_missing_journal_ids(journal_ids).await?;
    let missing_publishers = ref_store.find_missing_publisher_ids(publisher_ids).await?;
    let missing_institutions = ref_store
        .find_missing_institution_ids(institution_ids)
        .await?;
    let missing_schools = ref_store.find_missing_school_ids(school_ids).await?;
    let missing_series = ref_store.find_missing_series_ids(series_ids).await?;
    let missing_keywords = ref_store.find_missing_keyword_ids(keyword_ids).await?;
    let missing_crossrefs = ref_store.find_missing_bibitem_ids(crossref_ids).await?;

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

// =============================================================================
// Junction table insertion helpers
// =============================================================================

/// Insert bibitem-author junction records via the junction store.
async fn insert_bibitem_authors(
    junction_store: &impl BibitemJunctionStore,
    bibitem_id: i64,
    author_ids: &[i64],
    role: AuthorRole,
) -> Result<(), HexforgeError> {
    for (position, &author_id) in author_ids.iter().enumerate() {
        let pos = i16::try_from(position).map_err(|_| {
            HexforgeError::Validation(ValidationError::custom(format!(
                "author position {position} exceeds i16 range"
            )))
        })?;
        junction_store
            .insert_author_junction(bibitem_id, author_id, &role, pos)
            .await?;
    }
    Ok(())
}

/// Insert bibitem-keyword junction records via the junction store.
/// Keywords are looked up to determine their level.
async fn insert_bibitem_keywords(
    junction_store: &impl BibitemJunctionStore,
    bibitem_id: i64,
    keyword_ids: &[i64],
) -> Result<(), HexforgeError> {
    if keyword_ids.is_empty() {
        return Ok(());
    }

    // Fetch keyword levels via the junction store
    let kw_levels = junction_store.find_keyword_levels(keyword_ids).await?;

    for (kw_id, level) in &kw_levels {
        junction_store
            .insert_keyword_junction(bibitem_id, *kw_id, *level)
            .await?;
    }

    Ok(())
}
