//! Export handlers — thin HTTP adapters for CSV export endpoints.
//!
//! `POST /api/v1/admin/export/{entity}`
//!
//! Requires Admin permission.

use hexforge::HexforgeError;
use hexforge::axum_exports::{IntoResponse, Json, Response, State, StatusCode, header};
use serde::Serialize;

use crate::adapters::export::{
    PgBibitemFetcher, PgEntityBatchFetcher, PgExportJunctionFetcher, PgKeyedEntityFetcher,
    PgKeywordFetcher,
};
use crate::logic::export::{BibitemExportRequest, EntityExportRequest, ExportError};
use crate::process::export;
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

/// Convert a process result (rows) to an HTTP CSV response.
fn csv_result_to_response(
    result: Result<Vec<Vec<String>>, ExportError>,
    filename: &str,
) -> Result<Response, HexforgeError> {
    match result {
        Ok(rows) => {
            let bytes = rows_to_csv_bytes(rows)?;
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
    let pool = state.pool.pool();
    let fetcher = PgKeyedEntityFetcher::new(&state.author_ds, "author_key", "authors", pool);
    let result = export::export_authors(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "authors.csv")
}

/// Export journals as CSV.
///
/// `POST /api/v1/admin/export/journals`
pub async fn export_journals(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let pool = state.pool.pool();
    let fetcher = PgKeyedEntityFetcher::new(&state.journal_ds, "journal_key", "journals", pool);
    let result = export::export_journals(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "journals.csv")
}

/// Export publishers as CSV.
///
/// `POST /api/v1/admin/export/publishers`
pub async fn export_publishers(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let pool = state.pool.pool();
    let fetcher =
        PgKeyedEntityFetcher::new(&state.publisher_ds, "publisher_key", "publishers", pool);
    let result = export::export_publishers(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "publishers.csv")
}

/// Export institutions as CSV.
///
/// `POST /api/v1/admin/export/institutions`
pub async fn export_institutions(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let pool = state.pool.pool();
    let fetcher = PgKeyedEntityFetcher::new(
        &state.institution_ds,
        "institution_key",
        "institutions",
        pool,
    );
    let result = export::export_institutions(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "institutions.csv")
}

/// Export schools as CSV.
///
/// `POST /api/v1/admin/export/schools`
pub async fn export_schools(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let pool = state.pool.pool();
    let fetcher = PgKeyedEntityFetcher::new(&state.school_ds, "school_key", "schools", pool);
    let result = export::export_schools(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "schools.csv")
}

/// Export series as CSV.
///
/// `POST /api/v1/admin/export/series`
pub async fn export_series(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let pool = state.pool.pool();
    let fetcher = PgKeyedEntityFetcher::new(&state.series_ds, "series_key", "series", pool);
    let result = export::export_series(&fetcher, req.all, req.ids, req.keys).await;
    csv_result_to_response(result, "series.csv")
}

/// Export keywords as CSV.
///
/// `POST /api/v1/admin/export/keywords`
pub async fn export_keywords(
    State(state): State<AppState>,
    Json(req): Json<EntityExportRequest>,
) -> Result<Response, HexforgeError> {
    let pool = state.pool.pool();
    let fetcher = PgKeywordFetcher::new(&state.keyword_ds, pool);
    let result = export::export_keywords(&fetcher, req.all, req.ids, req.keys).await;
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
    let pool = state.pool.pool();
    let bibitem_fetcher = PgBibitemFetcher::new(&state.bibitem_ds, pool);
    let junction_fetcher = PgExportJunctionFetcher::new(pool);
    let author_batch = PgEntityBatchFetcher::new(&state.author_ds);
    let journal_batch = PgEntityBatchFetcher::new(&state.journal_ds);
    let publisher_batch = PgEntityBatchFetcher::new(&state.publisher_ds);
    let institution_batch = PgEntityBatchFetcher::new(&state.institution_ds);
    let school_batch = PgEntityBatchFetcher::new(&state.school_ds);
    let series_batch = PgEntityBatchFetcher::new(&state.series_ds);
    let bibitem_batch = PgEntityBatchFetcher::new(&state.bibitem_ds);
    let keyword_batch = PgEntityBatchFetcher::new(&state.keyword_ds);

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
    csv_result_to_response(result, "bibitems.csv")
}
