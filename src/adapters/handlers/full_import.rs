//! Full CSV import handlers — thin HTTP adapters for human-readable CSV endpoints.

use axum::extract::Multipart;
use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};

use super::import::extract_csv_bytes;
use crate::logic::full_import::{self, ValidationReport};
use crate::state::AppState;

/// Validate a human-readable CSV without importing.
/// `POST /api/v1/admin/validate-full-csv`
pub async fn validate_full_csv(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ValidationReport>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let report = full_import::validate_full_csv(&state, data).await?;
    Ok(Json(report))
}
