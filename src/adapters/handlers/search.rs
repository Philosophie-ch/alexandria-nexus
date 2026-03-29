//! Search handler — thin HTTP adapter for bibitem search.
//!
//! `POST /api/v1/search`

use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};

use crate::logic::search::{self, SearchRequest, SearchResponse};
use crate::state::AppState;

/// Search bibitems with full-text search and filters.
///
/// `POST /api/v1/search`
///
/// Parses the JSON request, delegates to the search logic, and returns the
/// JSON response.
pub async fn search_bibitems(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, HexforgeError> {
    let response = search::perform_search(&state, request).await?;
    Ok(Json(response))
}
