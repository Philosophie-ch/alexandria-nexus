//! Query filter for journals.

use hexforge::Filter;
use serde::Deserialize;

/// Query parameters for filtering journals.
///
/// - `name` — case-insensitive substring match on `name_simplified`
#[derive(Filter, Debug, Default, Deserialize)]
pub struct JournalQuery {
    #[query(like = "name_simplified")]
    pub name: Option<String>,
}
