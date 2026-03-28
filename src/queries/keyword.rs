//! Query filter for keywords.

use hexforge::Filter;
use serde::Deserialize;

/// Query parameters for filtering keywords.
///
/// - `level` — exact match on `level` (1-3)
/// - `name` — case-insensitive substring match on `name`
#[derive(Filter, Debug, Default, Deserialize)]
pub struct KeywordQuery {
    #[query(eq = "level")]
    pub level: Option<i16>,
    #[query(like = "name")]
    pub name: Option<String>,
}
