//! Keyword tree handler for hierarchical keyword display.

use hexforge::axum_exports::{Json, State};
use hexforge::{HexforgeError, WhereClause};
use serde::Serialize;

use crate::domain::Keyword;
use crate::state::AppState;

/// Response containing keyword tree organized by level.
#[derive(Debug, Serialize)]
pub struct KeywordTree {
    pub level_1: Vec<Keyword>,
    pub level_2: Vec<Keyword>,
    pub level_3: Vec<Keyword>,
}

/// Get the hierarchical keyword tree.
///
/// `GET /api/v1/keywords/tree`
///
/// Returns all keywords organized by level (1, 2, 3).
pub async fn get_keyword_tree(
    State(state): State<AppState>,
) -> Result<Json<KeywordTree>, HexforgeError> {
    let level_1 = state
        .keyword_ds
        .find_many(WhereClause::new("level = $1").bind(1_i16))
        .await
        .map_err(HexforgeError::data_source)?;

    let level_2 = state
        .keyword_ds
        .find_many(WhereClause::new("level = $1").bind(2_i16))
        .await
        .map_err(HexforgeError::data_source)?;

    let level_3 = state
        .keyword_ds
        .find_many(WhereClause::new("level = $1").bind(3_i16))
        .await
        .map_err(HexforgeError::data_source)?;

    Ok(Json(KeywordTree {
        level_1,
        level_2,
        level_3,
    }))
}
