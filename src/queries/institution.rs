//! Query filter for institutions.

use hexforge::Filter;
use serde::Deserialize;

/// Query parameters for filtering institutions.
///
/// - `name` — case-insensitive substring match on `name_simplified`
#[derive(Filter, Debug, Default, Deserialize)]
pub struct InstitutionQuery {
    #[query(like = "name_simplified")]
    pub name: Option<String>,
}
