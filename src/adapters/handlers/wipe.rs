use hexforge::axum_exports::{Json, Query, State};
use hexforge::{HexforgeError, ValidationError};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Deserialize)]
pub struct WipeParams {
    confirm: Option<bool>,
}

#[derive(Serialize)]
pub struct WipeResponse {
    wiped: bool,
}

pub async fn wipe_data(
    State(state): State<AppState>,
    Query(params): Query<WipeParams>,
) -> Result<Json<WipeResponse>, HexforgeError> {
    if params.confirm != Some(true) {
        return Err(HexforgeError::Validation(ValidationError::custom(
            "This operation truncates all data tables. Pass ?confirm=true to proceed.",
        )));
    }
    state.wipe().await?;
    Ok(Json(WipeResponse { wiped: true }))
}
