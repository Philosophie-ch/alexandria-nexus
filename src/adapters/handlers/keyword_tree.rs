//! Keyword tree handler — thin HTTP adapter.

use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};
use serde::Serialize;

use crate::AppState;
use crate::adapters::keyword_tree::PgKeywordFetcher;
use crate::domain::Keyword;
use crate::process::keyword_tree::build_keyword_tree;

/// JSON-serializable keyword tree response.
#[derive(Debug, Serialize)]
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
    let fetcher = PgKeywordFetcher::new(state.pool.pool());
    let tree = build_keyword_tree(&fetcher).await?;

    Ok(Json(KeywordTreeResponse {
        level_1: tree.level_1,
        level_2: tree.level_2,
        level_3: tree.level_3,
    }))
}
