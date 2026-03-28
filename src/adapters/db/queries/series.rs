//! Query filter for series.

use hexforge::Filter;
use serde::Deserialize;

/// Query parameters for filtering series.
///
/// - `name` — case-insensitive substring match on `name_simplified`
#[derive(Filter, Debug, Default, Deserialize)]
pub struct SeriesQuery {
    #[query(like = "name_simplified")]
    pub name: Option<String>,
}
