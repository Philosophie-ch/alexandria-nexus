//! Full CSV import handlers — thin HTTP adapters for human-readable CSV endpoints.

use axum::extract::Multipart;
use hexforge::HexforgeError;
use hexforge::axum_exports::{IntoResponse, Json, Response, State, StatusCode};

use super::import::extract_csv_bytes;
use crate::logic::full_import::{self, EntityImportReport, FullImportResult, ValidationReport};
use crate::state::AppState;

/// Validate a human-readable CSV without importing.
/// `POST /api/v1/admin/validate-full-csv`
pub async fn validate_full_csv(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ValidationReport>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let report = full_import::validate_full_csv(&state, &data).await?;
    Ok(Json(report))
}

/// Import missing entities from a human-readable CSV.
/// `POST /api/v1/admin/import-entities-from-full-csv`
pub async fn import_entities_from_full_csv(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<EntityImportReport>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let report = full_import::import_entities_from_full_csv(&state, &data).await?;
    Ok(Json(report))
}

/// Import bibitems from a human-readable CSV (source of truth).
/// `POST /api/v1/admin/import-full-csv`
pub async fn import_full_csv(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Response, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let result = full_import::import_full_csv(&state, &data).await?;
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
    let csv = full_import::export_full_csv(&state).await?;
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
