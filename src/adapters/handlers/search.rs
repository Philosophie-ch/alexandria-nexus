//! Search handler — thin HTTP adapter for bibitem search.
//!
//! `POST /api/v1/search`

use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};

use crate::adapters::search::PgBibitemSearcher;
use crate::logic::search::{SearchRequest, SearchResponse};
use crate::process::search::perform_search;
use crate::state::AppState;

/// Search bibitems with full-text search and filters.
///
/// `POST /api/v1/search`
pub async fn search_bibitems(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, HexforgeError> {
    let searcher = PgBibitemSearcher::new(state.pool.pool());
    let response = perform_search(&searcher, request).await?;
    Ok(Json(response))
}
