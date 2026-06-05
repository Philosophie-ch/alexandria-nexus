//! Full CSV import handlers -- thin HTTP adapters for human-readable CSV endpoints.
//!
//! Constructs Postgres adapters and calls process-layer functions.

use hexforge::HexforgeError;
use hexforge::axum_exports::{IntoResponse, Json, Multipart, Query, Response, State, StatusCode};

use serde::Deserialize;

use super::import::extract_csv_bytes;
use crate::AppState;
use crate::adapters::csv_rows::full_csv_to_bytes;
use crate::adapters::field_parsing::parse_all_rows;
use crate::logic::full_import::{EntityImportReport, FullImportResult, ValidationReport};
use crate::process::full_import;

#[derive(Deserialize)]
pub struct ImportFullCsvParams {
    #[serde(default)]
    pub delete_stale: bool,
}

/// Validate a human-readable CSV without importing.
/// `POST /api/v1/admin/validate-full-csv?delete_stale=true`
///
/// When `delete_stale=true`, crossrefs are checked against only the file's bibkeys
/// (stale DB bibkeys would be removed). Default: checks against DB + file bibkeys.
pub async fn validate_full_csv(
    State(state): State<AppState>,
    Query(params): Query<ImportFullCsvParams>,
    multipart: Multipart,
) -> Result<Json<ValidationReport>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let (rows, row_errors) = parse_all_rows(&data)?;
    let store = state.full_import_store();
    let report = full_import::validate_import(
        &store,
        &store,
        &store,
        &store,
        &rows,
        row_errors,
        params.delete_stale,
    )
    .await?;
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
    let store = state.full_import_store();
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
    Query(params): Query<ImportFullCsvParams>,
    multipart: Multipart,
) -> Result<Response, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let (rows, row_errors) = parse_all_rows(&data)?;
    let store = state.full_import_store();
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
    let store = state.full_import_store();
    let (further_refs, depends_on) = full_import::recompute_transitive_deps(&store).await?;
    Ok(Json(
        serde_json::json!({ "further_refs": further_refs, "depends_on": depends_on }),
    ))
}

/// Export all bibitems as a human-readable CSV matching the full import format.
/// `POST /api/v1/admin/export-full-csv`
pub async fn export_full_csv(State(state): State<AppState>) -> Result<Response, HexforgeError> {
    let store = state.full_import_store();
    let data =
        full_import::fetch_export_data(&store, &store, &store, &store, &store, &store).await?;
    let bytes = full_csv_to_bytes(&data)?;
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
