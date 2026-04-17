//! Render handler — thin HTTP wrapper for bibliography rendering.
//!
//! `POST /api/v1/render`
//!
//! Accepts a JSON request body with either `ids` or `bibkeys` to select bibitems.
//! Returns rendered HTML bibliography sorted by author, year, bibkey.
//! Capped at 1000 items per request.

use hexforge::HexforgeError;
use hexforge::axum_exports::{IntoResponse, Json, Response, State, StatusCode, header};
use serde::{Deserialize, Serialize};

use crate::adapters::render::{PgBibitemResolver, PgRenderAuthorFetcher, PgRenderNameFetcher};
use crate::process::render::{
    ResolveResult, render_bibitems_to_html, resolve_by_bibkeys, resolve_by_ids,
};
use crate::state::AppState;

// =============================================================================
// Request / response types
// =============================================================================

/// Maximum number of items per render request.
const MAX_RENDER_ITEMS: usize = 1000;

/// Request body for the render endpoint.
#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    /// Select bibitems by ID.
    pub ids: Option<Vec<i64>>,
    /// Select bibitems by bibkey.
    pub bibkeys: Option<Vec<String>>,
}

/// 422 error response for missing or too-many items.
#[derive(Debug, Serialize)]
struct RenderError {
    error: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_bibkeys: Option<Vec<String>>,
}

/// 400 bad request error.
#[derive(Debug, Serialize)]
struct BadRequestError {
    error: &'static str,
    message: &'static str,
}

// =============================================================================
// Handler
// =============================================================================

/// Render bibliography as HTML.
///
/// `POST /api/v1/render`
///
/// Returns `Content-Type: text/html` with the rendered bibliography.
pub async fn render_bibitems(
    State(state): State<AppState>,
    Json(req): Json<RenderRequest>,
) -> Result<Response, HexforgeError> {
    // Validate request
    let requested_count = req
        .ids
        .as_ref()
        .map(Vec::len)
        .or_else(|| req.bibkeys.as_ref().map(Vec::len));

    match requested_count {
        None => {
            return Ok((
                StatusCode::BAD_REQUEST,
                Json(BadRequestError {
                    error: "bad_request",
                    message: "Request must specify \"ids\" or \"bibkeys\"",
                }),
            )
                .into_response());
        }
        Some(0) => return Ok(html_response(String::new())),
        Some(count) if count > MAX_RENDER_ITEMS => {
            return Ok((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(RenderError {
                    error: "too_many_items",
                    message: format!("Requested {count} items, maximum is {MAX_RENDER_ITEMS}"),
                    missing_ids: None,
                    missing_bibkeys: None,
                }),
            )
                .into_response());
        }
        _ => {}
    }

    // Resolve bibitems
    let resolver = PgBibitemResolver::new(&state);
    let bibitems = if let Some(ref id_list) = req.ids {
        match resolve_by_ids(&resolver, id_list).await? {
            ResolveResult::Ok(items) => items,
            ResolveResult::MissingIds(missing) => {
                return Ok((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(RenderError {
                        error: "not_found",
                        message: format!("{} requested ID(s) not found", missing.len()),
                        missing_ids: Some(missing),
                        missing_bibkeys: None,
                    }),
                )
                    .into_response());
            }
            ResolveResult::MissingBibkeys(_) => unreachable!(),
        }
    } else if let Some(ref bibkey_list) = req.bibkeys {
        match resolve_by_bibkeys(&resolver, bibkey_list).await? {
            ResolveResult::Ok(items) => items,
            ResolveResult::MissingBibkeys(missing) => {
                return Ok((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(RenderError {
                        error: "not_found",
                        message: format!("{} requested bibkey(s) not found", missing.len()),
                        missing_ids: None,
                        missing_bibkeys: Some(missing),
                    }),
                )
                    .into_response());
            }
            ResolveResult::MissingIds(_) => unreachable!(),
        }
    } else {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(BadRequestError {
                error: "bad_request",
                message: "Request must specify \"ids\" or \"bibkeys\"",
            }),
        )
            .into_response());
    };

    // Render
    let name_fetcher = PgRenderNameFetcher::new(state.pool.pool());
    let author_fetcher = PgRenderAuthorFetcher::new(&state);
    let html = render_bibitems_to_html(&name_fetcher, &author_fetcher, bibitems).await?;

    Ok(html_response(html))
}

// =============================================================================
// Helpers
// =============================================================================

/// Build an HTML response.
fn html_response(body: String) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}
