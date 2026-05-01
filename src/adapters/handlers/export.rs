//! Export handlers — thin HTTP adapters for CSV export endpoints.
//!
//! `POST /api/v1/admin/export/{entity}`
//!
//! Requires Admin permission.

use hexforge::HexforgeError;
use hexforge::axum_exports::{IntoResponse, Json, Response, State, StatusCode, header};
use serde::Serialize;

use crate::AppState;
use crate::adapters::csv_rows::{
    bibitems_to_rows, build_author_rows, build_institution_rows, build_journal_rows,
    build_keyword_rows, build_publisher_rows, build_school_rows, build_series_rows,
};
use crate::logic::export::{BibitemExportRequest, EntityExportRequest};
use crate::process::export;
use crate::process::export::ExportError;

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

/// Serialize CSV rows to bytes using csv::Writer.
fn rows_to_csv_bytes(rows: Vec<Vec<String>>) -> Result<Vec<u8>, HexforgeError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    for row in &rows {
        wtr.write_record(row)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }
    wtr.into_inner()
        .map_err(|e| HexforgeError::internal(e.to_string()))
}

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

/// Convert a process result (domain entities) to an HTTP CSV response via a row builder.
fn csv_result_to_response<T>(
    result: Result<Vec<T>, ExportError>,
    build_rows: impl FnOnce(&[T]) -> Vec<Vec<String>>,
    filename: &str,
) -> Result<Response, HexforgeError> {
    match result {
        Ok(entities) => {
            let bytes = rows_to_csv_bytes(build_rows(&entities))?;
            Ok(csv_response(bytes, filename))
        }
        Err(err) => export_error_to_response(err),
    }
}

/// Convert a bibitem export result to an HTTP CSV response.
fn bibitem_csv_response(
    result: Result<crate::process::export::BibitemExportData, ExportError>,
    filename: &str,
) -> Result<Response, HexforgeError> {
    match result {
        Ok(data) => {
            let bytes = rows_to_csv_bytes(bibitems_to_rows(data))?;
            Ok(csv_response(bytes, filename))
        }
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
    let fetcher = state.author_export_fetcher();
    let result = export::export_authors(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, build_author_rows, "authors.csv")
}

/// Export journals as CSV.
///
/// `POST /api/v1/admin/export/journals`
pub async fn export_journals(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let fetcher = state.journal_export_fetcher();
    let result = export::export_journals(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, build_journal_rows, "journals.csv")
}

/// Export publishers as CSV.
///
/// `POST /api/v1/admin/export/publishers`
pub async fn export_publishers(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let fetcher = state.publisher_export_fetcher();
    let result = export::export_publishers(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, build_publisher_rows, "publishers.csv")
}

/// Export institutions as CSV.
///
/// `POST /api/v1/admin/export/institutions`
pub async fn export_institutions(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let fetcher = state.institution_export_fetcher();
    let result = export::export_institutions(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, build_institution_rows, "institutions.csv")
}

/// Export schools as CSV.
///
/// `POST /api/v1/admin/export/schools`
pub async fn export_schools(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let fetcher = state.school_export_fetcher();
    let result = export::export_schools(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, build_school_rows, "schools.csv")
}

/// Export series as CSV.
///
/// `POST /api/v1/admin/export/series`
pub async fn export_series(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let fetcher = state.series_export_fetcher();
    let result = export::export_series(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, build_series_rows, "series.csv")
}

/// Export keywords as CSV.
///
/// `POST /api/v1/admin/export/keywords`
pub async fn export_keywords(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let fetcher = state.keyword_export_fetcher();
    let result = export::export_keywords(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, build_keyword_rows, "keywords.csv")
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
    let bibitem_fetcher = state.bibitem_export_fetcher();
    let junction_fetcher = state.export_junction_fetcher();
    let author_batch = state.author_batch_export_fetcher();
    let journal_batch = state.journal_batch_export_fetcher();
    let publisher_batch = state.publisher_batch_export_fetcher();
    let institution_batch = state.institution_batch_export_fetcher();
    let school_batch = state.school_batch_export_fetcher();
    let series_batch = state.series_batch_export_fetcher();
    let bibitem_batch = state.bibitem_batch_export_fetcher();
    let keyword_batch = state.keyword_batch_export_fetcher();

    let result = export::export_bibitems(
        &bibitem_fetcher,
        &junction_fetcher,
        &author_batch,
        &journal_batch,
        &publisher_batch,
        &institution_batch,
        &school_batch,
        &series_batch,
        &bibitem_batch,
        &keyword_batch,
        req,
    )
    .await;
    bibitem_csv_response(result, "bibitems.csv")
}
