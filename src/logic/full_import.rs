//! Full CSV import — validate, create missing entities, and import bibitems.
//!
//! Three operations for human-readable CSVs:
//! 1. **validate** — parse CSV, check all references, report issues
//! 2. **import entities** — create missing authors/journals/etc. from the CSV
//! 3. **import bibitems** — resolve names to IDs, upsert bibitems, delete stale

use std::collections::{HashMap, HashSet};

use hexforge::db_exports::{FromRow, query, query_as, query_scalar};
use hexforge::{DataSource, HexforgeError, ValidationError};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::{
    CreateAuthor, CreateBibItem, CreateInstitution, CreateJournal, CreateKeyword, CreatePublisher,
    CreateSchool, CreateSeries, create_author_transform, create_bib_item_transform,
    create_institution_transform, create_journal_transform, create_keyword_transform,
    create_publisher_transform, create_school_transform, create_series_transform,
};
use crate::logic::csv_parsing::types::{
    DateRangeSeparator, FieldError, ParsedAuthor, ParsedBibRow, ParsedDate, RowParseResult,
};
use crate::logic::csv_parsing::{CsvHeaders, parse_csv_row};
use crate::state::AppState;
use crate::validation::{
    validate_create_author, validate_create_bibitem, validate_create_institution,
    validate_create_journal, validate_create_keyword, validate_create_publisher,
    validate_create_school, validate_create_series,
};

// =============================================================================
// Response types
// =============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct ValidationReport {
    pub total_rows: usize,
    pub valid_rows: usize,
    pub errors: Vec<RowError>,
    pub missing_authors: Vec<String>,
    pub ambiguous_authors: Vec<AmbiguousAuthor>,
    pub missing_journals: Vec<String>,
    pub missing_publishers: Vec<String>,
    pub missing_institutions: Vec<String>,
    pub missing_schools: Vec<String>,
    pub missing_series: Vec<String>,
    pub missing_keywords: MissingKeywords,
    pub missing_crossrefs: Vec<String>,
    pub missing_further_refs: Vec<String>,
    pub missing_depends_on: Vec<String>,
    pub stale_bibitems: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RowError {
    pub row: usize,
    pub bibkey: Option<String>,
    pub errors: Vec<FieldError>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AmbiguousAuthor {
    pub name: String,
    pub matching_ids: Vec<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MissingKeywords {
    pub level_1: Vec<String>,
    pub level_2: Vec<String>,
    pub level_3: Vec<String>,
}

// =============================================================================
// Author lookup key
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AuthorNameKey {
    Named {
        family_name: String,
        given_name: Option<String>,
    },
    Mononym(String),
}

impl AuthorNameKey {
    fn from_parsed(author: &ParsedAuthor) -> Self {
        match author {
            ParsedAuthor::Named {
                family_name,
                given_name,
            } => AuthorNameKey::Named {
                family_name: family_name.clone(),
                given_name: given_name.clone(),
            },
            ParsedAuthor::Mononym(m) => AuthorNameKey::Mononym(m.clone()),
        }
    }
}

// =============================================================================
// Validate endpoint logic
// =============================================================================

/// Parse and validate a full CSV, checking all references against the database.
/// Returns a validation report. Does NOT modify anything.
pub async fn validate_full_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<ValidationReport, HexforgeError> {
    let pool = state.pool.pool();

    // 1. Parse all rows
    let (parsed_rows, row_errors) = parse_all_rows(&data)?;
    let total_rows = parsed_rows.len() + row_errors.len();

    // 2. Collect all unique names from successfully parsed rows
    let mut collected = CollectedNames::default();
    let mut csv_bibkeys: HashSet<String> = HashSet::new();
    for row in &parsed_rows {
        csv_bibkeys.insert(row.bibkey.clone());
        collected.collect_from_row(row);
    }

    // 3. Batch DB lookups
    let maps = build_lookup_maps(pool).await?;

    // 4. Classify authors
    let mut missing_authors = Vec::new();
    let mut ambiguous_authors = Vec::new();
    for key in &collected.authors {
        match maps.authors.get(key) {
            None => missing_authors.push(format_author_key(key)),
            Some(ids) if ids.len() > 1 => {
                ambiguous_authors.push(AmbiguousAuthor {
                    name: format_author_key(key),
                    matching_ids: ids.clone(),
                });
            }
            _ => {}
        }
    }
    missing_authors.sort();

    // 5. Classify entities
    let missing_journals = find_missing_names(&collected.journal_names, &maps.journals);
    let missing_publishers = find_missing_names(&collected.publisher_names, &maps.publishers);
    let missing_institutions = find_missing_names(&collected.institution_names, &maps.institutions);
    let missing_schools = find_missing_names(&collected.school_names, &maps.schools);
    let missing_series = find_missing_names(&collected.series_names, &maps.series);

    // 6. Classify keywords
    let missing_keywords = MissingKeywords {
        level_1: find_missing_keywords(&collected.keywords_l1, 1, &maps.keywords),
        level_2: find_missing_keywords(&collected.keywords_l2, 2, &maps.keywords),
        level_3: find_missing_keywords(&collected.keywords_l3, 3, &maps.keywords),
    };

    // 7. Classify bibkey references
    let missing_crossrefs = find_missing_bibkeys(&collected.crossref_bibkeys, &maps.bibkeys);
    let missing_further_refs = find_missing_bibkeys(&collected.further_ref_bibkeys, &maps.bibkeys);
    let missing_depends_on = find_missing_bibkeys(&collected.depends_on_bibkeys, &maps.bibkeys);

    // 8. Stale bibitems: in DB but not in CSV
    let mut stale_bibitems: Vec<String> = maps.bibkeys.difference(&csv_bibkeys).cloned().collect();
    stale_bibitems.sort();

    Ok(ValidationReport {
        total_rows,
        valid_rows: parsed_rows.len(),
        errors: row_errors,
        missing_authors,
        ambiguous_authors,
        missing_journals,
        missing_publishers,
        missing_institutions,
        missing_schools,
        missing_series,
        missing_keywords,
        missing_crossrefs,
        missing_further_refs,
        missing_depends_on,
        stale_bibitems,
    })
}

// =============================================================================
// CSV parsing orchestration
// =============================================================================

fn parse_all_rows(data: &[u8]) -> Result<(Vec<ParsedBibRow>, Vec<RowError>), HexforgeError> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(data);

    let headers = rdr
        .headers()
        .map_err(|e| {
            HexforgeError::Validation(ValidationError::custom(format!("invalid CSV headers: {e}")))
        })?
        .clone();

    let csv_headers = CsvHeaders::from_record(&headers);
    let mut parsed_rows = Vec::new();
    let mut row_errors = Vec::new();

    for (idx, result) in rdr.records().enumerate() {
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                row_errors.push(RowError {
                    row: idx + 2, // 1-indexed, skip header
                    bibkey: None,
                    errors: vec![FieldError {
                        field: "_csv".to_string(),
                        error: format!("malformed CSV row: {e}"),
                    }],
                });
                continue;
            }
        };

        match parse_csv_row(&csv_headers, &record) {
            RowParseResult::Ok(row) => parsed_rows.push(*row),
            RowParseResult::Err { bibkey, errors } => {
                row_errors.push(RowError {
                    row: idx + 2,
                    bibkey,
                    errors,
                });
            }
        }
    }

    Ok((parsed_rows, row_errors))
}

