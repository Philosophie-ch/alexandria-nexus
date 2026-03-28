//! Export handlers for all entity types and bibitems in CSV format.
//!
//! `POST /api/v1/admin/export/{entity}`
//!
//! Request body carries selection criteria (all, IDs, or keys/bibkeys).
//! Bibitem export supports two formats: "expanded" (human-readable) and "ids" (machine-readable).
//!
//! Requires Admin permission.

use std::collections::{HashMap, HashSet};

use hexforge::axum_exports::{IntoResponse, Json, Response, State, StatusCode, header};
use hexforge::db_exports::{FromRow, query_as};
use hexforge::{HexforgeError, WhereClause};
use serde::{Deserialize, Serialize};

use crate::domain::{Author, BibItem, Institution, Journal, Keyword, Publisher, School, Series};
use crate::state::AppState;

// =============================================================================
// Request types
// =============================================================================

/// Bibitem export format.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    #[default]
    Expanded,
    Ids,
}

/// Request body for bibitem export.
#[derive(Debug, Deserialize)]
pub struct BibitemExportRequest {
    #[serde(default)]
    pub format: ExportFormat,
    #[serde(default)]
    pub all: bool,
    pub ids: Option<Vec<i64>>,
    pub bibkeys: Option<Vec<String>>,
}

/// Request body for entity export.
#[derive(Debug, Deserialize)]
pub struct EntityExportRequest {
    #[serde(default)]
    pub all: bool,
    pub ids: Option<Vec<i64>>,
    pub keys: Option<Vec<String>>,
}

// =============================================================================
// Error response types
// =============================================================================

/// 422 error response when requested IDs/keys are not found.
#[derive(Debug, Serialize)]
struct NotFoundError {
    error: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_bibkeys: Option<Vec<String>>,
}

/// 400 error response for invalid request.
#[derive(Debug, Serialize)]
struct BadRequestError {
    error: &'static str,
    message: &'static str,
}

// =============================================================================
// Junction row types for batch queries
// =============================================================================

/// A row from bibitem_authors junction table.
#[derive(Debug, FromRow)]
struct BibitemAuthorRow {
    bibitem_id: i64,
    author_id: i64,
    role: String,
    position: i16,
}

/// A row from bibitem_keywords junction table.
#[derive(Debug, FromRow)]
struct BibitemKeywordRow {
    bibitem_id: i64,
    keyword_id: i64,
    keyword_level: i16,
}

// =============================================================================
// CSV response helper
// =============================================================================

/// Build a CSV download response from raw CSV bytes.
fn csv_response(csv_data: Vec<u8>, filename: &str) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        csv_data,
    )
        .into_response()
}

/// Return a 422 not-found error response for missing IDs.
fn not_found_ids_response(missing: Vec<i64>) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(NotFoundError {
            error: "not_found",
            message: format!("{} requested ID(s) not found", missing.len()),
            missing_ids: Some(missing),
            missing_keys: None,
            missing_bibkeys: None,
        }),
    )
        .into_response()
}

/// Return a 422 not-found error response for missing keys.
fn not_found_keys_response(missing: Vec<String>) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(NotFoundError {
            error: "not_found",
            message: format!("{} requested key(s) not found", missing.len()),
            missing_ids: None,
            missing_keys: Some(missing),
            missing_bibkeys: None,
        }),
    )
        .into_response()
}

/// Return a 422 not-found error response for missing bibkeys.
fn not_found_bibkeys_response(missing: Vec<String>) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(NotFoundError {
            error: "not_found",
            message: format!("{} requested bibkey(s) not found", missing.len()),
            missing_ids: None,
            missing_keys: None,
            missing_bibkeys: Some(missing),
        }),
    )
        .into_response()
}

/// Return a 400 bad-request error response.
fn bad_request_response() -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(BadRequestError {
            error: "bad_request",
            message: "Request must specify \"all\": true, \"ids\", or \"keys\"/\"bibkeys\"",
        }),
    )
        .into_response()
}

// =============================================================================
// Helper: format optional values for CSV
// =============================================================================

fn opt_str(v: &Option<String>) -> &str {
    v.as_deref().unwrap_or("")
}

