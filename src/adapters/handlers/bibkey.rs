//! Handler for fetching bibitems by bibkey.

use hexforge::axum_exports::{Json, Path, State};
use hexforge::{HexforgeError, WhereClause};

use crate::domain::BibItem;
use crate::state::AppState;

/// Get a bibitem by its bibkey.
///
/// `GET /api/v1/bibitems/by-bibkey/{bibkey}`
pub async fn get_by_bibkey(
    State(state): State<AppState>,
    Path(bibkey): Path<String>,
) -> Result<Json<BibItem>, HexforgeError> {
    let result = state
        .bibitem_ds
        .find_one(WhereClause::new("bibkey = $1").bind(bibkey))
        .await
        .map_err(HexforgeError::data_source)?;

    result.map(Json).ok_or(HexforgeError::NotFound)
}