// =============================================================================
// Batch DB lookups
// =============================================================================

#[derive(FromRow)]
struct AuthorRow {
    id: i64,
    family_name_latex: Option<String>,
    given_name_latex: Option<String>,
    mononym_latex: Option<String>,
}

async fn batch_lookup_authors(
    pool: &hexforge::db_exports::PgPool,
) -> Result<HashMap<AuthorNameKey, Vec<i64>>, HexforgeError> {
    let rows: Vec<AuthorRow> =
        query_as("SELECT id, family_name_latex, given_name_latex, mononym_latex FROM authors")
            .fetch_all(pool)
            .await
            .map_err(HexforgeError::data_source)?;

    let mut map: HashMap<AuthorNameKey, Vec<i64>> = HashMap::new();
    for row in rows {
        let key = if let Some(mononym) = row.mononym_latex {
            AuthorNameKey::Mononym(mononym)
        } else if let Some(family) = row.family_name_latex {
            AuthorNameKey::Named {
                family_name: family,
                given_name: row.given_name_latex,
            }
        } else {
            continue;
        };
        map.entry(key).or_default().push(row.id);
    }
    Ok(map)
}

#[derive(FromRow)]
struct NameIdRow {
    id: i64,
    name_latex: String,
}

async fn batch_lookup_by_name_latex(
    pool: &hexforge::db_exports::PgPool,
    table: &str,
) -> Result<HashMap<String, i64>, HexforgeError> {
    let sql = format!("SELECT id, name_latex FROM {table}");
    let rows: Vec<NameIdRow> = query_as(&sql)
        .fetch_all(pool)
        .await
        .map_err(HexforgeError::data_source)?;

    Ok(rows.into_iter().map(|r| (r.name_latex, r.id)).collect())
}

#[derive(FromRow)]
struct KeywordRow {
    id: i64,
    name: String,
    level: i16,
}

async fn batch_lookup_keywords(
    pool: &hexforge::db_exports::PgPool,
) -> Result<HashMap<(String, i16), i64>, HexforgeError> {
    let rows: Vec<KeywordRow> = query_as("SELECT id, name, level FROM keywords")
        .fetch_all(pool)
        .await
        .map_err(HexforgeError::data_source)?;

    Ok(rows
        .into_iter()
        .map(|r| ((r.name, r.level), r.id))
        .collect())
}

