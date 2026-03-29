//! Export handlers — thin HTTP adapters for CSV export endpoints.
//!
//! `POST /api/v1/admin/export/{entity}`
//!
//! Requires Admin permission.

use hexforge::HexforgeError;
use hexforge::axum_exports::{IntoResponse, Json, Response, State, StatusCode, header};
use serde::Serialize;

use crate::logic::export::{self, BibitemExportRequest, EntityExportRequest, ExportError};
use crate::state::AppState;

// =============================================================================
// HTTP error response types
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
// Error conversion: ExportError -> HTTP Response
// =============================================================================

fn export_error_to_response(err: ExportError) -> Result<Response, HexforgeError> {
    match err {
        ExportError::MissingIds(missing) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(NotFoundError {
                error: "not_found",
                message: format!("{} requested ID(s) not found", missing.len()),
                missing_ids: Some(missing),
                missing_keys: None,
                missing_bibkeys: None,
            }),
        )
            .into_response()),
        ExportError::MissingKeys(missing) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(NotFoundError {
                error: "not_found",
                message: format!("{} requested key(s) not found", missing.len()),
                missing_ids: None,
                missing_keys: Some(missing),
                missing_bibkeys: None,
            }),
        )
            .into_response()),
        ExportError::MissingBibkeys(missing) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(NotFoundError {
                error: "not_found",
                message: format!("{} requested bibkey(s) not found", missing.len()),
                missing_ids: None,
                missing_keys: None,
                missing_bibkeys: Some(missing),
            }),
        )
            .into_response()),
        ExportError::BadRequest => Ok((
            StatusCode::BAD_REQUEST,
            Json(BadRequestError {
                error: "bad_request",
                message: "Request must specify \"all\": true, \"ids\", or \"keys\"/\"bibkeys\"",
            }),
        )
            .into_response()),
        ExportError::Internal(e) => Err(e),
    }
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

/// Convert a logic result to an HTTP response.
fn csv_result_to_response(
    result: Result<Vec<u8>, ExportError>,
    filename: &str,
) -> Result<Response, HexforgeError> {
    match result {
        Ok(data) => Ok(csv_response(data, filename)),
        Err(err) => export_error_to_response(err),
    }
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
    let result = export::export_authors_csv(&state, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "authors.csv")
}

/// Export journals as CSV.
///
/// `POST /api/v1/admin/export/journals`
pub async fn export_journals(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let result = export::export_journals_csv(&state, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "journals.csv")
}

/// Export publishers as CSV.
///
/// `POST /api/v1/admin/export/publishers`
pub async fn export_publishers(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let result = export::export_publishers_csv(&state, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "publishers.csv")
}

/// Export institutions as CSV.
///
/// `POST /api/v1/admin/export/institutions`
pub async fn export_institutions(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let result = export::export_institutions_csv(&state, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "institutions.csv")
}

/// Export schools as CSV.
///
/// `POST /api/v1/admin/export/schools`
pub async fn export_schools(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let result = export::export_schools_csv(&state, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "schools.csv")
}

/// Export series as CSV.
///
/// `POST /api/v1/admin/export/series`
pub async fn export_series(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let result = export::export_series_csv(&state, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "series.csv")
}

/// Export keywords as CSV.
///
/// `POST /api/v1/admin/export/keywords`
pub async fn export_keywords(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let result = export::export_keywords_csv(&state, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "keywords.csv")
}

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
    let result = export::export_bibitems_csv(&state, req).await;
    csv_result_to_response(result, "bibitems.csv")
}
