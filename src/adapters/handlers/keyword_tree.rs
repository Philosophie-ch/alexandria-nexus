//! Keyword tree handler — thin HTTP adapter.

use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::domain::Keyword;
use crate::process::keyword_tree::build_keyword_tree;

/// JSON-serializable keyword tree response.
#[derive(Debug, Serialize, ToSchema)]
pub struct KeywordTreeResponse {
    pub level_1: Vec<Keyword>,
    pub level_2: Vec<Keyword>,
    pub level_3: Vec<Keyword>,
}

/// Get the hierarchical keyword tree.
///
/// `GET /api/v1/keywords/tree`
pub async fn get_keyword_tree(
    State(state): State<AppState>,
) -> Result<Json<KeywordTreeResponse>, HexforgeError> {
    let fetcher = state.keyword_fetcher();
    let tree = build_keyword_tree(&fetcher).await?;

    Ok(Json(KeywordTreeResponse {
        level_1: tree.level_1,
        level_2: tree.level_2,
        level_3: tree.level_3,
    }))
}
