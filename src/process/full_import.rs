//! Full import process — orchestrates validation, entity creation, and bibitem import.
//!
//! Defines traits for I/O operations and coordinates between data fetching
//! (via traits) and pure logic functions (from `crate::logic::full_import`).
//! No AppState, no PgPool, no sqlx, no SQL -- only abstract contracts.
//!
//! **Architecture:** This module defines WHAT operations are needed via traits.
//! Concrete I/O implementations live in `crate::adapters::full_import`.

use std::collections::{HashMap, HashSet};
use std::future::Future;

use hexforge::{DataSource, HexforgeError};

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow};
use crate::domain::{
    BibItem, BibitemNotes, CreateInstitution, CreateKeyword, CreateSchool, CreateSeries,
    create_bib_item_transform, create_institution_transform, create_keyword_transform,
    create_school_transform, create_series_transform,
};
use crate::logic::full_import::{
    AuthorJunctionRow, AuthorLookupResult, BibitemRefInsertRow, CollectedNames, EntityImportError,
    EntityImportReport, ExportContext, FieldError, FullImportReport, FullImportResult,
    KeywordJunctionRow, LookupMaps, NamedEntityKind, ParsedBibRow, ResolutionCtx, RowError,
    ValidationReport, assemble_validation_report, build_bibitem_dto, build_export_record,
    collect_author_junction_rows, collect_keyword_junction_rows, collect_ref_rows,
    filter_importable_rows, generate_key,
};
use crate::process::import::{BibitemNotesData, BibitemNotesStore};
use crate::validation::{
    validate_create_bibitem, validate_create_institution, validate_create_keyword,
    validate_create_school, validate_create_series,
};

// =============================================================================
// Traits -- contracts for I/O operations that adapters implement
// =============================================================================

/// Contract for batch-looking up all authors and building the author name key maps.
pub trait AuthorLookup: Send + Sync {
    fn batch_lookup_authors(
        &self,
    ) -> impl Future<Output = Result<AuthorLookupResult, HexforgeError>> + Send;
}

/// Which named-entity table to query (journals, publishers, institutions, schools, series).
#[derive(Debug, Clone, Copy)]
pub enum NamedEntity {
    Journals,
    Publishers,
    Institutions,
    Schools,
    Series,
}

/// Contract for batch-looking up entities by name_latex.
pub trait EntityLookup: Send + Sync {
    fn batch_lookup_named_entity(
        &self,
        entity: NamedEntity,
    ) -> impl Future<Output = Result<HashMap<String, i64>, HexforgeError>> + Send;
}

/// Contract for batch-looking up keywords by (name, level).
pub trait KeywordLookup: Send + Sync {
    fn batch_lookup_keywords(
        &self,
    ) -> impl Future<Output = Result<HashMap<(String, i16), i64>, HexforgeError>> + Send;
}

/// Contract for fetching all existing bibkeys.
pub trait BibkeyLookup: Send + Sync {
    fn fetch_all_bibkeys(
        &self,
    ) -> impl Future<Output = Result<HashSet<String>, HexforgeError>> + Send;
}

/// Contract for deleting bibitems by bibkeys (stale removal + pre-update cleanup).
pub trait BibitemDeleter: Send + Sync {
    fn delete_bibitems_by_bibkeys(
        &self,
        bibkeys: &[String],
    ) -> impl Future<Output = Result<usize, HexforgeError>> + Send;
}

/// Contract for bulk-inserting bibitems.
/// Returns (id, bibkey) pairs for all inserted rows.
pub trait BulkBibitemInsert: Send + Sync {
    fn bulk_insert_bibitems(
        &self,
        entities: &[BibItem],
    ) -> impl Future<Output = Result<Vec<(i64, String)>, HexforgeError>> + Send;
}