#[derive(FromRow)]
struct BibkeyRow {
    bibkey: String,
}

async fn fetch_all_bibkeys(
    pool: &hexforge::db_exports::PgPool,
) -> Result<HashSet<String>, HexforgeError> {
    let rows: Vec<BibkeyRow> = query_as("SELECT bibkey FROM bibitems")
        .fetch_all(pool)
        .await
        .map_err(HexforgeError::data_source)?;

    Ok(rows.into_iter().map(|r| r.bibkey).collect())
}

// =============================================================================
// Classification helpers
// =============================================================================

fn find_missing_names(requested: &HashSet<String>, existing: &HashMap<String, i64>) -> Vec<String> {
    let mut missing: Vec<String> = requested
        .iter()
        .filter(|name| !existing.contains_key(*name))
        .cloned()
        .collect();
    missing.sort();
    missing
}

fn find_missing_keywords(
    requested: &HashSet<String>,
    level: i16,
    existing: &HashMap<(String, i16), i64>,
) -> Vec<String> {
    let mut missing: Vec<String> = requested
        .iter()
        .filter(|name| !existing.contains_key(&((*name).clone(), level)))
        .cloned()
        .collect();
    missing.sort();
    missing
}

fn find_missing_bibkeys(requested: &HashSet<String>, existing: &HashSet<String>) -> Vec<String> {
    let mut missing: Vec<String> = requested.difference(existing).cloned().collect();
    missing.sort();
    missing
}

fn format_author_key(key: &AuthorNameKey) -> String {
    match key {
        AuthorNameKey::Mononym(m) => m.clone(),
        AuthorNameKey::Named {
            family_name,
            given_name,
        } => match given_name {
            Some(g) => format!("{family_name}, {g}"),
            None => family_name.clone(),
        },
    }
}

// =============================================================================
// Import entities (create missing from CSV)
// =============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct EntityImportReport {
    pub created_authors: usize,
    pub created_journals: usize,
    pub created_publishers: usize,
    pub created_institutions: usize,
    pub created_schools: usize,
    pub created_series: usize,
    pub created_keywords: usize,
    pub errors: Vec<EntityImportError>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EntityImportError {
    pub entity_type: String,
    pub name: String,
    pub error: String,
}

