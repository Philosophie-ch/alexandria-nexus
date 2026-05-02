use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};

use crate::AppState;
use crate::process::compute_start_pages::{ComputeStartPagesReport, compute_start_pages};

/// Compute and persist `start_page` for every bibitem from its `pages` string.
///
/// Idempotent — safe to re-run. Use after migration to backfill existing data.
///
/// `POST /api/v1/admin/compute-start-pages`
pub async fn compute_start_pages_handler(
    State(state): State<AppState>,
) -> Result<Json<ComputeStartPagesReport>, HexforgeError> {
    let fetcher = state.start_page_fetcher();
    let writer = state.start_page_writer();
    let report = compute_start_pages(&fetcher, &writer).await?;
    Ok(Json(report))
}