fn opt_i64(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

fn opt_i16(v: Option<i16>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

fn opt_display<T: std::fmt::Display>(v: &Option<T>) -> String {
    v.as_ref().map(|x| x.to_string()).unwrap_or_default()
}

// =============================================================================
// Entity export handlers
// =============================================================================

/// Export authors as CSV.
///
/// `POST /api/v1/admin/export/authors`
pub async fn export_authors(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let authors =
        match fetch_entities_by_request(&state.author_ds, "author_key", req.all, req.ids, req.keys)
            .await?
        {
            Ok(entities) => entities,
            Err(resp) => return Ok(resp),
        };

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "author_key",
        "given_name_latex",
        "given_name_unicode",
        "given_name_simplified",
        "family_name_latex",
        "family_name_unicode",
        "family_name_simplified",
        "mononym_latex",
        "mononym_unicode",
        "mononym_simplified",
        "shorthand_latex",
        "shorthand_unicode",
        "shorthand_simplified",
        "famous_name_latex",
        "famous_name_unicode",
        "famous_name_simplified",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for a in &authors {
        wtr.write_record([
            &a.id.to_string(),
            &a.author_key,
            opt_str(&a.given_name_latex),
            opt_str(&a.given_name_unicode),
            opt_str(&a.given_name_simplified),
            opt_str(&a.family_name_latex),
            opt_str(&a.family_name_unicode),
            opt_str(&a.family_name_simplified),
            opt_str(&a.mononym_latex),
            opt_str(&a.mononym_unicode),
            opt_str(&a.mononym_simplified),
            opt_str(&a.shorthand_latex),
            opt_str(&a.shorthand_unicode),
            opt_str(&a.shorthand_simplified),
            opt_str(&a.famous_name_latex),
            opt_str(&a.famous_name_unicode),
            opt_str(&a.famous_name_simplified),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    Ok(csv_response(data, "authors.csv"))
}

/// Export journals as CSV.
///
/// `POST /api/v1/admin/export/journals`
pub async fn export_journals(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let journals = match fetch_entities_by_request(
        &state.journal_ds,
        "journal_key",
        req.all,
        req.ids,
        req.keys,
    )
    .await?
    {
        Ok(entities) => entities,
        Err(resp) => return Ok(resp),
    };

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "journal_key",
        "name_latex",
        "name_unicode",
        "name_simplified",
        "issn_print",
        "issn_electronic",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for j in &journals {
        wtr.write_record([
            &j.id.to_string(),
            &j.journal_key,
            &j.name_latex,
            &j.name_unicode,
            &j.name_simplified,
            opt_str(&j.issn_print),
            opt_str(&j.issn_electronic),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    Ok(csv_response(data, "journals.csv"))
}

/// Export publishers as CSV.
///
/// `POST /api/v1/admin/export/publishers`
pub async fn export_publishers(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let publishers = match fetch_entities_by_request(
        &state.publisher_ds,
        "publisher_key",
        req.all,
        req.ids,
        req.keys,
    )
    .await?
    {
        Ok(entities) => entities,
        Err(resp) => return Ok(resp),
    };

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "publisher_key",
        "name_latex",
        "name_unicode",
        "name_simplified",
        "default_address",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for p in &publishers {
        wtr.write_record([
            &p.id.to_string(),
            &p.publisher_key,
            &p.name_latex,
            &p.name_unicode,
            &p.name_simplified,
            opt_str(&p.default_address),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    Ok(csv_response(data, "publishers.csv"))
}

/// Export institutions as CSV.
///
/// `POST /api/v1/admin/export/institutions`
pub async fn export_institutions(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let institutions = match fetch_entities_by_request(
        &state.institution_ds,
        "institution_key",
        req.all,
        req.ids,
        req.keys,
    )
    .await?
    {
        Ok(entities) => entities,
        Err(resp) => return Ok(resp),
    };

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "institution_key",
        "name_latex",
        "name_unicode",
        "name_simplified",
        "default_address",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for inst in &institutions {
        wtr.write_record([
            &inst.id.to_string(),
            &inst.institution_key,
            &inst.name_latex,
            &inst.name_unicode,
            &inst.name_simplified,
            opt_str(&inst.default_address),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    Ok(csv_response(data, "institutions.csv"))
}

/// Export schools as CSV.
///
/// `POST /api/v1/admin/export/schools`
pub async fn export_schools(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let schools =
        match fetch_entities_by_request(&state.school_ds, "school_key", req.all, req.ids, req.keys)
            .await?
        {
            Ok(entities) => entities,
            Err(resp) => return Ok(resp),
        };

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "school_key",
        "name_latex",
        "name_unicode",
        "name_simplified",
        "default_address",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for s in &schools {
        wtr.write_record([
            &s.id.to_string(),
            &s.school_key,
            &s.name_latex,
            &s.name_unicode,
            &s.name_simplified,
            opt_str(&s.default_address),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    Ok(csv_response(data, "schools.csv"))
}

/// Export series as CSV.
///
/// `POST /api/v1/admin/export/series`
pub async fn export_series(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let series_list =
        match fetch_entities_by_request(&state.series_ds, "series_key", req.all, req.ids, req.keys)
            .await?
        {
            Ok(entities) => entities,
            Err(resp) => return Ok(resp),
        };

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "series_key",
        "name_latex",
        "name_unicode",
        "name_simplified",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for s in &series_list {
        wtr.write_record([
            &s.id.to_string(),
            &s.series_key,
            &s.name_latex,
            &s.name_unicode,
            &s.name_simplified,
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    Ok(csv_response(data, "series.csv"))
}

/// Export keywords as CSV.
///
/// `POST /api/v1/admin/export/keywords`
pub async fn export_keywords(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    // Keywords don't have a unique "key" column — identity is (name, level).
    // We treat `keys` as keyword names for filtering purposes.
    let keywords: Vec<Keyword> = if req.all {
        fetch_all_via_sql(state.pool.pool(), "keywords").await?
    } else if let Some(ref id_list) = req.ids {
        let found = state
            .keyword_ds
            .find_by_ids(id_list)
            .await
            .map_err(HexforgeError::data_source)?;
        let found_ids: HashSet<i64> = found.iter().map(|k| k.id).collect();
        let missing: Vec<i64> = id_list
            .iter()
            .filter(|id| !found_ids.contains(id))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Ok(not_found_ids_response(missing));
        }
        found
    } else if let Some(ref key_list) = req.keys {
        let mut all = Vec::new();
        for name in key_list {
            let found: Vec<Keyword> = state
                .keyword_ds
                .find_many(WhereClause::new("name = $1").bind(name.clone()))
                .await
                .map_err(HexforgeError::data_source)?;
            all.extend(found);
        }
        let found_names: HashSet<&str> = all.iter().map(|k| k.name.as_str()).collect();
        let missing: Vec<String> = key_list
            .iter()
            .filter(|k| !found_names.contains(k.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Ok(not_found_keys_response(missing));
        }
        all
    } else {
        return Ok(bad_request_response());
    };

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(["id", "name", "level"])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for kw in &keywords {
        wtr.write_record([&kw.id.to_string(), &kw.name, &kw.level.to_string()])
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    Ok(csv_response(data, "keywords.csv"))
}

// =============================================================================
// Bibitem export handler
// =============================================================================

/// Export bibitems as CSV.
///
/// `POST /api/v1/admin/export/bibitems`
///
/// Supports two formats:
/// - `"expanded"` (default): human-readable with resolved names
/// - `"ids"`: machine-readable with raw foreign key IDs
pub async fn export_bibitems(
    State(state): State<AppState>,
    Json(req): Json<BibitemExportRequest>,
) -> Result<Response, HexforgeError> {
    // 1. Fetch bibitems based on selection criteria
    let bibitems: Vec<BibItem> = if req.all {
        fetch_all_via_sql(state.pool.pool(), "bibitems").await?
    } else if let Some(ref id_list) = req.ids {
        let found = state
            .bibitem_ds
            .find_by_ids(id_list)
            .await
            .map_err(HexforgeError::data_source)?;
        let found_ids: HashSet<i64> = found.iter().map(|b| b.id).collect();
        let missing: Vec<i64> = id_list
            .iter()
            .filter(|id| !found_ids.contains(id))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Ok(not_found_ids_response(missing));
        }
        found
    } else if let Some(ref bibkey_list) = req.bibkeys {
        let mut all = Vec::new();
        for bibkey in bibkey_list {
            let found = state
                .bibitem_ds
                .find_one(WhereClause::new("bibkey = $1").bind(bibkey.clone()))
                .await
                .map_err(HexforgeError::data_source)?;
            if let Some(item) = found {
                all.push(item);
            }
        }
        let found_keys: HashSet<&str> = all.iter().map(|b| b.bibkey.as_str()).collect();
        let missing: Vec<String> = bibkey_list
            .iter()
            .filter(|k| !found_keys.contains(k.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Ok(not_found_bibkeys_response(missing));
        }
        all
    } else {
        return Ok(bad_request_response());
    };

    match req.format {
        ExportFormat::Ids => export_bibitems_ids_format(&bibitems, &state).await,
        ExportFormat::Expanded => export_bibitems_expanded_format(&bibitems, &state).await,
    }
}

// =============================================================================
// Bibitem IDs format
// =============================================================================

/// IDs format header columns.
const IDS_FORMAT_HEADER: &[&str] = &[
    "id",
    "entry_type",
    "bibkey",
    "options",
    "shorthand",
    "date_year",
    "pubstate",
    "title_latex",
    "title_unicode",
    "title_simplified",
    "booktitle_latex",
    "booktitle_unicode",
    "booktitle_simplified",
    "crossref_id",
    "journal_id",
    "volume",
    "number",
    "pages",
    "eid",
    "series_id",
    "address",
    "institution_id",
    "school_id",
    "publisher_id",
    "type_field",
    "edition",
    "note_latex",
    "note_unicode",
    "issuetitle_latex",
    "issuetitle_unicode",
    "extra_note_latex",
    "extra_note_unicode",
    "urn",
    "eprint",
    "doi",
    "url",
    "langid",
    "is_translation",
    "epoch",
    "author_ids",
    "editor_ids",
    "guesteditor_ids",
    "keyword_ids",
];

/// Export bibitems in IDs format (machine-readable with foreign key IDs).
async fn export_bibitems_ids_format(
    bibitems: &[BibItem],
    state: &AppState,
) -> Result<Response, HexforgeError> {
    if bibitems.is_empty() {
        let mut wtr = csv::Writer::from_writer(vec![]);
        wtr.write_record(IDS_FORMAT_HEADER)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
        let data = wtr
            .into_inner()
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
        return Ok(csv_response(data, "bibitems.csv"));
    }

    let bibitem_ids: Vec<i64> = bibitems.iter().map(|b| b.id).collect();

    // Batch-fetch junction data
    let author_rows = fetch_bibitem_authors_batch(state, &bibitem_ids).await?;
    let keyword_rows = fetch_bibitem_keywords_batch(state, &bibitem_ids).await?;

    // Group authors by bibitem_id
    let mut authors_by_bibitem: HashMap<i64, Vec<&BibitemAuthorRow>> = HashMap::new();
    for row in &author_rows {
        authors_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    // Group keywords by bibitem_id
    let mut keywords_by_bibitem: HashMap<i64, Vec<&BibitemKeywordRow>> = HashMap::new();
    for row in &keyword_rows {
        keywords_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(IDS_FORMAT_HEADER)
        .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for bib in bibitems {
        let bib_authors = authors_by_bibitem.get(&bib.id);

        let author_ids = format_role_ids(bib_authors, "author");
        let editor_ids = format_role_ids(bib_authors, "editor");
        let guesteditor_ids = format_role_ids(bib_authors, "guesteditor");

        let keyword_ids = keywords_by_bibitem
            .get(&bib.id)
            .map(|rows| {
                let mut ids: Vec<String> = rows.iter().map(|r| r.keyword_id.to_string()).collect();
                ids.sort();
                ids.join(";")
            })
            .unwrap_or_default();

        wtr.write_record([
            &bib.id.to_string(),
            &bib.entry_type.to_string(),
            &bib.bibkey,
            opt_str(&bib.options),
            opt_str(&bib.shorthand),
            &opt_i16(bib.date_year),
            &opt_display(&bib.pubstate),
            &bib.title_latex,
            &bib.title_unicode,
            &bib.title_simplified,
            opt_str(&bib.booktitle_latex),
            opt_str(&bib.booktitle_unicode),
            opt_str(&bib.booktitle_simplified),
            &opt_i64(bib.crossref_id),
            &opt_i64(bib.journal_id),
            opt_str(&bib.volume),
            opt_str(&bib.number),
            opt_str(&bib.pages),
            opt_str(&bib.eid),
            &opt_i64(bib.series_id),
            opt_str(&bib.address),
            &opt_i64(bib.institution_id),
            &opt_i64(bib.school_id),
            &opt_i64(bib.publisher_id),
            opt_str(&bib.type_field),
            opt_str(&bib.edition),
            opt_str(&bib.note_latex),
            opt_str(&bib.note_unicode),
            opt_str(&bib.issuetitle_latex),
            opt_str(&bib.issuetitle_unicode),
            opt_str(&bib.extra_note_latex),
            opt_str(&bib.extra_note_unicode),
            opt_str(&bib.urn),
            opt_str(&bib.eprint),
            opt_str(&bib.doi),
            opt_str(&bib.url),
            &opt_display(&bib.langid),
            &bib.is_translation.to_string(),
            &opt_display(&bib.epoch),
            &author_ids,
            &editor_ids,
            &guesteditor_ids,
            &keyword_ids,
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    Ok(csv_response(data, "bibitems.csv"))
}

/// Format author IDs for a given role, sorted by position.
fn format_role_ids(bib_authors: Option<&Vec<&BibitemAuthorRow>>, role: &str) -> String {
    bib_authors
        .map(|rows| {
            let mut filtered: Vec<&BibitemAuthorRow> =
                rows.iter().filter(|r| r.role == role).copied().collect();
            filtered.sort_by_key(|r| r.position);
            filtered
                .iter()
                .map(|r| r.author_id.to_string())
                .collect::<Vec<_>>()
                .join(";")
        })
        .unwrap_or_default()
}

// =============================================================================
// Bibitem expanded format
// =============================================================================

/// Expanded format header columns.
const EXPANDED_FORMAT_HEADER: &[&str] = &[
    "entry_type",
    "bibkey",
    "author",
    "editor",
    "guesteditor",
    "options",
    "shorthand",
    "date_year",
    "pubstate",
    "title_latex",
    "title_unicode",
    "title_simplified",
    "booktitle_latex",
    "booktitle_unicode",
    "booktitle_simplified",
    "crossref",
    "journal",
    "volume",
    "number",
    "pages",
    "eid",
    "series",
    "address",
    "institution",
    "school",
    "publisher",
    "type_field",
    "edition",
    "note_latex",
    "note_unicode",
    "issuetitle_latex",
    "issuetitle_unicode",
    "extra_note_latex",
    "extra_note_unicode",
    "urn",
    "eprint",
    "doi",
    "url",
    "kw_level1",
    "kw_level2",
    "kw_level3",
    "epoch",
    "langid",
    "is_translation",
];

/// Export bibitems in expanded format (human-readable with resolved names).
///
/// N+1 prevention:
/// 1. Fetch all bibitems
/// 2. Collect all unique FK IDs (journal_ids, publisher_ids, etc.)
/// 3. Batch-fetch all related entities into HashMaps
/// 4. Batch-query bibitem_authors and bibitem_keywords junctions
/// 5. Assemble rows using lookups
async fn export_bibitems_expanded_format(
    bibitems: &[BibItem],
    state: &AppState,
) -> Result<Response, HexforgeError> {
    if bibitems.is_empty() {
        let mut wtr = csv::Writer::from_writer(vec![]);
        wtr.write_record(EXPANDED_FORMAT_HEADER)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
        let data = wtr
            .into_inner()
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
        return Ok(csv_response(data, "bibitems.csv"));
    }

    let bibitem_ids: Vec<i64> = bibitems.iter().map(|b| b.id).collect();

    // 2. Collect all unique FK IDs
    let mut journal_ids = HashSet::new();
    let mut publisher_ids = HashSet::new();
    let mut institution_ids = HashSet::new();
    let mut school_ids = HashSet::new();
    let mut series_ids = HashSet::new();
    let mut crossref_ids = HashSet::new();

    for bib in bibitems {
        if let Some(id) = bib.journal_id {
            journal_ids.insert(id);
        }
        if let Some(id) = bib.publisher_id {
            publisher_ids.insert(id);
        }
        if let Some(id) = bib.institution_id {
            institution_ids.insert(id);
        }
        if let Some(id) = bib.school_id {
            school_ids.insert(id);
        }
        if let Some(id) = bib.series_id {
            series_ids.insert(id);
        }
        if let Some(id) = bib.crossref_id {
            crossref_ids.insert(id);
        }
    }

    // 3. Batch-fetch all related entities into HashMaps
    let journals_map = batch_fetch_map(&state.journal_ds, &journal_ids).await?;
    let publishers_map = batch_fetch_map(&state.publisher_ds, &publisher_ids).await?;
    let institutions_map = batch_fetch_map(&state.institution_ds, &institution_ids).await?;
    let schools_map = batch_fetch_map(&state.school_ds, &school_ids).await?;
    let series_map = batch_fetch_map(&state.series_ds, &series_ids).await?;
    let crossrefs_map = batch_fetch_map(&state.bibitem_ds, &crossref_ids).await?;

    // 4. Batch-query junction tables
    let author_rows = fetch_bibitem_authors_batch(state, &bibitem_ids).await?;
    let keyword_rows = fetch_bibitem_keywords_batch(state, &bibitem_ids).await?;

    // Build author lookup: author_id -> Author
    let all_author_ids: HashSet<i64> = author_rows.iter().map(|r| r.author_id).collect();
    let authors_map = batch_fetch_map(&state.author_ds, &all_author_ids).await?;

    // Build keyword lookup: keyword_id -> Keyword
    let all_keyword_ids: HashSet<i64> = keyword_rows.iter().map(|r| r.keyword_id).collect();
    let keywords_map = batch_fetch_map(&state.keyword_ds, &all_keyword_ids).await?;

    // Group junction data by bibitem_id
    let mut authors_by_bibitem: HashMap<i64, Vec<&BibitemAuthorRow>> = HashMap::new();
    for row in &author_rows {
        authors_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    let mut keywords_by_bibitem: HashMap<i64, Vec<&BibitemKeywordRow>> = HashMap::new();
    for row in &keyword_rows {
        keywords_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    // 5. Assemble CSV rows
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(EXPANDED_FORMAT_HEADER)
        .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for bib in bibitems {
        let bib_authors = authors_by_bibitem.get(&bib.id);

        let author_col = format_role_names(bib_authors, "author", &authors_map);
        let editor_col = format_role_names(bib_authors, "editor", &authors_map);
        let guesteditor_col = format_role_names(bib_authors, "guesteditor", &authors_map);

        let journal_name = bib
            .journal_id
            .and_then(|id| journals_map.get(&id))
            .map(|j| j.name_simplified.as_str())
            .unwrap_or("");

        let publisher_name = bib
            .publisher_id
            .and_then(|id| publishers_map.get(&id))
            .map(|p| p.name_simplified.as_str())
            .unwrap_or("");

        let institution_name = bib
            .institution_id
            .and_then(|id| institutions_map.get(&id))
            .map(|i| i.name_simplified.as_str())
            .unwrap_or("");

        let school_name = bib
            .school_id
            .and_then(|id| schools_map.get(&id))
            .map(|s| s.name_simplified.as_str())
            .unwrap_or("");

        let series_name = bib
            .series_id
            .and_then(|id| series_map.get(&id))
            .map(|s| s.name_simplified.as_str())
            .unwrap_or("");

        let crossref_bibkey = bib
            .crossref_id
            .and_then(|id| crossrefs_map.get(&id))
            .map(|b| b.bibkey.as_str())
            .unwrap_or("");

        // Keywords by level
        let bib_keywords = keywords_by_bibitem.get(&bib.id);
        let kw_level1 = format_keywords_at_level(bib_keywords, 1, &keywords_map);
        let kw_level2 = format_keywords_at_level(bib_keywords, 2, &keywords_map);
        let kw_level3 = format_keywords_at_level(bib_keywords, 3, &keywords_map);

        wtr.write_record([
            &bib.entry_type.to_string(),
            &bib.bibkey,
            &author_col,
            &editor_col,
            &guesteditor_col,
            opt_str(&bib.options),
            opt_str(&bib.shorthand),
            &opt_i16(bib.date_year),
            &opt_display(&bib.pubstate),
            &bib.title_latex,
            &bib.title_unicode,
            &bib.title_simplified,
            opt_str(&bib.booktitle_latex),
            opt_str(&bib.booktitle_unicode),
            opt_str(&bib.booktitle_simplified),
            crossref_bibkey,
            journal_name,
            opt_str(&bib.volume),
            opt_str(&bib.number),
            opt_str(&bib.pages),
            opt_str(&bib.eid),
            series_name,
            opt_str(&bib.address),
            institution_name,
            school_name,
            publisher_name,
            opt_str(&bib.type_field),
            opt_str(&bib.edition),
            opt_str(&bib.note_latex),
            opt_str(&bib.note_unicode),
            opt_str(&bib.issuetitle_latex),
            opt_str(&bib.issuetitle_unicode),
            opt_str(&bib.extra_note_latex),
            opt_str(&bib.extra_note_unicode),
            opt_str(&bib.urn),
            opt_str(&bib.eprint),
            opt_str(&bib.doi),
            opt_str(&bib.url),
            &kw_level1,
            &kw_level2,
            &kw_level3,
            &opt_display(&bib.epoch),
            &opt_display(&bib.langid),
            &bib.is_translation.to_string(),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    Ok(csv_response(data, "bibitems.csv"))
}

/// Format author/editor/guesteditor names for a given role.
///
/// Returns "LastName, FirstName and LastName2, FirstName2" using simplified names,
/// ordered by position.
fn format_role_names(
    bib_authors: Option<&Vec<&BibitemAuthorRow>>,
    role: &str,
    authors_map: &HashMap<i64, Author>,
) -> String {
    bib_authors
        .map(|rows| {
            let mut filtered: Vec<&BibitemAuthorRow> =
                rows.iter().filter(|r| r.role == role).copied().collect();
            filtered.sort_by_key(|r| r.position);

            let names: Vec<String> = filtered
                .iter()
                .filter_map(|r| {
                    authors_map.get(&r.author_id).map(|a| {
                        if let Some(ref mononym) = a.mononym_simplified {
                            mononym.clone()
                        } else {
                            let family = a.family_name_simplified.as_deref().unwrap_or("");
                            let given = a.given_name_simplified.as_deref().unwrap_or("");
                            if given.is_empty() {
                                family.to_string()
                            } else {
                                format!("{family}, {given}")
                            }
                        }
                    })
                })
                .collect();

            names.join(" and ")
        })
        .unwrap_or_default()
}

/// Format keyword names at a given level, semicolon-separated.
fn format_keywords_at_level(
    bib_keywords: Option<&Vec<&BibitemKeywordRow>>,
    level: i16,
    keywords_map: &HashMap<i64, Keyword>,
) -> String {
    bib_keywords
        .map(|rows| {
            let names: Vec<&str> = rows
                .iter()
                .filter(|r| r.keyword_level == level)
                .filter_map(|r| keywords_map.get(&r.keyword_id).map(|k| k.name.as_str()))
                .collect();
            names.join(";")
        })
        .unwrap_or_default()
}

// =============================================================================
// Generic entity fetch helpers
// =============================================================================

/// Trait to extract entity id and key for generic entity operations.
trait EntityWithKey {
    fn entity_id(&self) -> i64;
    fn key_value(&self) -> &str;
}

impl EntityWithKey for Author {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.author_key
    }
}

impl EntityWithKey for Journal {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.journal_key
    }
}

impl EntityWithKey for Publisher {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.publisher_key
    }
}

impl EntityWithKey for Institution {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.institution_key
    }
}

impl EntityWithKey for School {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.school_key
    }
}

impl EntityWithKey for Series {
    fn entity_id(&self) -> i64 {
        self.id
    }
    fn key_value(&self) -> &str {
        &self.series_key
    }
}

/// Trait to extract the id for batch fetching into a HashMap.
trait EntityWithId {
    fn entity_id(&self) -> i64;
}

impl EntityWithId for Author {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Journal {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Publisher {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Institution {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for School {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Series {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for Keyword {
    fn entity_id(&self) -> i64 {
        self.id
    }
}
impl EntityWithId for BibItem {
    fn entity_id(&self) -> i64 {
        self.id
    }
}

/// Fetch entities by request criteria: all, ids, or keys.
///
/// Returns `Ok(Ok(entities))` on success, `Ok(Err(response))` for not-found/bad-request
/// error responses that should be returned directly, or `Err(HexforgeError)` for
/// internal errors.
async fn fetch_entities_by_request<T, Q>(
    ds: &hexforge::DataStore<T, Q>,
    key_column: &str,
    all: bool,
    ids: Option<Vec<i64>>,
    keys: Option<Vec<String>>,
) -> Result<Result<Vec<T>, Response>, HexforgeError>
where
    T: hexforge::PgEntity + Clone + EntityWithKey + Send + Unpin,
    Q: hexforge::PgQuery + 'static,
{
    if all {
        let all_entities = ds
            .find_many(WhereClause::new("1=1"))
            .await
            .map_err(HexforgeError::data_source)?;
        return Ok(Ok(all_entities));
    }

    if let Some(ref id_list) = ids {
        let found = ds
            .find_by_ids(id_list)
            .await
            .map_err(HexforgeError::data_source)?;
        let found_ids: HashSet<i64> = found.iter().map(|e| e.entity_id()).collect();
        let missing: Vec<i64> = id_list
            .iter()
            .filter(|id| !found_ids.contains(id))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Ok(Err(not_found_ids_response(missing)));
        }
        return Ok(Ok(found));
    }

    if let Some(ref key_list) = keys {
        let mut all_found = Vec::new();
        for key in key_list {
            let clause = WhereClause::new(format!("{key_column} = $1")).bind(key.clone());
            let found = ds
                .find_one(clause)
                .await
                .map_err(HexforgeError::data_source)?;
            if let Some(entity) = found {
                all_found.push(entity);
            }
        }
        let found_keys: HashSet<&str> = all_found.iter().map(|e| e.key_value()).collect();
        let missing: Vec<String> = key_list
            .iter()
            .filter(|k| !found_keys.contains(k.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Ok(Err(not_found_keys_response(missing)));
        }
        return Ok(Ok(all_found));
    }

    Ok(Err(bad_request_response()))
}

/// Batch-fetch entities into a HashMap keyed by id.
async fn batch_fetch_map<T, Q>(
    ds: &hexforge::DataStore<T, Q>,
    ids: &HashSet<i64>,
) -> Result<HashMap<i64, T>, HexforgeError>
where
    T: hexforge::PgEntity + Clone + EntityWithId + Send + Unpin,
    Q: hexforge::PgQuery + 'static,
{
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let id_vec: Vec<i64> = ids.iter().copied().collect();
    let entities = ds
        .find_by_ids(&id_vec)
        .await
        .map_err(HexforgeError::data_source)?;
    Ok(entities.into_iter().map(|e| (e.entity_id(), e)).collect())
}

/// Fetch all rows from a table using raw SQL (for "all" mode).
async fn fetch_all_via_sql<T>(
    pool: &hexforge::db_exports::PgPool,
    table: &str,
) -> Result<Vec<T>, HexforgeError>
where
    T: for<'r> sqlx::FromRow<'r, hexforge::db_exports::PgRow> + Send + Unpin,
{
    let sql = format!("SELECT * FROM {table} ORDER BY id");
    query_as::<_, T>(&sql)
        .fetch_all(pool)
        .await
        .map_err(HexforgeError::data_source)
}

// =============================================================================
// Batch junction queries
// =============================================================================

/// Batch-fetch all bibitem_authors rows for the given bibitem IDs.
async fn fetch_bibitem_authors_batch(
    state: &AppState,
    bibitem_ids: &[i64],
) -> Result<Vec<BibitemAuthorRow>, HexforgeError> {
    if bibitem_ids.is_empty() {
        return Ok(vec![]);
    }
    query_as::<_, BibitemAuthorRow>(
        r#"
        SELECT bibitem_id, author_id, role::text as role, position
        FROM bibitem_authors
        WHERE bibitem_id = ANY($1)
        ORDER BY bibitem_id, role, position
        "#,
    )
    .bind(bibitem_ids)
    .fetch_all(state.pool.pool())
    .await
    .map_err(HexforgeError::data_source)
}

/// Batch-fetch all bibitem_keywords rows for the given bibitem IDs.
async fn fetch_bibitem_keywords_batch(
    state: &AppState,
    bibitem_ids: &[i64],
) -> Result<Vec<BibitemKeywordRow>, HexforgeError> {
    if bibitem_ids.is_empty() {
        return Ok(vec![]);
    }
    query_as::<_, BibitemKeywordRow>(
        r#"
        SELECT bibitem_id, keyword_id, keyword_level
        FROM bibitem_keywords
        WHERE bibitem_id = ANY($1)
        ORDER BY bibitem_id, keyword_level
        "#,
    )
    .bind(bibitem_ids)
    .fetch_all(state.pool.pool())
    .await
    .map_err(HexforgeError::data_source)
}
