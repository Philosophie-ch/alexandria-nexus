//! Full CSV import handlers -- thin HTTP adapters for human-readable CSV endpoints.
//!
//! Constructs Postgres adapters and calls process-layer functions.

use hexforge::HexforgeError;
use hexforge::axum_exports::{IntoResponse, Json, Multipart, Response, State, StatusCode};

use serde::Deserialize;

use super::import::extract_csv_bytes;

#[derive(Deserialize)]
pub struct ImportFullCsvParams {
    #[serde(default)]
    pub delete_stale: bool,
}

use crate::adapters::full_import::PgFullImportStore;
use crate::logic::full_import::{
    EntityImportReport, FullImportResult, ValidationReport, parse_all_rows,
};
use crate::process::full_import;
use crate::state::AppState;

const FULL_CSV_HEADERS: &str = "entry_type,bibkey,author,editor,_guesteditor,date,pubstate,title,\
booktitle,journal,publisher,institution,school,series,volume,number,pages,eid,address,type,edition,\
note,_issuetitle,_extra_note,crossref,\
_kw_level1,_kw_level2,_kw_level3,_epoch,_langid,_lang_der,_person,\
_has_link_to_full_text,shorthand,options,doi,url,eprint,urn,\
_note-perso,_note-stock,_note-missing,_change-request,_dltc_copyediting_note,_to-do-general";

/// Validate a human-readable CSV without importing.
/// `POST /api/v1/admin/validate-full-csv`
pub async fn validate_full_csv(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ValidationReport>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let (rows, row_errors) = parse_all_rows(&data)?;
    let store = PgFullImportStore::new(state.pool.pool());
    let report =
        full_import::validate_import(&store, &store, &store, &store, &rows, row_errors).await?;
    Ok(Json(report))
}

/// Import missing entities from a human-readable CSV.
/// `POST /api/v1/admin/import-entities-from-full-csv`
pub async fn import_entities_from_full_csv(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<EntityImportReport>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let (rows, _) = parse_all_rows(&data)?;
    let store = PgFullImportStore::new(state.pool.pool());
    let report = full_import::import_entities(
        &store,
        &store,
        &store,
        &store,
        &state.institution_ds,
        &state.school_ds,
        &state.series_ds,
        &state.keyword_ds,
        &rows,
    )
    .await?;
    Ok(Json(report))
}

/// Import bibitems from a human-readable CSV.
/// `POST /api/v1/admin/import-full-csv?delete_stale=true`
///
/// By default, only upserts (insert new + update existing). Stale bibitems
/// (in DB but not in CSV) are left untouched unless `?delete_stale=true`.
pub async fn import_full_csv(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<ImportFullCsvParams>,
    multipart: Multipart,
) -> Result<Response, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let (rows, row_errors) = parse_all_rows(&data)?;
    let store = PgFullImportStore::new(state.pool.pool());
    let result = full_import::import_bibitems(
        &store,
        &store,
        &store,
        &store,
        &store,
        &store,
        &store,
        &store,
        &store,
        rows,
        row_errors,
        params.delete_stale,
    )
    .await?;
    match result {
        FullImportResult::Success(report) => Ok(Json(report).into_response()),
        FullImportResult::ValidationFailed(report) => {
            Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(report)).into_response())
        }
    }
}

/// Recompute transitive further-ref dependencies from `bibitem_refs`.
/// `POST /api/v1/admin/recompute-deps`
///
/// Truncates `bibitem_further_refs` and repopulates it via recursive CTE.
/// Use after manual edits to `bibitem_refs` without doing a full import.
pub async fn recompute_deps(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, HexforgeError> {
    let store = PgFullImportStore::new(state.pool.pool());
    let (further_refs, depends_on) = full_import::recompute_transitive_deps(&store).await?;
    Ok(Json(
        serde_json::json!({ "further_refs": further_refs, "depends_on": depends_on }),
    ))
}

/// Export all bibitems as a human-readable CSV matching the full import format.
/// `POST /api/v1/admin/export-full-csv`
pub async fn export_full_csv(State(state): State<AppState>) -> Result<Response, HexforgeError> {
    let store = PgFullImportStore::new(state.pool.pool());
    let rows =
        full_import::fetch_export_rows(&store, &store, &store, &store, &store, &store).await?;

    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record(FULL_CSV_HEADERS.split(','))
        .map_err(|e| HexforgeError::internal(format!("CSV header error: {e}")))?;
    for row in rows {
        wtr.write_record(&row)
            .map_err(|e| HexforgeError::internal(format!("CSV write error: {e}")))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| HexforgeError::internal(format!("CSV flush error: {e}")))?;
    let csv = String::from_utf8(bytes)
        .map_err(|e| HexforgeError::internal(format!("CSV UTF-8 error: {e}")))?;

    Ok((
        StatusCode::OK,
        [
            ("content-type", "text/csv; charset=utf-8"),
            (
                "content-disposition",
                "attachment; filename=\"bibliography-export.csv\"",
            ),
        ],
        csv,
    )
        .into_response())
}
