//! Keyword tree logic — groups keywords by level into a hierarchical tree.
//!
//! Pure application service: takes a data store, returns structured data.
//! No HTTP types.

use hexforge::{HexforgeError, WhereClause};

use crate::domain::Keyword;
use crate::state::AppState;

/// Keyword tree organized by level.
#[derive(Debug)]
pub struct KeywordTree {
    pub level_1: Vec<Keyword>,
    pub level_2: Vec<Keyword>,
    pub level_3: Vec<Keyword>,
}

/// Fetch all keywords and organize them by level.
pub async fn build_keyword_tree(state: &AppState) -> Result<KeywordTree, HexforgeError> {
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

    Ok(KeywordTree {
        level_1,
        level_2,
        level_3,
    })
}