/// Parse CSV, find entities referenced but not in DB, and create them.
/// Authors with exact duplicate names (same family+given or mononym) produce an error.
pub async fn import_entities_from_full_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<EntityImportReport, HexforgeError> {
    let pool = state.pool.pool();
    let (parsed_rows, _) = parse_all_rows(&data)?;

    let mut collected = CollectedNames::default();
    for row in &parsed_rows {
        collected.collect_from_row(row);
    }

    let maps = build_lookup_maps(pool).await?;

    let mut report = EntityImportReport {
        created_authors: 0,
        created_journals: 0,
        created_publishers: 0,
        created_institutions: 0,
        created_schools: 0,
        created_series: 0,
        created_keywords: 0,
        errors: Vec::new(),
    };

    // Authors
    for key in &collected.authors {
        if maps.authors.contains_key(key) {
            continue;
        }
        let name = format_author_key(key);
        match create_author_from_key(key, state).await {
            Ok(()) => report.created_authors += 1,
            Err(e) => report.errors.push(EntityImportError {
                entity_type: "author".to_string(),
                name,
                error: e,
            }),
        }
    }

    // Named entities
    report.created_journals += create_missing_named_entities(
        &collected.journal_names,
        &maps.journals,
        "journals",
        state,
        &mut report.errors,
    )
    .await;
    report.created_publishers += create_missing_named_entities(
        &collected.publisher_names,
        &maps.publishers,
        "publishers",
        state,
        &mut report.errors,
    )
    .await;
    report.created_institutions += create_missing_named_entities(
        &collected.institution_names,
        &maps.institutions,
        "institutions",
        state,
        &mut report.errors,
    )
    .await;
    report.created_schools += create_missing_named_entities(
        &collected.school_names,
        &maps.schools,
        "schools",
        state,
        &mut report.errors,
    )
    .await;
    report.created_series += create_missing_named_entities(
        &collected.series_names,
        &maps.series,
        "series",
        state,
        &mut report.errors,
    )
    .await;

    // Keywords
    for kw in &collected.keywords_l1 {
        if !maps.keywords.contains_key(&(kw.clone(), 1)) {
            match create_keyword(kw, 1, state).await {
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
            match create_keyword(kw, 2, state).await {
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
            match create_keyword(kw, 3, state).await {
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
// Import bibitems (full source-of-truth import)
// =============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct FullImportReport {
    pub imported: usize,
    pub updated: usize,
    pub deleted: usize,
    pub failed: usize,
    pub errors: Vec<RowError>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnresolvableNamesError {
    pub error: &'static str,
    pub message: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_authors: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ambiguous_authors: Vec<AmbiguousAuthor>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_journals: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_publishers: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_institutions: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_schools: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_series: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_keywords: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_bibkeys: Vec<String>,
}

pub enum FullImportResult {
    Success(FullImportReport),
    UnresolvableNames(UnresolvableNamesError),
    ParseErrors(Vec<RowError>),
}

/// Parse CSV, resolve all names to IDs, upsert bibitems + junctions.
/// CSV is source of truth: bibitems in DB but not in CSV get deleted.
pub async fn import_full_csv(
    state: &AppState,
    data: Vec<u8>,
) -> Result<FullImportResult, HexforgeError> {
    let pool = state.pool.pool();

    // 1. Parse
    let (parsed_rows, row_errors) = parse_all_rows(&data)?;
    if !row_errors.is_empty() {
        return Ok(FullImportResult::ParseErrors(row_errors));
    }

    // 2. Collect names and lookup
    let mut collected = CollectedNames::default();
    let mut csv_bibkeys = HashSet::new();
    for row in &parsed_rows {
        collected.collect_from_row(row);
        csv_bibkeys.insert(row.bibkey.clone());
    }

    let maps = build_lookup_maps(pool).await?;

    // 3. Check for unresolvable names
    let unresolvable = build_unresolvable(&collected, &maps, &csv_bibkeys);
    if unresolvable.has_any() {
        return Ok(FullImportResult::UnresolvableNames(unresolvable));
    }

    // 4. Build resolution context
    let ctx = ResolutionCtx {
        author_resolve: maps
            .authors
            .into_iter()
            .filter_map(|(k, ids)| {
                if ids.len() == 1 {
                    Some((k, ids[0]))
                } else {
                    None
                }
            })
            .collect(),
        journal_map: maps.journals,
        publisher_map: maps.publishers,
        institution_map: maps.institutions,
        school_map: maps.schools,
        series_map: maps.series,
        keyword_map: maps.keywords,
        existing_bibkeys: maps.bibkeys,
    };

    // 5. Delete stale bibitems
    let stale: Vec<String> = ctx
        .existing_bibkeys
        .difference(&csv_bibkeys)
        .cloned()
        .collect();
    let deleted = if stale.is_empty() {
        0
    } else {
        delete_bibitems_by_bibkeys(pool, &stale).await?
    };

    // 6. Upsert each bibitem
    let mut imported = 0usize;
    let mut updated = 0usize;
    let mut errors = Vec::new();

    for (idx, row) in parsed_rows.iter().enumerate() {
        let row_num = idx + 2;
        match upsert_bibitem_row(row, state, &ctx).await {
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
// Collected names helper
// =============================================================================

#[derive(Default)]
struct CollectedNames {
    authors: HashSet<AuthorNameKey>,
    journal_names: HashSet<String>,
    publisher_names: HashSet<String>,
    institution_names: HashSet<String>,
    school_names: HashSet<String>,
    series_names: HashSet<String>,
    keywords_l1: HashSet<String>,
    keywords_l2: HashSet<String>,
    keywords_l3: HashSet<String>,
    crossref_bibkeys: HashSet<String>,
    further_ref_bibkeys: HashSet<String>,
    depends_on_bibkeys: HashSet<String>,
}

impl CollectedNames {
    fn collect_from_row(&mut self, row: &ParsedBibRow) {
        for a in row
            .authors
            .iter()
            .chain(&row.editors)
            .chain(&row.guesteditors)
        {
            self.authors.insert(AuthorNameKey::from_parsed(a));
        }
        if let Some(p) = &row.person {
            self.authors.insert(AuthorNameKey::from_parsed(p));
        }
        if let Some(n) = &row.journal_name {
            self.journal_names.insert(n.clone());
        }
        if let Some(n) = &row.publisher_name {
            self.publisher_names.insert(n.clone());
        }
        if let Some(n) = &row.institution_name {
            self.institution_names.insert(n.clone());
        }
        if let Some(n) = &row.school_name {
            self.school_names.insert(n.clone());
        }
        if let Some(n) = &row.series_name {
            self.series_names.insert(n.clone());
        }
        self.keywords_l1
            .extend(row.keywords.level_1.iter().cloned());
        self.keywords_l2
            .extend(row.keywords.level_2.iter().cloned());
        self.keywords_l3
            .extend(row.keywords.level_3.iter().cloned());
        if let Some(cr) = &row.crossref_bibkey {
            self.crossref_bibkeys.insert(cr.clone());
        }
        self.further_ref_bibkeys
            .extend(row.further_ref_bibkeys.iter().cloned());
        self.depends_on_bibkeys
            .extend(row.depends_on_bibkeys.iter().cloned());
    }
}

// =============================================================================
// Entity creation helpers
// =============================================================================

fn generate_key(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

async fn create_author_from_key(key: &AuthorNameKey, state: &AppState) -> Result<(), String> {
    let (author_key, dto) = match key {
        AuthorNameKey::Named {
            family_name,
            given_name,
        } => {
            let ak = match given_name {
                Some(g) => format!("{}_{}", generate_key(family_name), generate_key(g)),
                None => generate_key(family_name),
            };
            let dto = CreateAuthor {
                author_key: ak.clone(),
                family_name_latex: Some(family_name.clone()),
                family_name_unicode: Some(family_name.clone()),
                family_name_simplified: Some(family_name.clone()),
                given_name_latex: given_name.clone(),
                given_name_unicode: given_name.clone(),
                given_name_simplified: given_name.clone(),
                mononym_latex: None,
                mononym_unicode: None,
                mononym_simplified: None,
                shorthand_latex: None,
                shorthand_unicode: None,
                shorthand_simplified: None,
                famous_name_latex: None,
                famous_name_unicode: None,
                famous_name_simplified: None,
            };
            (ak, dto)
        }
        AuthorNameKey::Mononym(m) => {
            let ak = generate_key(m);
            let dto = CreateAuthor {
                author_key: ak.clone(),
                family_name_latex: None,
                family_name_unicode: None,
                family_name_simplified: None,
                given_name_latex: None,
                given_name_unicode: None,
                given_name_simplified: None,
                mononym_latex: Some(m.clone()),
                mononym_unicode: Some(m.clone()),
                mononym_simplified: Some(m.clone()),
                shorthand_latex: None,
                shorthand_unicode: None,
                shorthand_simplified: None,
                famous_name_latex: None,
                famous_name_unicode: None,
                famous_name_simplified: None,
            };
            (ak, dto)
        }
    };

    validate_create_author(&dto).map_err(|e| e.to_string())?;
    let entity = create_author_transform(dto);
    state
        .author_ds
        .insert(entity)
        .await
        .map_err(|e| format!("failed to create author '{author_key}': {e}"))?;
    Ok(())
}

async fn create_missing_named_entities(
    requested: &HashSet<String>,
    existing: &HashMap<String, i64>,
    entity_type: &str,
    state: &AppState,
    errors: &mut Vec<EntityImportError>,
) -> usize {
    let mut created = 0;
    for name in requested {
        if existing.contains_key(name) {
            continue;
        }
        let key = generate_key(name);
        let result = match entity_type {
            "journals" => create_named_entity_journal(&key, name, state).await,
            "publishers" => create_named_entity_publisher(&key, name, state).await,
            "institutions" => create_named_entity_institution(&key, name, state).await,
            "schools" => create_named_entity_school(&key, name, state).await,
            "series" => create_named_entity_series(&key, name, state).await,
            _ => Err(format!("unknown entity type: {entity_type}")),
        };
        match result {
            Ok(()) => created += 1,
            Err(e) => errors.push(EntityImportError {
                entity_type: entity_type.to_string(),
                name: name.clone(),
                error: e,
            }),
        }
    }
    created
}

async fn create_named_entity_journal(
    key: &str,
    name: &str,
    state: &AppState,
) -> Result<(), String> {
    let dto = CreateJournal {
        journal_key: key.to_string(),
        name_latex: Some(name.to_string()),
        name_unicode: Some(name.to_string()),
        name_simplified: Some(name.to_string()),
        issn_print: None,
        issn_electronic: None,
    };
    validate_create_journal(&dto).map_err(|e| e.to_string())?;
    let entity = create_journal_transform(dto);
    state
        .journal_ds
        .insert(entity)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_named_entity_publisher(
    key: &str,
    name: &str,
    state: &AppState,
) -> Result<(), String> {
    let dto = CreatePublisher {
        publisher_key: key.to_string(),
        name_latex: Some(name.to_string()),
        name_unicode: Some(name.to_string()),
        name_simplified: Some(name.to_string()),
        default_address: None,
    };
    validate_create_publisher(&dto).map_err(|e| e.to_string())?;
    let entity = create_publisher_transform(dto);
    state
        .publisher_ds
        .insert(entity)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_named_entity_institution(
    key: &str,
    name: &str,
    state: &AppState,
) -> Result<(), String> {
    let dto = CreateInstitution {
        institution_key: key.to_string(),
        name_latex: Some(name.to_string()),
        name_unicode: Some(name.to_string()),
        name_simplified: Some(name.to_string()),
        default_address: None,
    };
    validate_create_institution(&dto).map_err(|e| e.to_string())?;
    let entity = create_institution_transform(dto);
    state
        .institution_ds
        .insert(entity)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_named_entity_school(key: &str, name: &str, state: &AppState) -> Result<(), String> {
    let dto = CreateSchool {
        school_key: key.to_string(),
        name_latex: Some(name.to_string()),
        name_unicode: Some(name.to_string()),
        name_simplified: Some(name.to_string()),
        default_address: None,
    };
    validate_create_school(&dto).map_err(|e| e.to_string())?;
    let entity = create_school_transform(dto);
    state
        .school_ds
        .insert(entity)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_named_entity_series(key: &str, name: &str, state: &AppState) -> Result<(), String> {
    let dto = CreateSeries {
        series_key: key.to_string(),
        name_latex: Some(name.to_string()),
        name_unicode: Some(name.to_string()),
        name_simplified: Some(name.to_string()),
    };
    validate_create_series(&dto).map_err(|e| e.to_string())?;
    let entity = create_series_transform(dto);
    state
        .series_ds
        .insert(entity)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_keyword(name: &str, level: i16, state: &AppState) -> Result<(), String> {
    let dto = CreateKeyword {
        name: name.to_string(),
        level,
    };
    validate_create_keyword(&dto).map_err(|e| e.to_string())?;
    let entity = create_keyword_transform(dto);
    state
        .keyword_ds
        .insert(entity)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// =============================================================================
// Resolution context (bundles all lookup maps to avoid too-many-args)
// =============================================================================

/// All DB lookup maps, built once per import operation.
struct LookupMaps {
    authors: HashMap<AuthorNameKey, Vec<i64>>,
    journals: HashMap<String, i64>,
    publishers: HashMap<String, i64>,
    institutions: HashMap<String, i64>,
    schools: HashMap<String, i64>,
    series: HashMap<String, i64>,
    keywords: HashMap<(String, i16), i64>,
    bibkeys: HashSet<String>,
}

/// Resolution context for bibitem upsert (single-match authors only).
struct ResolutionCtx {
    author_resolve: HashMap<AuthorNameKey, i64>,
    journal_map: HashMap<String, i64>,
    publisher_map: HashMap<String, i64>,
    institution_map: HashMap<String, i64>,
    school_map: HashMap<String, i64>,
    series_map: HashMap<String, i64>,
    keyword_map: HashMap<(String, i16), i64>,
    existing_bibkeys: HashSet<String>,
}

async fn build_lookup_maps(
    pool: &hexforge::db_exports::PgPool,
) -> Result<LookupMaps, HexforgeError> {
    Ok(LookupMaps {
        authors: batch_lookup_authors(pool).await?,
        journals: batch_lookup_by_name_latex(pool, "journals").await?,
        publishers: batch_lookup_by_name_latex(pool, "publishers").await?,
        institutions: batch_lookup_by_name_latex(pool, "institutions").await?,
        schools: batch_lookup_by_name_latex(pool, "schools").await?,
        series: batch_lookup_by_name_latex(pool, "series").await?,
        keywords: batch_lookup_keywords(pool).await?,
        bibkeys: fetch_all_bibkeys(pool).await?,
    })
}

// =============================================================================
// Unresolvable names check
// =============================================================================

impl UnresolvableNamesError {
    fn has_any(&self) -> bool {
        !self.missing_authors.is_empty()
            || !self.ambiguous_authors.is_empty()
            || !self.missing_journals.is_empty()
            || !self.missing_publishers.is_empty()
            || !self.missing_institutions.is_empty()
            || !self.missing_schools.is_empty()
            || !self.missing_series.is_empty()
            || !self.missing_keywords.is_empty()
            || !self.missing_bibkeys.is_empty()
    }
}

fn build_unresolvable(
    collected: &CollectedNames,
    maps: &LookupMaps,
    csv_bibkeys: &HashSet<String>,
) -> UnresolvableNamesError {
    let mut missing_authors = Vec::new();
    let mut ambiguous_authors = Vec::new();
    for key in &collected.authors {
        match maps.authors.get(key) {
            None => missing_authors.push(format_author_key(key)),
            Some(ids) if ids.len() > 1 => ambiguous_authors.push(AmbiguousAuthor {
                name: format_author_key(key),
                matching_ids: ids.clone(),
            }),
            _ => {}
        }
    }
    missing_authors.sort();

    // All referenced bibkeys (crossrefs, further_refs, depends_on) must be either
    // in the CSV itself or already in the DB
    let all_ref_bibkeys: HashSet<&String> = collected
        .crossref_bibkeys
        .iter()
        .chain(&collected.further_ref_bibkeys)
        .chain(&collected.depends_on_bibkeys)
        .collect();
    let mut missing_bibkeys: Vec<String> = all_ref_bibkeys
        .iter()
        .filter(|bk| !csv_bibkeys.contains(**bk) && !maps.bibkeys.contains(**bk))
        .map(|bk| (*bk).clone())
        .collect();
    missing_bibkeys.sort();

    let mut missing_kw = Vec::new();
    for kw in &collected.keywords_l1 {
        if !maps.keywords.contains_key(&(kw.clone(), 1)) {
            missing_kw.push(format!("{kw} (level 1)"));
        }
    }
    for kw in &collected.keywords_l2 {
        if !maps.keywords.contains_key(&(kw.clone(), 2)) {
            missing_kw.push(format!("{kw} (level 2)"));
        }
    }
    for kw in &collected.keywords_l3 {
        if !maps.keywords.contains_key(&(kw.clone(), 3)) {
            missing_kw.push(format!("{kw} (level 3)"));
        }
    }
    missing_kw.sort();

    UnresolvableNamesError {
        error: "unresolvable_names",
        message: "Some referenced entities could not be resolved",
        missing_authors,
        ambiguous_authors,
        missing_journals: find_missing_names(&collected.journal_names, &maps.journals),
        missing_publishers: find_missing_names(&collected.publisher_names, &maps.publishers),
        missing_institutions: find_missing_names(&collected.institution_names, &maps.institutions),
        missing_schools: find_missing_names(&collected.school_names, &maps.schools),
        missing_series: find_missing_names(&collected.series_names, &maps.series),
        missing_keywords: missing_kw,
        missing_bibkeys,
    }
}

// =============================================================================
// Bibitem upsert
// =============================================================================

/// Resolve a parsed row to IDs and upsert the bibitem + junctions.
/// Returns Ok(true) if updated, Ok(false) if inserted.
async fn upsert_bibitem_row(
    row: &ParsedBibRow,
    state: &AppState,
    ctx: &ResolutionCtx,
) -> Result<bool, String> {
    let pool = state.pool.pool();
    let was_update = ctx.existing_bibkeys.contains(&row.bibkey);

    // Build CreateBibItem DTO
    let dto = build_bibitem_dto(row, ctx)?;
    validate_create_bibitem(&dto).map_err(|e| e.to_string())?;

    if was_update {
        // Delete existing bibitem (cascade deletes junctions), then re-insert
        query("DELETE FROM bibitems WHERE bibkey = $1")
            .bind(&row.bibkey)
            .execute(pool)
            .await
            .map_err(|e| format!("failed to delete old bibitem: {e}"))?;
    }

    let entity = create_bib_item_transform(dto);
    let inserted = state
        .bibitem_ds
        .insert(entity)
        .await
        .map_err(|e| format!("failed to insert bibitem: {e}"))?;

    let bibitem_id: i64 = inserted.id;

    // Insert author junctions
    insert_author_junctions(
        pool,
        bibitem_id,
        &row.authors,
        "author",
        &ctx.author_resolve,
    )
    .await?;
    insert_author_junctions(
        pool,
        bibitem_id,
        &row.editors,
        "editor",
        &ctx.author_resolve,
    )
    .await?;
    insert_author_junctions(
        pool,
        bibitem_id,
        &row.guesteditors,
        "guesteditor",
        &ctx.author_resolve,
    )
    .await?;

    // Insert keyword junctions
    insert_keyword_junctions(pool, bibitem_id, row, &ctx.keyword_map).await?;

    // Insert bibitem refs (further_refs, depends_on)
    insert_bibitem_refs(pool, bibitem_id, &row.further_ref_bibkeys, "further_ref").await?;
    insert_bibitem_refs(pool, bibitem_id, &row.depends_on_bibkeys, "depends_on").await?;

    Ok(was_update)
}

fn build_bibitem_dto(row: &ParsedBibRow, ctx: &ResolutionCtx) -> Result<CreateBibItem, String> {
    let person_id = row.person.as_ref().and_then(|p| {
        let key = AuthorNameKey::from_parsed(p);
        ctx.author_resolve.get(&key).copied()
    });

    let mut dto = CreateBibItem {
        bibkey: row.bibkey.clone(),
        entry_type: row.entry_type,
        date_year: None,
        date_year_2_hyphen: None,
        date_year_2_slash: None,
        date_month: None,
        date_day: None,
        date_is_no_date: None,
        pubstate: row.pubstate,
        title_latex: row.title.clone(),
        title_unicode: row.title.clone(),
        title_simplified: row.title.clone(),
        booktitle_latex: row.booktitle.clone(),
        booktitle_unicode: row.booktitle.clone(),
        booktitle_simplified: row.booktitle.clone(),
        journal_id: row
            .journal_name
            .as_ref()
            .and_then(|n| ctx.journal_map.get(n).copied()),
        publisher_id: row
            .publisher_name
            .as_ref()
            .and_then(|n| ctx.publisher_map.get(n).copied()),
        address: row.address.clone(),
        volume: row.volume.clone(),
        number: row.number.clone(),
        pages: row.pages.clone(),
        eid: row.eid.clone(),
        series_id: row
            .series_name
            .as_ref()
            .and_then(|n| ctx.series_map.get(n).copied()),
        edition: row.edition.clone(),
        institution_id: row
            .institution_name
            .as_ref()
            .and_then(|n| ctx.institution_map.get(n).copied()),
        school_id: row
            .school_name
            .as_ref()
            .and_then(|n| ctx.school_map.get(n).copied()),
        type_field: row.type_field.clone(),
        doi: row.doi.clone(),
        url: row.url.clone(),
        eprint: row.eprint.clone(),
        urn: row.urn.clone(),
        crossref_id: None, // resolved after all bibitems inserted — skip for now
        issuetitle_latex: row.issuetitle.clone(),
        issuetitle_unicode: row.issuetitle.clone(),
        note_latex: row.note.clone(),
        note_unicode: row.note.clone(),
        extra_note_latex: row.extra_note.clone(),
        extra_note_unicode: row.extra_note.clone(),
        langid: row.langid,
        is_translation: Some(row.is_translation),
        epoch: row.epoch,
        options: row.options.clone(),
        shorthand: row.shorthand.clone(),
        person_id,
        has_fulltext: Some(row.has_fulltext),
        fulltext_path: None,
    };

    apply_date_to_dto(&row.date, &mut dto);
    Ok(dto)
}

fn apply_date_to_dto(date: &ParsedDate, dto: &mut CreateBibItem) {
    match date {
        ParsedDate::NoDate => {
            dto.date_is_no_date = Some(true);
        }
        ParsedDate::Year(y) => {
            dto.date_year = Some(*y);
        }
        ParsedDate::YearRange {
            year,
            year2,
            separator,
        } => {
            dto.date_year = Some(*year);
            match separator {
                DateRangeSeparator::Hyphen => dto.date_year_2_hyphen = Some(*year2),
                DateRangeSeparator::Slash => dto.date_year_2_slash = Some(*year2),
            }
        }
        ParsedDate::FullDate { year, month, day } => {
            dto.date_year = Some(*year);
            dto.date_month = Some(*month);
            dto.date_day = Some(*day);
        }
    }
}

// =============================================================================
// Junction insertion helpers
// =============================================================================

async fn insert_author_junctions(
    pool: &hexforge::db_exports::PgPool,
    bibitem_id: i64,
    authors: &[ParsedAuthor],
    role: &str,
    resolve: &HashMap<AuthorNameKey, i64>,
) -> Result<(), String> {
    for (position, author) in authors.iter().enumerate() {
        let key = AuthorNameKey::from_parsed(author);
        let author_id = resolve
            .get(&key)
            .ok_or_else(|| format!("could not resolve author: {}", author.display_name()))?;
        let pos = i16::try_from(position).map_err(|_| "too many authors")?;
        query(
            "INSERT INTO bibitem_authors (bibitem_id, author_id, role, position) \
             VALUES ($1, $2, $3::author_role, $4) \
             ON CONFLICT (bibitem_id, author_id, role) DO UPDATE SET position = $4",
        )
        .bind(bibitem_id)
        .bind(author_id)
        .bind(role)
        .bind(pos)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to link author: {e}"))?;
    }
    Ok(())
}

async fn insert_keyword_junctions(
    pool: &hexforge::db_exports::PgPool,
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
        query(
            "INSERT INTO bibitem_keywords (bibitem_id, keyword_id, keyword_level) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (bibitem_id, keyword_id) DO NOTHING",
        )
        .bind(bibitem_id)
        .bind(keyword_id)
        .bind(level)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to link keyword: {e}"))?;
    }
    Ok(())
}

async fn insert_bibitem_refs(
    pool: &hexforge::db_exports::PgPool,
    source_id: i64,
    target_bibkeys: &[String],
    ref_type: &str,
) -> Result<(), String> {
    for bibkey in target_bibkeys {
        // Resolve bibkey to ID
        let target_id: Option<i64> = query_scalar("SELECT id FROM bibitems WHERE bibkey = $1")
            .bind(bibkey)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("failed to resolve bibkey '{bibkey}': {e}"))?;

        let target_id = match target_id {
            Some(id) => id,
            None => continue, // referenced bibitem doesn't exist yet (may be in same batch)
        };

        query(
            "INSERT INTO bibitem_refs (source_id, target_id, ref_type) \
             VALUES ($1, $2, $3::ref_type) \
             ON CONFLICT (source_id, target_id, ref_type) DO NOTHING",
        )
        .bind(source_id)
        .bind(target_id)
        .bind(ref_type)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to insert ref: {e}"))?;
    }
    Ok(())
}

async fn delete_bibitems_by_bibkeys(
    pool: &hexforge::db_exports::PgPool,
    bibkeys: &[String],
) -> Result<usize, HexforgeError> {
    let result = query("DELETE FROM bibitems WHERE bibkey = ANY($1)")
        .bind(bibkeys)
        .execute(pool)
        .await
        .map_err(HexforgeError::data_source)?;
    Ok(usize::try_from(result.rows_affected()).unwrap_or(0))
}
