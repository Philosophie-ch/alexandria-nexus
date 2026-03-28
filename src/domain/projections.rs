//! Projections — typed column subsets for list and search views.

use hexforge::Projection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{BibItem, EntryType, PubState};

/// Summary projection for bibliography list endpoints.
///
/// Selects only the essential columns needed for list views,
/// avoiding the full 46-column SELECT on every list request.
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "BibItem")]
pub struct BibItemSummary {
    pub id: i64,
    pub bibkey: String,
    pub entry_type: EntryType,
    pub title_simplified: String,
    pub date_year: Option<i16>,
    pub pubstate: Option<PubState>,
}
