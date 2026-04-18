//! Full CSV import process -- orchestrates validation, entity creation, and bibitem import.
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

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow, BibitemRefsRow};
use crate::domain::{
    AuthorRole, BibItem, CreateInstitution, CreateKeyword, CreateSchool, CreateSeries, RefType,
    create_bib_item_transform, create_institution_transform, create_keyword_transform,
    create_school_transform, create_series_transform,
};
use crate::logic::csv_parsing::types::{FieldError, ParsedAuthor, ParsedBibRow};
use crate::logic::full_import::{
    AuthorLookupResult, AuthorNameKey, CollectedNames, EntityImportError, EntityImportReport,
    ExportContext, FULL_CSV_HEADERS, FullImportReport, FullImportResult, LookupMaps,
    NamedEntityKind, ResolutionCtx, RowError, ValidationReport, VariantInfo,
    assemble_validation_report, build_bibitem_dto, build_export_record, generate_key,
    parse_all_rows,
};
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

/// Contract for batch-looking up entities by name_latex.
pub trait EntityLookup: Send + Sync {
    fn batch_lookup_by_name_latex(
        &self,
        table: &str,
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

/// Contract for inserting author junctions with name variant support.
pub trait FullImportAuthorJunctionStore: Send + Sync {
    fn insert_author_junction(
        &self,
        bibitem_id: i64,
        author_id: i64,
        role: &AuthorRole,
        position: i16,
        variant_latex: Option<&str>,
        variant_unicode: Option<&str>,
    ) -> impl Future<Output = Result<(), String>> + Send;
}

/// Contract for inserting keyword junctions.
pub trait FullImportKeywordJunctionStore: Send + Sync {
    fn insert_keyword_junction(
        &self,
        bibitem_id: i64,
        keyword_id: i64,
        keyword_level: i16,
    ) -> impl Future<Output = Result<(), String>> + Send;
}

/// Contract for inserting bibitem refs.
pub trait FullImportRefStore: Send + Sync {
    fn insert_bibitem_ref(
        &self,
        source_id: i64,
        target_bibkey: &str,
        ref_type: &RefType,
    ) -> impl Future<Output = Result<(), String>> + Send;
}

/// Contract for deleting bibitems by bibkeys.
pub trait BibitemDeleter: Send + Sync {
    fn delete_bibitems_by_bibkeys(
        &self,
        bibkeys: &[String],
    ) -> impl Future<Output = Result<usize, HexforgeError>> + Send;
}

/// Contract for deleting a single bibitem by bibkey (for upsert: delete-then-reinsert).
pub trait BibitemByBibkeyDeleter: Send + Sync {
    fn delete_by_bibkey(&self, bibkey: &str) -> impl Future<Output = Result<(), String>> + Send;
}

/// Contract for fetching all bibitems (for export).
pub trait FullCsvBibitemFetcher: Send + Sync {
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
    fn fetch_reverse_name_map(
        &self,
        table: &str,
    ) -> impl Future<Output = Result<HashMap<i64, String>, HexforgeError>> + Send;
}

/// Contract for fetching keyword names (id -> (name, level)).
pub trait KeywordNameFetcher: Send + Sync {
    fn fetch_keyword_names(
        &self,
    ) -> impl Future<Output = Result<HashMap<i64, (String, i16)>, HexforgeError>> + Send;
}

/// Contract for batch-fetching junction data for export.
pub trait FullCsvJunctionFetcher: Send + Sync {
    fn fetch_bibitem_authors_batch(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemAuthorsRow>, HexforgeError>> + Send;

    fn fetch_bibitem_keywords_batch(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemKeywordsRow>, HexforgeError>> + Send;

    fn fetch_bibitem_refs_batch(
        &self,
        ids: &[i64],
    ) -> impl Future<Output = Result<Vec<BibitemRefsRow>, HexforgeError>> + Send;
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
        journals: entity_lookup.batch_lookup_by_name_latex("journals").await?,
        publishers: entity_lookup
            .batch_lookup_by_name_latex("publishers")
            .await?,
        institutions: entity_lookup
            .batch_lookup_by_name_latex("institutions")
            .await?,
        schools: entity_lookup.batch_lookup_by_name_latex("schools").await?,
        series: entity_lookup.batch_lookup_by_name_latex("series").await?,
        keywords: keyword_lookup.batch_lookup_keywords().await?,
        bibkeys: bibkey_lookup.fetch_all_bibkeys().await?,
    })
}

// =============================================================================
// Validate endpoint orchestration
// =============================================================================

/// Parse and validate a full CSV, checking all references against the database.
/// Returns a validation report. Does NOT modify anything.
pub async fn validate_full_csv(
    author_lookup: &impl AuthorLookup,
    entity_lookup: &impl EntityLookup,
    keyword_lookup: &impl KeywordLookup,
    bibkey_lookup: &impl BibkeyLookup,
    data: &[u8],
) -> Result<ValidationReport, HexforgeError> {
    // 1. Parse all rows (pure)
    let (parsed_rows, row_errors) = parse_all_rows(data)?;

    // 2. Batch DB lookups
    let maps =
        build_lookup_maps(author_lookup, entity_lookup, keyword_lookup, bibkey_lookup).await?;

    // 3. Assemble report (pure)
    Ok(assemble_validation_report(&parsed_rows, row_errors, &maps))
}

// =============================================================================
// Import entities orchestration
// =============================================================================

/// Parse CSV, find entities referenced but not in DB, and create them.
/// Only creates institutions, schools, series, and keywords.
/// Authors, journals, and publishers must be imported separately.
#[allow(clippy::too_many_arguments)]
pub async fn import_entities_from_full_csv(
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
    data: &[u8],
) -> Result<EntityImportReport, HexforgeError> {
    let (parsed_rows, _) = parse_all_rows(data)?;

    let mut collected = CollectedNames::default();
    for row in &parsed_rows {
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
                entity_type: NamedEntityKind::Institution.label().to_string(),
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
                entity_type: NamedEntityKind::School.label().to_string(),
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
                entity_type: NamedEntityKind::Series.label().to_string(),
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
                    entity_type: "keyword".to_string(),
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
                    entity_type: "keyword".to_string(),
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
                    entity_type: "keyword".to_string(),
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
// Import bibitems (full source-of-truth import) orchestration
// =============================================================================

/// Parse CSV, resolve all names to IDs, upsert bibitems + junctions.
/// CSV is source of truth: bibitems in DB but not in CSV get deleted.
///
/// Runs full validation first -- if any issues exist, returns the complete
/// `ValidationReport` so the caller sees everything in one response.
#[allow(clippy::too_many_arguments)]
pub async fn import_full_csv(
    author_lookup: &impl AuthorLookup,
    entity_lookup: &impl EntityLookup,
    keyword_lookup: &impl KeywordLookup,
    bibkey_lookup: &impl BibkeyLookup,
    bibitem_ds: &impl DataSource<crate::domain::BibItem, Id = i64, Error = hexforge::DataSourceError>,
    bibitem_deleter: &impl BibitemDeleter,
    bibkey_deleter: &impl BibitemByBibkeyDeleter,
    author_junction_store: &impl FullImportAuthorJunctionStore,
    keyword_junction_store: &impl FullImportKeywordJunctionStore,
    ref_store: &impl FullImportRefStore,
    data: &[u8],
    delete_stale: bool,
) -> Result<FullImportResult, HexforgeError> {
    // 1. Validate -- return full report if anything is wrong
    let report = validate_full_csv(
        author_lookup,
        entity_lookup,
        keyword_lookup,
        bibkey_lookup,
        data,
    )
    .await?;
    if report.has_issues() {
        return Ok(FullImportResult::ValidationFailed(Box::new(report)));
    }

    // 2. Parse (validated, so no errors expected)
    let (parsed_rows, _) = parse_all_rows(data)?;

    // 3. Build resolution context
    let mut collected = CollectedNames::default();
    let mut csv_bibkeys = HashSet::new();
    for row in &parsed_rows {
        collected.collect_from_row(row);
        csv_bibkeys.insert(row.bibkey.clone());
    }
    let maps =
        build_lookup_maps(author_lookup, entity_lookup, keyword_lookup, bibkey_lookup).await?;
    let ctx = ResolutionCtx::from_lookup_maps(maps);

    // 4. Delete stale bibitems (only if explicitly requested)
    let deleted = if delete_stale {
        let stale: Vec<String> = ctx
            .existing_bibkeys
            .difference(&csv_bibkeys)
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

    // 5. Upsert each bibitem
    let mut imported = 0usize;
    let mut updated = 0usize;
    let mut errors = Vec::new();

    for (idx, row) in parsed_rows.iter().enumerate() {
        let row_num = idx + 2;
        match upsert_bibitem_row(
            row,
            &ctx,
            bibitem_ds,
            bibkey_deleter,
            author_junction_store,
            keyword_junction_store,
            ref_store,
        )
        .await
        {
            Ok(was_update) => {
                if was_update {
                    updated += 1;
                } else {
                    imported += 1;
                }
            }
            Err(e) => {
                errors.push(RowError {
                    row: row_num,
                    bibkey: Some(row.bibkey.clone()),
                    errors: vec![FieldError {
                        field: "_insert".to_string(),
                        error: e,
                    }],
                });
            }
        }
    }

    Ok(FullImportResult::Success(FullImportReport {
        imported,
        updated,
        deleted,
        failed: errors.len(),
        errors,
    }))
}

// =============================================================================
// Bibitem upsert (orchestration)
// =============================================================================

/// Resolve a parsed row to IDs and upsert the bibitem + junctions.
/// Returns Ok(true) if updated, Ok(false) if inserted.
async fn upsert_bibitem_row(
    row: &ParsedBibRow,
    ctx: &ResolutionCtx,
    bibitem_ds: &impl DataSource<crate::domain::BibItem, Id = i64, Error = hexforge::DataSourceError>,
    bibkey_deleter: &impl BibitemByBibkeyDeleter,
    author_junction_store: &impl FullImportAuthorJunctionStore,
    keyword_junction_store: &impl FullImportKeywordJunctionStore,
    ref_store: &impl FullImportRefStore,
) -> Result<bool, String> {
    let was_update = ctx.existing_bibkeys.contains(&row.bibkey);

    // Build CreateBibItem DTO (pure)
    let dto = build_bibitem_dto(row, ctx)?;
    validate_create_bibitem(&dto).map_err(|e| e.to_string())?;

    if was_update {
        // Delete existing bibitem (cascade deletes junctions), then re-insert
        bibkey_deleter.delete_by_bibkey(&row.bibkey).await?;
    }

    let entity = create_bib_item_transform(dto);
    let inserted = bibitem_ds
        .insert(entity)
        .await
        .map_err(|e| format!("failed to insert bibitem: {e}"))?;

    let bibitem_id: i64 = inserted.id;

    // Insert author junctions
    insert_author_junctions(
        author_junction_store,
        bibitem_id,
        &row.authors,
        AuthorRole::Author,
        &ctx.author_resolve,
        &ctx.author_variants,
    )
    .await?;
    insert_author_junctions(
        author_junction_store,
        bibitem_id,
        &row.editors,
        AuthorRole::Editor,
        &ctx.author_resolve,
        &ctx.author_variants,
    )
    .await?;
    insert_author_junctions(
        author_junction_store,
        bibitem_id,
        &row.guesteditors,
        AuthorRole::Guesteditor,
        &ctx.author_resolve,
        &ctx.author_variants,
    )
    .await?;

    // Insert keyword junctions
    insert_keyword_junctions(keyword_junction_store, bibitem_id, row, &ctx.keyword_map).await?;

    // Insert bibitem refs (further_refs, depends_on)
    insert_bibitem_refs(
        ref_store,
        bibitem_id,
        &row.further_ref_bibkeys,
        RefType::FurtherRef,
    )
    .await?;
    insert_bibitem_refs(
        ref_store,
        bibitem_id,
        &row.depends_on_bibkeys,
        RefType::DependsOn,
    )
    .await?;

    Ok(was_update)
}

// =============================================================================
// Junction insertion helpers (orchestration)
// =============================================================================

async fn insert_author_junctions(
    store: &impl FullImportAuthorJunctionStore,
    bibitem_id: i64,
    authors: &[ParsedAuthor],
    role: AuthorRole,
    resolve: &HashMap<AuthorNameKey, i64>,
    variants: &HashMap<AuthorNameKey, VariantInfo>,
) -> Result<(), String> {
    for (position, author) in authors.iter().enumerate() {
        let key = AuthorNameKey::from_parsed(author);
        let author_id = resolve
            .get(&key)
            .ok_or_else(|| format!("could not resolve author: {}", author.display_name()))?;
        let pos = i16::try_from(position).map_err(|_| "too many authors")?;
        let variant = variants.get(&key);
        let variant_latex = variant.and_then(|v| v.variant_latex.as_deref());
        let variant_unicode = variant.and_then(|v| v.variant_unicode.as_deref());
        store
            .insert_author_junction(
                bibitem_id,
                *author_id,
                &role,
                pos,
                variant_latex,
                variant_unicode,
            )
            .await?;
    }
    Ok(())
}

async fn insert_keyword_junctions(
    store: &impl FullImportKeywordJunctionStore,
    bibitem_id: i64,
    row: &ParsedBibRow,
    keyword_map: &HashMap<(String, i16), i64>,
) -> Result<(), String> {
    let all_keywords: Vec<(&String, i16)> = row
        .keywords
        .level_1
        .iter()
        .map(|k| (k, 1i16))
        .chain(row.keywords.level_2.iter().map(|k| (k, 2i16)))
        .chain(row.keywords.level_3.iter().map(|k| (k, 3i16)))
        .collect();

    for (name, level) in all_keywords {
        let keyword_id = keyword_map
            .get(&(name.clone(), level))
            .ok_or_else(|| format!("could not resolve keyword: {name} (level {level})"))?;
        store
            .insert_keyword_junction(bibitem_id, *keyword_id, level)
            .await?;
    }
    Ok(())
}

async fn insert_bibitem_refs(
    store: &impl FullImportRefStore,
    source_id: i64,
    target_bibkeys: &[String],
    ref_type: RefType,
) -> Result<(), String> {
    for bibkey in target_bibkeys {
        store
            .insert_bibitem_ref(source_id, bibkey, &ref_type)
            .await?;
    }
    Ok(())
}

// =============================================================================
// Full CSV export orchestration
// =============================================================================

/// Export all bibitems as a human-readable CSV matching the full import format.
pub async fn export_full_csv(
    bibitem_fetcher: &impl FullCsvBibitemFetcher,
    author_name_fetcher: &impl AuthorNameFetcher,
    reverse_name_fetcher: &impl ReverseNameMapFetcher,
    keyword_name_fetcher: &impl KeywordNameFetcher,
    junction_fetcher: &impl FullCsvJunctionFetcher,
) -> Result<String, HexforgeError> {
    // Fetch all bibitems
    let bibitems = bibitem_fetcher.fetch_all_bibitems().await?;

    // Build reverse lookup maps (ID -> name)
    let author_names = author_name_fetcher.fetch_author_names().await?;
    let journal_names = reverse_name_fetcher
        .fetch_reverse_name_map("journals")
        .await?;
    let publisher_names = reverse_name_fetcher
        .fetch_reverse_name_map("publishers")
        .await?;
    let institution_names = reverse_name_fetcher
        .fetch_reverse_name_map("institutions")
        .await?;
    let school_names = reverse_name_fetcher
        .fetch_reverse_name_map("schools")
        .await?;
    let series_names = reverse_name_fetcher
        .fetch_reverse_name_map("series")
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
    let bib_refs = junction_fetcher.fetch_bibitem_refs_batch(&bib_ids).await?;

    // Index junction data by bibitem_id
    let mut authors_by_bib: HashMap<i64, Vec<&BibitemAuthorsRow>> = HashMap::new();
    for row in &bib_authors {
        authors_by_bib.entry(row.bibitem_id).or_default().push(row);
    }
    let mut keywords_by_bib: HashMap<i64, Vec<&BibitemKeywordsRow>> = HashMap::new();
    for row in &bib_keywords {
        keywords_by_bib.entry(row.bibitem_id).or_default().push(row);
    }
    let mut refs_by_bib: HashMap<i64, Vec<&BibitemRefsRow>> = HashMap::new();
    for row in &bib_refs {
        refs_by_bib.entry(row.source_id).or_default().push(row);
    }

    // Build CSV using pure helper
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
        refs_by_bib: &refs_by_bib,
    };

    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record(FULL_CSV_HEADERS.split(','))
        .map_err(|e| HexforgeError::Internal(format!("CSV header error: {e}")))?;

    for bib in &bibitems {
        let record = build_export_record(bib, &export_ctx);

        wtr.write_record(&record)
            .map_err(|e| HexforgeError::Internal(format!("CSV write error: {e}")))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| HexforgeError::Internal(format!("CSV flush error: {e}")))?;
    String::from_utf8(bytes).map_err(|e| HexforgeError::Internal(format!("CSV UTF-8 error: {e}")))
}
