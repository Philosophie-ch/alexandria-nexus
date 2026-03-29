//! Keyword tree handler — thin HTTP adapter.

use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};
use serde::Serialize;

use crate::logic::keyword_tree;
use crate::state::AppState;

/// JSON-serializable keyword tree response.
#[derive(Debug, Serialize)]
pub struct KeywordTreeResponse {
    pub level_1: Vec<crate::domain::Keyword>,
    pub level_2: Vec<crate::domain::Keyword>,
    pub level_3: Vec<crate::domain::Keyword>,
}

/// Get the hierarchical keyword tree.
///
/// `GET /api/v1/keywords/tree`
///
/// Returns all keywords organized by level (1, 2, 3).
pub async fn get_keyword_tree(
    State(state): State<AppState>,
) -> Result<Json<KeywordTreeResponse>, HexforgeError> {
    let tree = keyword_tree::build_keyword_tree(&state).await?;

    Ok(Json(KeywordTreeResponse {
        level_1: tree.level_1,
        level_2: tree.level_2,
        level_3: tree.level_3,
    }))
}
