//! Import process — orchestrates row parsing, validation, and entity insertion.
//!
//! Receives I/O dependencies as params (DataStore, trait impls). No AppState.
//! Pure types and helpers are imported from `crate::logic::import`.
//!
//! **Architecture:** This module defines WHAT operations are needed via traits.
//! Concrete I/O implementations live in `crate::adapters::import`.

use std::collections::{HashMap, HashSet};
use std::future::Future;

use crate::domain::{
    AuthorRole, CreateAuthor, CreateInstitution, CreateJournal, CreateKeyword, CreatePublisher,
    CreateSchool, CreateSeries, UpdateAuthor, UpdateInstitution, UpdateJournal, UpdateKeyword,
    UpdatePublisher, UpdateSchool, UpdateSeries, create_author_transform,
    create_bib_item_transform, create_institution_transform, create_journal_transform,
    create_keyword_transform, create_publisher_transform, create_school_transform,
    create_series_transform, update_author_transform, update_bib_item_transform,
    update_institution_transform, update_journal_transform, update_keyword_transform,
    update_publisher_transform, update_school_transform, update_series_transform,
};
use crate::logic::import::{
    BibitemImportResult, ImportResponse, ImportRowError, MissingReferencesError, NameVariantType,
    ParsedAuthorRow, ParsedBibitemNotesRow, ParsedBibitemRefRow, ParsedBibitemRow,
    ParsedInstitutionRow, ParsedJournalRow, ParsedKeywordRow, ParsedNameVariantRow,
    ParsedPublisherRow, ParsedSchoolRow, ParsedSeriesRow, build_bibitem_update_dto,
    format_insert_error,
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

/// Contract for batch-fetching entities by IDs.
pub trait EntityBatchLookup<T>: Send + Sync {
    fn find_by_ids(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<Vec<T>, HexforgeError>> + Send;
}

/// Contract for syncing a PostgreSQL sequence to the current max ID after bulk insert.
pub trait SequenceSyncer: Send + Sync {
    /// Advance the sequence for `table` to at least MAX(id).
    fn sync_sequence(
        &self,
        table: &'static str,
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;
}

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
        bibkey: &str,
        author_key: &str,
        role: &AuthorRole,
        position: i16,
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;

    /// Insert a bibitem-keyword junction record (no-op on conflict).
    fn insert_keyword_junction(
        &self,
        bibkey: &str,
        keyword_key: &str,
        keyword_level: i16,
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;

    /// Look up keyword levels for the given keyword keys.
    /// Returns pairs of (keyword_key, level).
    fn find_keyword_levels(
        &self,
        keyword_keys: &[String],
    ) -> impl Future<Output = Result<Vec<(String, i16)>, HexforgeError>> + Send;
}

/// Contract for reference validation (checking that referenced keys exist).
pub trait ReferenceStore: Send + Sync {
    fn find_missing_author_keys(
        &self,
        keys: &HashSet<String>,
    ) -> impl Future<Output = Result<Vec<String>, HexforgeError>> + Send;

    fn find_missing_journal_keys(
        &self,
        keys: &HashSet<String>,
    ) -> impl Future<Output = Result<Vec<String>, HexforgeError>> + Send;

    fn find_missing_publisher_keys(
        &self,
        keys: &HashSet<String>,
    ) -> impl Future<Output = Result<Vec<String>, HexforgeError>> + Send;

    fn find_missing_institution_keys(
        &self,
        keys: &HashSet<String>,
    ) -> impl Future<Output = Result<Vec<String>, HexforgeError>> + Send;

    fn find_missing_school_keys(
        &self,
        keys: &HashSet<String>,
    ) -> impl Future<Output = Result<Vec<String>, HexforgeError>> + Send;

    fn find_missing_series_keys(
        &self,
        keys: &HashSet<String>,
    ) -> impl Future<Output = Result<Vec<String>, HexforgeError>> + Send;

    fn find_missing_keyword_keys(
        &self,
        keys: &HashSet<String>,
    ) -> impl Future<Output = Result<Vec<String>, HexforgeError>> + Send;

    fn find_missing_bibitem_keys(
        &self,
        keys: &HashSet<String>,
    ) -> impl Future<Output = Result<Vec<String>, HexforgeError>> + Send;
}

// =============================================================================
// Author import
// =============================================================================

/// Import authors from parsed rows.
pub async fn import_authors(
    author_ds: &impl DataSource<crate::domain::Author, Id = i64, Error = hexforge::DataSourceError>,
    batch_lookup: &impl EntityBatchLookup<crate::domain::Author>,
    syncer: &impl SequenceSyncer,
    rows: Vec<ParsedAuthorRow>,
    mut errors: Vec<ImportRowError>,
    auto_assign_ids: bool,
) -> Result<ImportResponse, HexforgeError> {
    let mut imported = 0usize;
    let mut updated = 0usize;

    let source_ids: Vec<i64> = rows.iter().filter_map(|r| r.source_id).collect();
    let existing_map: HashMap<i64, crate::domain::Author> = batch_lookup
        .find_by_ids(&source_ids)
        .await?
        .into_iter()
        .map(|e| (e.id, e))
        .collect();

    for row in rows {
        let row_num = row.row_num;
        let author_key = row.author_key;

        if let Some(id) = row.source_id {
            match existing_map.get(&id) {
                Some(existing) => {
                    if existing.author_key != author_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', input has '{author_key}'",
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
                        given_name_latex: row.given_name_latex.clone(),
                        given_name_unicode: row.given_name_unicode.clone(),
                        family_name_latex: row.family_name_latex.clone(),
                        family_name_unicode: row.family_name_unicode.clone(),
                        mononym_latex: row.mononym_latex.clone(),
                        mononym_unicode: row.mononym_unicode.clone(),
                        shorthand_latex: row.shorthand_latex.clone(),
                        shorthand_unicode: row.shorthand_unicode.clone(),
                        famous_name_latex: row.famous_name_latex.clone(),
                        famous_name_unicode: row.famous_name_unicode.clone(),
                        famous: Some(row.famous),
                        name_variants_latex: row.name_variants_latex.clone(),
                        name_variants_unicode: row.name_variants_unicode.clone(),
                    };
                    if let Err(e) = validate_update_author(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: author_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_author_transform(update_dto, existing.clone());
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
                None => {
                    // ID not in DB — create with this ID
                    let dto = CreateAuthor {
                        author_key: author_key.clone(),
                        given_name_latex: row.given_name_latex.clone(),
                        given_name_unicode: row.given_name_unicode.clone(),
                        family_name_latex: row.family_name_latex.clone(),
                        family_name_unicode: row.family_name_unicode.clone(),
                        mononym_latex: row.mononym_latex.clone(),
                        mononym_unicode: row.mononym_unicode.clone(),
                        shorthand_latex: row.shorthand_latex.clone(),
                        shorthand_unicode: row.shorthand_unicode.clone(),
                        famous_name_latex: row.famous_name_latex.clone(),
                        famous_name_unicode: row.famous_name_unicode.clone(),
                        famous: row.famous,
                        name_variants_latex: row.name_variants_latex.clone(),
                        name_variants_unicode: row.name_variants_unicode.clone(),
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
            given_name_latex: row.given_name_latex,
            given_name_unicode: row.given_name_unicode,
            family_name_latex: row.family_name_latex,
            family_name_unicode: row.family_name_unicode,
            mononym_latex: row.mononym_latex,
            mononym_unicode: row.mononym_unicode,
            shorthand_latex: row.shorthand_latex,
            shorthand_unicode: row.shorthand_unicode,
            famous_name_latex: row.famous_name_latex,
            famous_name_unicode: row.famous_name_unicode,
            famous: row.famous,
            name_variants_latex: row.name_variants_latex,
            name_variants_unicode: row.name_variants_unicode,
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

    syncer.sync_sequence("authors").await?;
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

/// Import journals from parsed rows.
pub async fn import_journals(
    journal_ds: &impl DataSource<crate::domain::Journal, Id = i64, Error = hexforge::DataSourceError>,
    batch_lookup: &impl EntityBatchLookup<crate::domain::Journal>,
    syncer: &impl SequenceSyncer,
    rows: Vec<ParsedJournalRow>,
    mut errors: Vec<ImportRowError>,
    auto_assign_ids: bool,
) -> Result<ImportResponse, HexforgeError> {
    let mut imported = 0usize;
    let mut updated = 0usize;

    let source_ids: Vec<i64> = rows.iter().filter_map(|r| r.source_id).collect();
    let existing_map: HashMap<i64, crate::domain::Journal> = batch_lookup
        .find_by_ids(&source_ids)
        .await?
        .into_iter()
        .map(|e| (e.id, e))
        .collect();

    for row in rows {
        let row_num = row.row_num;
        let journal_key = row.journal_key;

        if let Some(id) = row.source_id {
            match existing_map.get(&id) {
                Some(existing) => {
                    if existing.journal_key != journal_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', input has '{journal_key}'",
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
                        name_latex: row.name_latex.clone(),
                        name_unicode: row.name_unicode.clone(),
                        issn_print: row.issn_print.clone(),
                        issn_electronic: row.issn_electronic.clone(),
                    };
                    if let Err(e) = validate_update_journal(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: journal_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_journal_transform(update_dto, existing.clone());
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
                None => {
                    // ID not in DB — create with this ID
                    let dto = CreateJournal {
                        journal_key: journal_key.clone(),
                        name_latex: row.name_latex.clone().unwrap_or_default(),
                        name_unicode: row.name_unicode.clone().unwrap_or_default(),
                        issn_print: row.issn_print.clone(),
                        issn_electronic: row.issn_electronic.clone(),
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
            name_latex: row.name_latex.unwrap_or_default(),
            name_unicode: row.name_unicode.unwrap_or_default(),
            issn_print: row.issn_print,
            issn_electronic: row.issn_electronic,
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

    syncer.sync_sequence("journals").await?;
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

/// Import publishers from parsed rows.
pub async fn import_publishers(
    publisher_ds: &impl DataSource<
        crate::domain::Publisher,
        Id = i64,
        Error = hexforge::DataSourceError,
    >,
    batch_lookup: &impl EntityBatchLookup<crate::domain::Publisher>,
    syncer: &impl SequenceSyncer,
    rows: Vec<ParsedPublisherRow>,
    mut errors: Vec<ImportRowError>,
    auto_assign_ids: bool,
) -> Result<ImportResponse, HexforgeError> {
    let mut imported = 0usize;
    let mut updated = 0usize;

    let source_ids: Vec<i64> = rows.iter().filter_map(|r| r.source_id).collect();
    let existing_map: HashMap<i64, crate::domain::Publisher> = batch_lookup
        .find_by_ids(&source_ids)
        .await?
        .into_iter()
        .map(|e| (e.id, e))
        .collect();

    for row in rows {
        let row_num = row.row_num;
        let publisher_key = row.publisher_key;

        if let Some(id) = row.source_id {
            match existing_map.get(&id) {
                Some(existing) => {
                    if existing.publisher_key != publisher_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', input has '{publisher_key}'",
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
                        name_latex: row.name_latex.clone(),
                        name_unicode: row.name_unicode.clone(),
                        default_address: row.default_address.clone(),
                    };
                    if let Err(e) = validate_update_publisher(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: publisher_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_publisher_transform(update_dto, existing.clone());
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
                None => {
                    // ID not in DB — create with this ID
                    let dto = CreatePublisher {
                        publisher_key: publisher_key.clone(),
                        name_latex: row.name_latex.clone().unwrap_or_default(),
                        name_unicode: row.name_unicode.clone().unwrap_or_default(),
                        default_address: row.default_address.clone(),
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
            name_latex: row.name_latex.unwrap_or_default(),
            name_unicode: row.name_unicode.unwrap_or_default(),
            default_address: row.default_address,
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

    syncer.sync_sequence("publishers").await?;
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

/// Import institutions from parsed rows.
pub async fn import_institutions(
    institution_ds: &impl DataSource<
        crate::domain::Institution,
        Id = i64,
        Error = hexforge::DataSourceError,
    >,
    batch_lookup: &impl EntityBatchLookup<crate::domain::Institution>,
    syncer: &impl SequenceSyncer,
    rows: Vec<ParsedInstitutionRow>,
    mut errors: Vec<ImportRowError>,
) -> Result<ImportResponse, HexforgeError> {
    let mut imported = 0usize;
    let mut updated = 0usize;

    let source_ids: Vec<i64> = rows.iter().filter_map(|r| r.source_id).collect();
    let existing_map: HashMap<i64, crate::domain::Institution> = batch_lookup
        .find_by_ids(&source_ids)
        .await?
        .into_iter()
        .map(|e| (e.id, e))
        .collect();

    for row in rows {
        let row_num = row.row_num;
        let institution_key = row.institution_key;

        if let Some(id) = row.source_id {
            match existing_map.get(&id) {
                Some(existing) => {
                    if existing.institution_key != institution_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', input has '{institution_key}'",
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
                        name_latex: row.name_latex.clone(),
                        name_unicode: row.name_unicode.clone(),
                        default_address: row.default_address.clone(),
                    };
                    if let Err(e) = validate_update_institution(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: institution_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_institution_transform(update_dto, existing.clone());
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
                None => {
                    // ID not in DB — create with this ID
                    let dto = CreateInstitution {
                        institution_key: institution_key.clone(),
                        name_latex: row.name_latex.clone().unwrap_or_default(),
                        name_unicode: row.name_unicode.clone().unwrap_or_default(),
                        default_address: row.default_address.clone(),
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
            }
        }

        // No ID — create normally
        let dto = CreateInstitution {
            institution_key: institution_key.clone(),
            name_latex: row.name_latex.unwrap_or_default(),
            name_unicode: row.name_unicode.unwrap_or_default(),
            default_address: row.default_address,
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

    syncer.sync_sequence("institutions").await?;
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

/// Import schools from parsed rows.
pub async fn import_schools(
    school_ds: &impl DataSource<crate::domain::School, Id = i64, Error = hexforge::DataSourceError>,
    batch_lookup: &impl EntityBatchLookup<crate::domain::School>,
    syncer: &impl SequenceSyncer,
    rows: Vec<ParsedSchoolRow>,
    mut errors: Vec<ImportRowError>,
) -> Result<ImportResponse, HexforgeError> {
    let mut imported = 0usize;
    let mut updated = 0usize;

    let source_ids: Vec<i64> = rows.iter().filter_map(|r| r.source_id).collect();
    let existing_map: HashMap<i64, crate::domain::School> = batch_lookup
        .find_by_ids(&source_ids)
        .await?
        .into_iter()
        .map(|e| (e.id, e))
        .collect();

    for row in rows {
        let row_num = row.row_num;
        let school_key = row.school_key;

        if let Some(id) = row.source_id {
            match existing_map.get(&id) {
                Some(existing) => {
                    if existing.school_key != school_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', input has '{school_key}'",
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
                        name_latex: row.name_latex.clone(),
                        name_unicode: row.name_unicode.clone(),
                    };
                    if let Err(e) = validate_update_school(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: school_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_school_transform(update_dto, existing.clone());
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
                None => {
                    // ID not in DB — create with this ID
                    let dto = CreateSchool {
                        school_key: school_key.clone(),
                        name_latex: row.name_latex.clone().unwrap_or_default(),
                        name_unicode: row.name_unicode.clone().unwrap_or_default(),
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
            }
        }

        // No ID — create normally
        let dto = CreateSchool {
            school_key: school_key.clone(),
            name_latex: row.name_latex.unwrap_or_default(),
            name_unicode: row.name_unicode.unwrap_or_default(),
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

    syncer.sync_sequence("schools").await?;
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

/// Import series from parsed rows.
pub async fn import_series(
    series_ds: &impl DataSource<crate::domain::Series, Id = i64, Error = hexforge::DataSourceError>,
    batch_lookup: &impl EntityBatchLookup<crate::domain::Series>,
    syncer: &impl SequenceSyncer,
    rows: Vec<ParsedSeriesRow>,
    mut errors: Vec<ImportRowError>,
) -> Result<ImportResponse, HexforgeError> {
    let mut imported = 0usize;
    let mut updated = 0usize;

    let source_ids: Vec<i64> = rows.iter().filter_map(|r| r.source_id).collect();
    let existing_map: HashMap<i64, crate::domain::Series> = batch_lookup
        .find_by_ids(&source_ids)
        .await?
        .into_iter()
        .map(|e| (e.id, e))
        .collect();

    for row in rows {
        let row_num = row.row_num;
        let series_key = row.series_key;

        if let Some(id) = row.source_id {
            match existing_map.get(&id) {
                Some(existing) => {
                    if existing.series_key != series_key {
                        let msg = format!(
                            "ID {id} exists but has key '{}', input has '{series_key}'",
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
                        name_latex: row.name_latex.clone(),
                        name_unicode: row.name_unicode.clone(),
                    };
                    if let Err(e) = validate_update_series(&update_dto) {
                        errors.push(ImportRowError {
                            row: row_num,
                            identifier: series_key,
                            error: e.to_string(),
                        });
                        continue;
                    }
                    let merged = update_series_transform(update_dto, existing.clone());
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
                None => {
                    // ID not in DB — create with this ID
                    let dto = CreateSeries {
                        series_key: series_key.clone(),
                        name_latex: row.name_latex.clone().unwrap_or_default(),
                        name_unicode: row.name_unicode.clone().unwrap_or_default(),
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
            }
        }

        // No ID — create normally
        let dto = CreateSeries {
            series_key: series_key.clone(),
            name_latex: row.name_latex.unwrap_or_default(),
            name_unicode: row.name_unicode.unwrap_or_default(),
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

    syncer.sync_sequence("series").await?;
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

/// Import keywords from parsed rows.
pub async fn import_keywords(
    keyword_ds: &impl DataSource<crate::domain::Keyword, Id = i64, Error = hexforge::DataSourceError>,
    batch_lookup: &impl EntityBatchLookup<crate::domain::Keyword>,
    syncer: &impl SequenceSyncer,
    rows: Vec<ParsedKeywordRow>,
    mut errors: Vec<ImportRowError>,
) -> Result<ImportResponse, HexforgeError> {
    let mut imported = 0usize;
    let mut updated = 0usize;

    let source_ids: Vec<i64> = rows.iter().filter_map(|r| r.source_id).collect();
    let existing_map: HashMap<i64, crate::domain::Keyword> = batch_lookup
        .find_by_ids(&source_ids)
        .await?
        .into_iter()
        .map(|e| (e.id, e))
        .collect();

    for row in rows {
        let row_num = row.row_num;
        let name = row.name;
        let level = row.level;

        // Check if this is an update (input row has an existing DB ID)
        if let Some(id) = row.source_id {
            match existing_map.get(&id) {
                Some(existing) => {
                    if existing.name != name {
                        let msg = format!(
                            "ID {id} exists but has key '{}', input has '{name}'",
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
                        keyword_key: None,
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
                    let merged = update_keyword_transform(update_dto, existing.clone());
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
                None => {
                    // ID not in DB — create with this ID
                    let dto = CreateKeyword {
                        keyword_key: format!(
                            "{}:{}",
                            level,
                            crate::logic::full_import::generate_key(&name)
                        ),
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
            }
        }

        // No ID — create normally
        let dto = CreateKeyword {
            keyword_key: format!(
                "{}:{}",
                level,
                crate::logic::full_import::generate_key(&name)
            ),
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

    syncer.sync_sequence("keywords").await?;
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

/// Import author name variants from parsed rows.
///
/// Appends each variant to the author's `name_variants_latex` or
/// `name_variants_unicode` array. Variants must be unique per author per type.
pub async fn import_author_name_variants(
    author_ds: &impl DataSource<crate::domain::Author, Id = i64, Error = hexforge::DataSourceError>,
    variant_store: &impl NameVariantStore,
    rows: Vec<ParsedNameVariantRow>,
    mut errors: Vec<ImportRowError>,
) -> Result<ImportResponse, HexforgeError> {
    let mut updated = 0usize;
    let mut seen: HashSet<(i64, NameVariantType, String)> = HashSet::new();

    for row in rows {
        let row_num = row.row_num;
        let profile_id = row.profile_id;
        let variant_type = row.variant_type;
        let variant = row.variant;

        // Deduplicate within this batch
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

/// Import bibitems from parsed rows.
///
/// Before inserting, validates ALL referenced IDs exist. If any are missing,
/// returns all missing IDs in a `MissingReferences` result and inserts nothing.
pub async fn import_bibitems(
    bibitem_ds: &impl DataSource<crate::domain::BibItem, Id = i64, Error = hexforge::DataSourceError>,
    junction_store: &impl BibitemJunctionStore,
    ref_store: &impl ReferenceStore,
    syncer: &impl SequenceSyncer,
    rows: Vec<ParsedBibitemRow>,
    mut parse_errors: Vec<ImportRowError>,
) -> Result<BibitemImportResult, HexforgeError> {
    // Validate all parsed rows
    let valid_rows: Vec<ParsedBibitemRow> = rows
        .into_iter()
        .filter_map(|row| match validate_create_bibitem(&row.dto) {
            Ok(()) => Some(row),
            Err(e) => {
                parse_errors.push(ImportRowError {
                    row: row.row_num,
                    identifier: row.bibkey.clone(),
                    error: e.to_string(),
                });
                None
            }
        })
        .collect();
    let parse_errors = parse_errors; // make immutable

    // Collect all referenced keys from valid rows
    let mut all_author_keys: HashSet<String> = HashSet::new();
    let all_journal_keys: HashSet<String> = HashSet::new();
    let all_publisher_keys: HashSet<String> = HashSet::new();
    let all_institution_keys: HashSet<String> = HashSet::new();
    let all_school_keys: HashSet<String> = HashSet::new();
    let all_series_keys: HashSet<String> = HashSet::new();
    let mut all_keyword_keys: HashSet<String> = HashSet::new();
    let all_crossref_keys: HashSet<String> = HashSet::new();

    for row in &valid_rows {
        all_author_keys.extend(row.author_keys.iter().cloned());
        all_author_keys.extend(row.editor_keys.iter().cloned());
        all_author_keys.extend(row.guesteditor_keys.iter().cloned());
        all_keyword_keys.extend(row.keyword_keys.iter().cloned());
        // FK fields (journal_key, publisher_key, etc.) validated via FK constraints at insert time
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

    let parsed_rows = valid_rows;

    if parsed_rows.is_empty() {
        return Ok(BibitemImportResult::Success(ImportResponse {
            imported: 0,
            updated: 0,
            failed: 0,
            errors: vec![],
        }));
    }

    // Phase 2: Batch-check all referenced keys exist
    let missing = check_all_references(
        ref_store,
        &all_author_keys,
        &all_journal_keys,
        &all_publisher_keys,
        &all_institution_keys,
        &all_school_keys,
        &all_series_keys,
        &all_keyword_keys,
        &all_crossref_keys,
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
        // Determine if this is an insert or update; perform the bibitem upsert
        let is_update = if let Some(id) = row.source_id {
            match bibitem_ds.find_by_id(&id).await {
                Ok(Some(existing)) => {
                    if existing.bibkey != row.bibkey {
                        insert_errors.push(ImportRowError {
                            row: row.row_num,
                            identifier: row.bibkey.clone(),
                            error: format!(
                                "ID {id} exists but has bibkey '{}', input has '{}'",
                                existing.bibkey, row.bibkey
                            ),
                        });
                        continue;
                    }
                    // Update existing bibitem
                    let merged =
                        update_bib_item_transform(build_bibitem_update_dto(&row.dto), existing);
                    match bibitem_ds.update(&id, merged).await {
                        Ok(Some(_)) => true,
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
                        Ok(_) => false,
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
            // No input ID — insert normally
            let bibitem = create_bib_item_transform(row.dto.clone());
            match bibitem_ds.insert(bibitem).await {
                Ok(_) => false,
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
            &row.bibkey,
            &row.author_keys,
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
            &row.bibkey,
            &row.editor_keys,
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
            &row.bibkey,
            &row.guesteditor_keys,
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
        if let Err(e) =
            insert_bibitem_keywords(junction_store, &row.bibkey, &row.keyword_keys).await
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

    syncer.sync_sequence("bibitems").await?;
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

/// Check all referenced keys exist, returning any missing ones.
#[allow(clippy::too_many_arguments)]
async fn check_all_references(
    ref_store: &impl ReferenceStore,
    author_keys: &HashSet<String>,
    journal_keys: &HashSet<String>,
    publisher_keys: &HashSet<String>,
    institution_keys: &HashSet<String>,
    school_keys: &HashSet<String>,
    series_keys: &HashSet<String>,
    keyword_keys: &HashSet<String>,
    crossref_keys: &HashSet<String>,
) -> Result<MissingReferencesError, HexforgeError> {
    let (
        missing_authors,
        missing_journals,
        missing_publishers,
        missing_institutions,
        missing_schools,
        missing_series,
        missing_keywords,
        missing_crossrefs,
    ) = tokio::try_join!(
        ref_store.find_missing_author_keys(author_keys),
        ref_store.find_missing_journal_keys(journal_keys),
        ref_store.find_missing_publisher_keys(publisher_keys),
        ref_store.find_missing_institution_keys(institution_keys),
        ref_store.find_missing_school_keys(school_keys),
        ref_store.find_missing_series_keys(series_keys),
        ref_store.find_missing_keyword_keys(keyword_keys),
        ref_store.find_missing_bibitem_keys(crossref_keys),
    )?;

    Ok(MissingReferencesError {
        error: "missing_references",
        message: "Some referenced entities were not found",
        missing_author_keys: missing_authors,
        missing_journal_keys: missing_journals,
        missing_publisher_keys: missing_publishers,
        missing_institution_keys: missing_institutions,
        missing_school_keys: missing_schools,
        missing_series_keys: missing_series,
        missing_keyword_keys: missing_keywords,
        missing_crossref_keys: missing_crossrefs,
    })
}

// =============================================================================
// Junction table insertion helpers
// =============================================================================

/// Insert bibitem-author junction records via the junction store.
async fn insert_bibitem_authors(
    junction_store: &impl BibitemJunctionStore,
    bibkey: &str,
    author_keys: &[String],
    role: AuthorRole,
) -> Result<(), HexforgeError> {
    for (position, author_key) in author_keys.iter().enumerate() {
        let pos = i16::try_from(position).map_err(|_| {
            HexforgeError::Validation(ValidationError::custom(format!(
                "author position {position} exceeds i16 range"
            )))
        })?;
        junction_store
            .insert_author_junction(bibkey, author_key, &role, pos)
            .await?;
    }
    Ok(())
}

/// Insert bibitem–keyword junction records via the junction store.
/// Keywords are looked up to determine their level.
async fn insert_bibitem_keywords(
    junction_store: &impl BibitemJunctionStore,
    bibkey: &str,
    keyword_keys: &[String],
) -> Result<(), HexforgeError> {
    if keyword_keys.is_empty() {
        return Ok(());
    }

    // Fetch keyword levels via the junction store
    let kw_levels = junction_store.find_keyword_levels(keyword_keys).await?;

    for (kw_key, level) in &kw_levels {
        junction_store
            .insert_keyword_junction(bibkey, kw_key, *level)
            .await?;
    }

    Ok(())
}

// =============================================================================
// BibitemRefsStore + import_bibitem_refs
// =============================================================================

/// Contract for inserting bibitem reference rows.
pub trait BibitemRefsStore: Send + Sync {
    fn insert_bibitem_ref(
        &self,
        source_key: &str,
        target_key: &str,
        ref_type: &str,
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;
}

/// Import bibitem refs from parsed rows.
///
/// All referenced bibitem keys must exist; returns a validation error listing any that are missing.
pub async fn import_bibitem_refs(
    refs_store: &impl BibitemRefsStore,
    key_store: &impl ReferenceStore,
    rows: Vec<ParsedBibitemRefRow>,
    errors: Vec<ImportRowError>,
) -> Result<ImportResponse, HexforgeError> {
    if !errors.is_empty() {
        return Ok(ImportResponse {
            imported: 0,
            updated: 0,
            failed: errors.len(),
            errors,
        });
    }

    let all_keys: HashSet<String> = rows
        .iter()
        .flat_map(|r| [r.source_key.clone(), r.target_key.clone()])
        .collect();
    if !all_keys.is_empty() {
        let missing = key_store.find_missing_bibitem_keys(&all_keys).await?;
        if !missing.is_empty() {
            return Err(HexforgeError::Validation(ValidationError::custom(format!(
                "missing bibitem keys: {missing:?}"
            ))));
        }
    }

    let total = rows.len();
    for row in &rows {
        refs_store
            .insert_bibitem_ref(&row.source_key, &row.target_key, &row.ref_type)
            .await?;
    }

    Ok(ImportResponse {
        imported: total,
        updated: 0,
        failed: 0,
        errors: vec![],
    })
}

// =============================================================================
// BibitemNotesStore + import_bibitem_notes
// =============================================================================

/// Six optional note fields for upsert.
pub struct BibitemNotesData<'a> {
    pub note_perso: Option<&'a str>,
    pub note_stock: Option<&'a str>,
    pub note_missing: Option<&'a str>,
    pub change_request: Option<&'a str>,
    pub dltc_copyediting_note: Option<&'a str>,
    pub todo_general: Option<&'a str>,
}

/// Contract for upserting bibitem notes rows.
pub trait BibitemNotesStore: Send + Sync {
    fn upsert_bibitem_notes(
        &self,
        bibkey: &str,
        notes: &BibitemNotesData<'_>,
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;
}

/// Import bibitem notes from parsed rows.
///
/// Notes are upserted by `bibkey`.
pub async fn import_bibitem_notes(
    notes_store: &impl BibitemNotesStore,
    rows: Vec<ParsedBibitemNotesRow>,
    errors: Vec<ImportRowError>,
) -> Result<ImportResponse, HexforgeError> {
    if !errors.is_empty() {
        return Ok(ImportResponse {
            imported: 0,
            updated: 0,
            failed: errors.len(),
            errors,
        });
    }

    let total = rows.len();
    for row in &rows {
        notes_store
            .upsert_bibitem_notes(
                &row.bibkey,
                &BibitemNotesData {
                    note_perso: row.note_perso.as_deref(),
                    note_stock: row.note_stock.as_deref(),
                    note_missing: row.note_missing.as_deref(),
                    change_request: row.change_request.as_deref(),
                    dltc_copyediting_note: row.dltc_copyediting_note.as_deref(),
                    todo_general: row.todo_general.as_deref(),
                },
            )
            .await?;
    }

    Ok(ImportResponse {
        imported: total,
        updated: 0,
        failed: 0,
        errors: vec![],
    })
}
