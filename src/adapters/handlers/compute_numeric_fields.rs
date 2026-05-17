use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};

use crate::AppState;
use crate::process::compute_numeric_fields::{ComputeNumericFieldsReport, compute_numeric_fields};

pub async fn compute_numeric_fields_handler(
    State(state): State<AppState>,
) -> Result<Json<ComputeNumericFieldsReport>, HexforgeError> {
    let fetcher = state.numeric_field_fetcher();
    let writer = state.numeric_field_writer();
    let report = compute_numeric_fields(&fetcher, &writer).await?;
    Ok(Json(report))
}
