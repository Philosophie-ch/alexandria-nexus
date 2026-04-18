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
use crate::logic::full_import::{EntityImportReport, FullImportResult, ValidationReport};
use crate::process::full_import;
use crate::state::AppState;

/// Validate a human-readable CSV without importing.
/// `POST /api/v1/admin/validate-full-csv`
pub async fn validate_full_csv(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ValidationReport>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let store = PgFullImportStore::new(state.pool.pool());
    let report = full_import::validate_full_csv(&store, &store, &store, &store, &data).await?;
    Ok(Json(report))
}

/// Import missing entities from a human-readable CSV.
/// `POST /api/v1/admin/import-entities-from-full-csv`
pub async fn import_entities_from_full_csv(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<EntityImportReport>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let store = PgFullImportStore::new(state.pool.pool());
    let report = full_import::import_entities_from_full_csv(
        &store,
        &store,
        &store,
        &store,
        &state.institution_ds,
        &state.school_ds,
        &state.series_ds,
        &state.keyword_ds,
        &data,
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
    let store = PgFullImportStore::new(state.pool.pool());
    let result = full_import::import_full_csv(
        &store,
        &store,
        &store,
        &store,
        &store,
        &store,
        &store,
        &data,
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

/// Export all bibitems as a human-readable CSV matching the full import format.
/// `POST /api/v1/admin/export-full-csv`
pub async fn export_full_csv(State(state): State<AppState>) -> Result<Response, HexforgeError> {
    let store = PgFullImportStore::new(state.pool.pool());
    let csv = full_import::export_full_csv(&store, &store, &store, &store, &store).await?;
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
