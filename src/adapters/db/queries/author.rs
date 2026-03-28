//! Query filter for authors.

use hexforge::Filter;
use serde::Deserialize;

/// Query parameters for filtering authors.
///
/// - `family_name` — case-insensitive substring match on `family_name_simplified`
/// - `search_term` — case-insensitive substring match on `family_name_simplified`
///   OR `given_name_simplified`
#[derive(Filter, Debug, Default, Deserialize)]
pub struct AuthorQuery {
    #[query(like = "family_name_simplified")]
    pub family_name: Option<String>,
    #[query(like_any = "family_name_simplified,given_name_simplified")]
    pub search_term: Option<String>,
}
