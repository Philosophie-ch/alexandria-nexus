//! Full CSV import — validate human-readable CSVs against the database.
//!
//! Parses ODS/CSV spreadsheets with human-readable names (not IDs),
//! resolves names against the database, and reports missing/ambiguous entities.

use std::collections::{HashMap, HashSet};

use hexforge::db_exports::{FromRow, query_as};
use hexforge::{HexforgeError, ValidationError};
use serde::Serialize;
use utoipa::ToSchema;

use crate::logic::csv_parsing::types::{FieldError, ParsedAuthor, ParsedBibRow, RowParseResult};
use crate::logic::csv_parsing::{CsvHeaders, parse_csv_row};
use crate::state::AppState;

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
    let mut all_authors: HashSet<AuthorNameKey> = HashSet::new();
    let mut all_journal_names: HashSet<String> = HashSet::new();
    let mut all_publisher_names: HashSet<String> = HashSet::new();
    let mut all_institution_names: HashSet<String> = HashSet::new();
    let mut all_school_names: HashSet<String> = HashSet::new();
    let mut all_series_names: HashSet<String> = HashSet::new();
    let mut all_keywords_l1: HashSet<String> = HashSet::new();
    let mut all_keywords_l2: HashSet<String> = HashSet::new();
    let mut all_keywords_l3: HashSet<String> = HashSet::new();
    let mut all_crossref_bibkeys: HashSet<String> = HashSet::new();
    let mut all_further_ref_bibkeys: HashSet<String> = HashSet::new();
    let mut all_depends_on_bibkeys: HashSet<String> = HashSet::new();
    let mut csv_bibkeys: HashSet<String> = HashSet::new();

    for row in &parsed_rows {
        csv_bibkeys.insert(row.bibkey.clone());

        for a in row
            .authors
            .iter()
            .chain(&row.editors)
            .chain(&row.guesteditors)
        {
            all_authors.insert(AuthorNameKey::from_parsed(a));
        }
        if let Some(p) = &row.person {
            all_authors.insert(AuthorNameKey::from_parsed(p));
        }

        if let Some(n) = &row.journal_name {
            all_journal_names.insert(n.clone());
        }
        if let Some(n) = &row.publisher_name {
            all_publisher_names.insert(n.clone());
        }
        if let Some(n) = &row.institution_name {
            all_institution_names.insert(n.clone());
        }
        if let Some(n) = &row.school_name {
            all_school_names.insert(n.clone());
        }
        if let Some(n) = &row.series_name {
            all_series_names.insert(n.clone());
        }
        for kw in &row.keywords.level_1 {
            all_keywords_l1.insert(kw.clone());
        }
        for kw in &row.keywords.level_2 {
            all_keywords_l2.insert(kw.clone());
        }
        for kw in &row.keywords.level_3 {
            all_keywords_l3.insert(kw.clone());
        }
        if let Some(cr) = &row.crossref_bibkey {
            all_crossref_bibkeys.insert(cr.clone());
        }
        for bk in &row.further_ref_bibkeys {
            all_further_ref_bibkeys.insert(bk.clone());
        }
        for bk in &row.depends_on_bibkeys {
            all_depends_on_bibkeys.insert(bk.clone());
        }
    }

    // 3. Batch DB lookups
    let author_map = batch_lookup_authors(pool).await?;
    let journal_map = batch_lookup_by_name_latex(pool, "journals").await?;
    let publisher_map = batch_lookup_by_name_latex(pool, "publishers").await?;
    let institution_map = batch_lookup_by_name_latex(pool, "institutions").await?;
    let school_map = batch_lookup_by_name_latex(pool, "schools").await?;
    let series_map = batch_lookup_by_name_latex(pool, "series").await?;
    let keyword_map = batch_lookup_keywords(pool).await?;
    let db_bibkeys = fetch_all_bibkeys(pool).await?;

    // 4. Classify authors
    let mut missing_authors = Vec::new();
    let mut ambiguous_authors = Vec::new();
    for key in &all_authors {
        match author_map.get(key) {
            None => missing_authors.push(format_author_key(key)),
            Some(ids) if ids.len() > 1 => {
                ambiguous_authors.push(AmbiguousAuthor {
                    name: format_author_key(key),
                    matching_ids: ids.clone(),
                });
            }
            _ => {} // exactly one match — good
        }
    }
    missing_authors.sort();

    // 5. Classify entities
    let missing_journals = find_missing_names(&all_journal_names, &journal_map);
    let missing_publishers = find_missing_names(&all_publisher_names, &publisher_map);
    let missing_institutions = find_missing_names(&all_institution_names, &institution_map);
    let missing_schools = find_missing_names(&all_school_names, &school_map);
    let missing_series = find_missing_names(&all_series_names, &series_map);

    // 6. Classify keywords
    let missing_keywords = MissingKeywords {
        level_1: find_missing_keywords(&all_keywords_l1, 1, &keyword_map),
        level_2: find_missing_keywords(&all_keywords_l2, 2, &keyword_map),
        level_3: find_missing_keywords(&all_keywords_l3, 3, &keyword_map),
    };

    // 7. Classify bibkey references
    let missing_crossrefs = find_missing_bibkeys(&all_crossref_bibkeys, &db_bibkeys);
    let missing_further_refs = find_missing_bibkeys(&all_further_ref_bibkeys, &db_bibkeys);
    let missing_depends_on = find_missing_bibkeys(&all_depends_on_bibkeys, &db_bibkeys);

    // 8. Stale bibitems: in DB but not in CSV
    let mut stale_bibitems: Vec<String> = db_bibkeys.difference(&csv_bibkeys).cloned().collect();
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