/// Contract for bulk-inserting all junction and ref rows.
pub trait BulkJunctionInsert: Send + Sync {
    fn bulk_insert_author_junctions(
        &self,
        rows: &[AuthorJunctionRow],
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;

    fn bulk_insert_keyword_junctions(
        &self,
        rows: &[KeywordJunctionRow],
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;

    fn bulk_insert_bibitem_refs(
        &self,
        rows: &[BibitemRefInsertRow],
    ) -> impl Future<Output = Result<(), HexforgeError>> + Send;
}

/// Contract for computing and storing the transitive closure of ref edges.
///
/// Returns `(further_refs_inserted, depends_on_inserted)`.
pub trait TransitiveDepsComputer: Send + Sync {
    fn compute_transitive_deps(
        &self,
    ) -> impl Future<Output = Result<(usize, usize), HexforgeError>> + Send;
}

/// Contract for fetching all bibitems (for export).
pub trait BibitemFetcher: Send + Sync {
    fn fetch_all_bibitems(
        &self,
    ) -> impl Future<Output = Result<Vec<BibItem>, HexforgeError>> + Send;
}

/// Contract for fetching author display names (id -> name string).
pub trait AuthorNameFetcher: Send + Sync {
    fn fetch_author_names(
        &self,
    ) -> impl Future<Output = Result<HashMap<i64, String>, HexforgeError>> + Send;
}

/// Contract for fetching reverse name maps (id -> name_latex) for entity tables.
pub trait ReverseNameMapFetcher: Send + Sync {
    fn fetch_entity_name_map(
        &self,
        entity: NamedEntity,
    ) -> impl Future<Output = Result<HashMap<i64, String>, HexforgeError>> + Send;
}

/// Contract for fetching keyword names (id -> (name, level)).
pub trait KeywordNameFetcher: Send + Sync {
    fn fetch_keyword_names(
        &self,
    ) -> impl Future<Output = Result<HashMap<i64, (String, i16)>, HexforgeError>> + Send;
}

/// Contract for batch-fetching junction data for export.
pub trait JunctionFetcher: Send + Sync {
    fn fetch_bibitem_authors_batch(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemAuthorsRow>, HexforgeError>> + Send;

    fn fetch_bibitem_keywords_batch(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemKeywordsRow>, HexforgeError>> + Send;
}

pub trait BibitemNotesFetcher: Send + Sync {
    fn fetch_all_bibitem_notes(
        &self,
    ) -> impl Future<Output = Result<HashMap<i64, BibitemNotes>, HexforgeError>> + Send;
}

// =============================================================================
// Build lookup maps (orchestration helper)
// =============================================================================

/// Build all lookup maps by calling the provided trait implementations.
async fn build_lookup_maps(
    author_lookup: &impl AuthorLookup,
    entity_lookup: &impl EntityLookup,
    keyword_lookup: &impl KeywordLookup,
    bibkey_lookup: &impl BibkeyLookup,
) -> Result<LookupMaps, HexforgeError> {
    Ok(LookupMaps {
        authors: author_lookup.batch_lookup_authors().await?,
        journals: entity_lookup
            .batch_lookup_named_entity(NamedEntity::Journals)
            .await?,
        publishers: entity_lookup
            .batch_lookup_named_entity(NamedEntity::Publishers)
            .await?,
        institutions: entity_lookup
            .batch_lookup_named_entity(NamedEntity::Institutions)
            .await?,
        schools: entity_lookup
            .batch_lookup_named_entity(NamedEntity::Schools)
            .await?,
        series: entity_lookup
            .batch_lookup_named_entity(NamedEntity::Series)
            .await?,
        keywords: keyword_lookup.batch_lookup_keywords().await?,
        bibkeys: bibkey_lookup.fetch_all_bibkeys().await?,
    })
}

// =============================================================================
// Validate endpoint orchestration
// =============================================================================

/// Validate parsed rows against the database. Returns a validation report. Does NOT modify anything.
pub async fn validate_import(
    author_lookup: &impl AuthorLookup,
    entity_lookup: &impl EntityLookup,
    keyword_lookup: &impl KeywordLookup,
    bibkey_lookup: &impl BibkeyLookup,
    rows: &[ParsedBibRow],
    row_errors: Vec<RowError>,
) -> Result<ValidationReport, HexforgeError> {
    let maps =
        build_lookup_maps(author_lookup, entity_lookup, keyword_lookup, bibkey_lookup).await?;
    Ok(assemble_validation_report(rows, row_errors, &maps))
}

// =============================================================================
// Import entities orchestration
// =============================================================================

/// Find entities referenced in rows but not in DB, and create them.
/// Only creates institutions, schools, series, and keywords.
/// Authors, journals, and publishers must be imported separately.
#[allow(clippy::too_many_arguments)]
pub async fn import_entities(
    author_lookup: &impl AuthorLookup,
    entity_lookup: &impl EntityLookup,
    keyword_lookup: &impl KeywordLookup,
    bibkey_lookup: &impl BibkeyLookup,
    institution_ds: &impl DataSource<
        crate::domain::Institution,
        Id = i64,
        Error = hexforge::DataSourceError,
    >,
    school_ds: &impl DataSource<crate::domain::School, Id = i64, Error = hexforge::DataSourceError>,
    series_ds: &impl DataSource<crate::domain::Series, Id = i64, Error = hexforge::DataSourceError>,
    keyword_ds: &impl DataSource<crate::domain::Keyword, Id = i64, Error = hexforge::DataSourceError>,
    rows: &[ParsedBibRow],
) -> Result<EntityImportReport, HexforgeError> {
    let mut collected = CollectedNames::default();
    for row in rows {
        collected.collect_from_row(row);
    }

    let maps =
        build_lookup_maps(author_lookup, entity_lookup, keyword_lookup, bibkey_lookup).await?;

    let mut report = EntityImportReport {
        created_institutions: 0,
        created_schools: 0,
        created_series: 0,
        created_keywords: 0,
        errors: Vec::new(),
    };

    // Create missing institutions
    for name in &collected.institution_names {
        if maps.institutions.contains_key(name) {
            continue;
        }
        let key = generate_key(name);
        match create_named_entity_institution(&key, name, institution_ds).await {
            Ok(()) => report.created_institutions += 1,
            Err(e) => report.errors.push(EntityImportError {
                entity_type: NamedEntityKind::Institution,
                name: name.clone(),
                error: e,
            }),
        }
    }

    // Create missing schools
    for name in &collected.school_names {
        if maps.schools.contains_key(name) {
            continue;
        }
        let key = generate_key(name);
        match create_named_entity_school(&key, name, school_ds).await {
            Ok(()) => report.created_schools += 1,
            Err(e) => report.errors.push(EntityImportError {
                entity_type: NamedEntityKind::School,
                name: name.clone(),
                error: e,
            }),
        }
    }

    // Create missing series
    for name in &collected.series_names {
        if maps.series.contains_key(name) {
            continue;
        }
        let key = generate_key(name);
        match create_named_entity_series(&key, name, series_ds).await {
            Ok(()) => report.created_series += 1,
            Err(e) => report.errors.push(EntityImportError {
                entity_type: NamedEntityKind::Series,
                name: name.clone(),
                error: e,
            }),
        }
    }

    // Create missing keywords
    for kw in &collected.keywords_l1 {
        if !maps.keywords.contains_key(&(kw.clone(), 1)) {
            match create_keyword(kw, 1, keyword_ds).await {
                Ok(()) => report.created_keywords += 1,
                Err(e) => report.errors.push(EntityImportError {
                    entity_type: NamedEntityKind::Keyword,
                    name: format!("{kw} (level 1)"),
                    error: e,
                }),
            }
        }
    }
    for kw in &collected.keywords_l2 {
        if !maps.keywords.contains_key(&(kw.clone(), 2)) {
            match create_keyword(kw, 2, keyword_ds).await {
                Ok(()) => report.created_keywords += 1,
                Err(e) => report.errors.push(EntityImportError {
                    entity_type: NamedEntityKind::Keyword,
                    name: format!("{kw} (level 2)"),
                    error: e,
                }),
            }
        }
    }
    for kw in &collected.keywords_l3 {
        if !maps.keywords.contains_key(&(kw.clone(), 3)) {
            match create_keyword(kw, 3, keyword_ds).await {
                Ok(()) => report.created_keywords += 1,
                Err(e) => report.errors.push(EntityImportError {
                    entity_type: NamedEntityKind::Keyword,
                    name: format!("{kw} (level 3)"),
                    error: e,
                }),
            }
        }
    }

    Ok(report)
}

// =============================================================================
// Entity creation helpers (use DataSource trait)
// =============================================================================

async fn create_named_entity_institution(
    key: &str,
    name: &str,
    ds: &impl DataSource<crate::domain::Institution, Id = i64, Error = hexforge::DataSourceError>,
) -> Result<(), String> {
    let dto = CreateInstitution {
        institution_key: key.to_string(),
        name_latex: name.to_string(),
        name_unicode: name.to_string(),
        default_address: None,
    };
    validate_create_institution(&dto).map_err(|e| e.to_string())?;
    let entity = create_institution_transform(dto);
    ds.insert(entity).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_named_entity_school(
    key: &str,
    name: &str,
    ds: &impl DataSource<crate::domain::School, Id = i64, Error = hexforge::DataSourceError>,
) -> Result<(), String> {
    let dto = CreateSchool {
        school_key: key.to_string(),
        name_latex: name.to_string(),
        name_unicode: name.to_string(),
    };
    validate_create_school(&dto).map_err(|e| e.to_string())?;
    let entity = create_school_transform(dto);
    ds.insert(entity).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_named_entity_series(
    key: &str,
    name: &str,
    ds: &impl DataSource<crate::domain::Series, Id = i64, Error = hexforge::DataSourceError>,
) -> Result<(), String> {
    let dto = CreateSeries {
        series_key: key.to_string(),
        name_latex: name.to_string(),
        name_unicode: name.to_string(),
    };
    validate_create_series(&dto).map_err(|e| e.to_string())?;
    let entity = create_series_transform(dto);
    ds.insert(entity).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_keyword(
    name: &str,
    level: i16,
    ds: &impl DataSource<crate::domain::Keyword, Id = i64, Error = hexforge::DataSourceError>,
) -> Result<(), String> {
    let dto = CreateKeyword {
        name: name.to_string(),
        level,
    };
    validate_create_keyword(&dto).map_err(|e| e.to_string())?;
    let entity = create_keyword_transform(dto);
    ds.insert(entity).await.map_err(|e| e.to_string())?;
    Ok(())
}

// =============================================================================
// Recompute transitive deps orchestration
// =============================================================================

/// Truncate and recompute all transitive dependency tables from `bibitem_refs`.
///
/// Returns `(further_refs_inserted, depends_on_inserted)`.
pub async fn recompute_transitive_deps(
    deps_computer: &impl TransitiveDepsComputer,
) -> Result<(usize, usize), HexforgeError> {
    deps_computer.compute_transitive_deps().await
}

// =============================================================================
// Import bibitems (full source-of-truth import) orchestration
// =============================================================================

#[allow(clippy::too_many_arguments)]
/// Resolve all names to IDs, bulk-upsert bibitems + junctions.
/// Rows are the source of truth: bibitems in DB but not in rows get deleted if `delete_stale=true`.
///
/// Hard errors (duplicate bibkeys, unresolved entity references, ambiguous authors) return a
/// 422 ValidationFailed. Soft issues (missing cross-bibitem refs, missing keywords) are
/// skipped per-row — check the validate endpoint for the full picture.
pub async fn import_bibitems(
    author_lookup: &impl AuthorLookup,
    entity_lookup: &impl EntityLookup,
    keyword_lookup: &impl KeywordLookup,
    bibkey_lookup: &impl BibkeyLookup,
    bibitem_deleter: &impl BibitemDeleter,
    bulk_inserter: &impl BulkBibitemInsert,
    bulk_junction: &impl BulkJunctionInsert,
    transitive_deps: &impl TransitiveDepsComputer,
    notes_store: &impl BibitemNotesStore,
    rows: Vec<ParsedBibRow>,
    row_errors: Vec<RowError>,
    delete_stale: bool,
) -> Result<FullImportResult, HexforgeError> {
    // 1. Build lookup maps once (single DB round-trip)
    let maps =
        build_lookup_maps(author_lookup, entity_lookup, keyword_lookup, bibkey_lookup).await?;

    // 2. Assemble validation report; block only on hard errors (duplicate bibkeys)
    let report = assemble_validation_report(&rows, row_errors, &maps);
    if report.has_hard_errors() {
        return Ok(FullImportResult::ValidationFailed(Box::new(report)));
    }

    // 3. Capture full-file bibkeys before consuming `rows` (stale detection needs the full set)
    let file_bibkeys: HashSet<String> = rows.iter().map(|r| r.bibkey.clone()).collect();

    // 4. Build resolution context; filter to rows where all entity refs resolve
    let ctx = ResolutionCtx::from_lookup_maps(maps);
    let (parsed_rows, filtered_errors) = filter_importable_rows(rows, &ctx);

    // 5. Delete stale bibitems (in DB but absent from the source file entirely)
    let deleted = if delete_stale {
        let stale: Vec<String> = ctx
            .existing_bibkeys
            .difference(&file_bibkeys)
            .cloned()
            .collect();
        if stale.is_empty() {
            0
        } else {
            bibitem_deleter.delete_bibitems_by_bibkeys(&stale).await?
        }
    } else {
        0
    };

    // 6. Build all entities in memory; track update vs insert counts
    let (entities_with_flags, build_errors) = build_all_bibitem_entities(&parsed_rows, &ctx);
    let updated = entities_with_flags
        .iter()
        .filter(|(was_update, _)| *was_update)
        .count();
    let imported = entities_with_flags
        .iter()
        .filter(|(was_update, _)| !was_update)
        .count();

    // 7. Batch-delete all bibkeys that will be re-inserted (cascade clears junctions)
    let update_bibkeys: Vec<String> = entities_with_flags
        .iter()
        .filter(|(was_update, _)| *was_update)
        .map(|(_, e)| e.bibkey.clone())
        .collect();
    if !update_bibkeys.is_empty() {
        bibitem_deleter
            .delete_bibitems_by_bibkeys(&update_bibkeys)
            .await?;
    }

    // 8. Bulk insert all bibitems, get back (id, bibkey) pairs
    let entities: Vec<BibItem> = entities_with_flags.into_iter().map(|(_, e)| e).collect();
    let inserted_ids = bulk_inserter.bulk_insert_bibitems(&entities).await?;
    let bibkey_to_id: HashMap<String, i64> =
        inserted_ids.into_iter().map(|(id, bk)| (bk, id)).collect();

    // 9. Collect junction rows and bulk insert
    let author_rows = collect_author_junction_rows(&parsed_rows, &bibkey_to_id, &ctx);
    let keyword_rows = collect_keyword_junction_rows(&parsed_rows, &bibkey_to_id, &ctx);
    let ref_rows = collect_ref_rows(&parsed_rows, &bibkey_to_id);

    bulk_junction
        .bulk_insert_author_junctions(&author_rows)
        .await?;
    bulk_junction
        .bulk_insert_keyword_junctions(&keyword_rows)
        .await?;
    bulk_junction.bulk_insert_bibitem_refs(&ref_rows).await?;
    transitive_deps.compute_transitive_deps().await?;

    // Upsert notes for rows that have at least one note field set
    for row in &parsed_rows {
        let has_notes = row.note_perso.is_some()
            || row.note_stock.is_some()
            || row.note_missing.is_some()
            || row.change_request.is_some()
            || row.dltc_copyediting_note.is_some()
            || row.todo_general.is_some();
        if has_notes && let Some(&bibitem_id) = bibkey_to_id.get(&row.bibkey) {
            notes_store
                .upsert_bibitem_notes(
                    bibitem_id,
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
    }

    let skipped = filtered_errors.len();
    let failed = build_errors.len();
    let mut all_errors = filtered_errors;
    all_errors.extend(build_errors);

    Ok(FullImportResult::Success(FullImportReport {
        imported,
        updated,
        deleted,
        failed,
        skipped,
        errors: all_errors,
    }))
}

// =============================================================================
// Bulk entity builder (pure)
// =============================================================================

/// Build all bibitem entities from parsed rows.
/// Returns (was_update, entity) pairs and a list of rows that failed to build.
fn build_all_bibitem_entities(
    parsed_rows: &[ParsedBibRow],
    ctx: &ResolutionCtx,
) -> (Vec<(bool, BibItem)>, Vec<RowError>) {
    let mut entities = Vec::new();
    let mut errors = Vec::new();

    for (idx, row) in parsed_rows.iter().enumerate() {
        let row_num = idx + 2;
        match build_bibitem_dto(row, ctx) {
            Err(e) => errors.push(RowError {
                row: row_num,
                bibkey: Some(row.bibkey.clone()),
                errors: vec![FieldError {
                    field: "_build".to_string(),
                    error: e,
                }],
            }),
            Ok(dto) => match validate_create_bibitem(&dto) {
                Err(e) => errors.push(RowError {
                    row: row_num,
                    bibkey: Some(row.bibkey.clone()),
                    errors: vec![FieldError {
                        field: "_validation".to_string(),
                        error: e.to_string(),
                    }],
                }),
                Ok(()) => {
                    let was_update = ctx.existing_bibkeys.contains(&row.bibkey);
                    entities.push((was_update, create_bib_item_transform(dto)));
                }
            },
        }
    }

    (entities, errors)
}

// =============================================================================
// Export orchestration
// =============================================================================

/// Export all bibitems as raw rows in the full import format.
///
/// Returns one `Vec<String>` per bibitem (fields in column order). The caller
/// is responsible for serialising to whatever output format is needed (tabular,
/// JSON, etc.).
pub async fn fetch_export_rows(
    bibitem_fetcher: &impl BibitemFetcher,
    author_name_fetcher: &impl AuthorNameFetcher,
    reverse_name_fetcher: &impl ReverseNameMapFetcher,
    keyword_name_fetcher: &impl KeywordNameFetcher,
    junction_fetcher: &impl JunctionFetcher,
    notes_fetcher: &impl BibitemNotesFetcher,
) -> Result<Vec<Vec<String>>, HexforgeError> {
    // Fetch all bibitems
    let bibitems = bibitem_fetcher.fetch_all_bibitems().await?;

    // Build reverse lookup maps (ID -> name)
    let author_names = author_name_fetcher.fetch_author_names().await?;
    let journal_names = reverse_name_fetcher
        .fetch_entity_name_map(NamedEntity::Journals)
        .await?;
    let publisher_names = reverse_name_fetcher
        .fetch_entity_name_map(NamedEntity::Publishers)
        .await?;
    let institution_names = reverse_name_fetcher
        .fetch_entity_name_map(NamedEntity::Institutions)
        .await?;
    let school_names = reverse_name_fetcher
        .fetch_entity_name_map(NamedEntity::Schools)
        .await?;
    let series_names = reverse_name_fetcher
        .fetch_entity_name_map(NamedEntity::Series)
        .await?;

    // Keywords by ID
    let keyword_names = keyword_name_fetcher.fetch_keyword_names().await?;

    // Bibkey by ID (for crossref/refs resolution)
    let bibkey_by_id: HashMap<i64, String> =
        bibitems.iter().map(|b| (b.id, b.bibkey.clone())).collect();

    // Junction data
    let bib_ids: Vec<i64> = bibitems.iter().map(|b| b.id).collect();
    let bib_authors = junction_fetcher
        .fetch_bibitem_authors_batch(&bib_ids)
        .await?;
    let bib_keywords = junction_fetcher
        .fetch_bibitem_keywords_batch(&bib_ids)
        .await?;

    // Index junction data by bibitem_id
    let mut authors_by_bib: HashMap<i64, Vec<&BibitemAuthorsRow>> = HashMap::new();
    for row in &bib_authors {
        authors_by_bib.entry(row.bibitem_id).or_default().push(row);
    }
    let mut keywords_by_bib: HashMap<i64, Vec<&BibitemKeywordsRow>> = HashMap::new();
    for row in &bib_keywords {
        keywords_by_bib.entry(row.bibitem_id).or_default().push(row);
    }

    let notes_by_bib = notes_fetcher.fetch_all_bibitem_notes().await?;

    let export_ctx = ExportContext {
        author_names: &author_names,
        journal_names: &journal_names,
        publisher_names: &publisher_names,
        institution_names: &institution_names,
        school_names: &school_names,
        series_names: &series_names,
        keyword_names: &keyword_names,
        bibkey_by_id: &bibkey_by_id,
        authors_by_bib: &authors_by_bib,
        keywords_by_bib: &keywords_by_bib,
        notes_by_bib: &notes_by_bib,
    };

    let rows = bibitems
        .iter()
        .map(|bib| build_export_record(bib, &export_ctx))
        .collect();
    Ok(rows)
}
