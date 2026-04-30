//! Render handler — thin HTTP wrapper for bibliography rendering.
//!
//! `POST /api/v1/render`
//!
//! Accepts a JSON request body with either `ids` or `bibkeys` to select bibitems.
//! Returns rendered HTML bibliography sorted by author, year, bibkey.

use hexforge::axum_exports::{IntoResponse, Json, Response, State, StatusCode};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::adapters::render::{
    PgBibitemResolver, PgRenderAuthorFetcher, PgRenderEntityFetcher, PgTransitiveDepsResolver,
};
use crate::process::render::{RenderPipelineError, RenderSelection, render_pipeline};

// =============================================================================
// Request / response types
// =============================================================================

/// Request body for the render endpoint.
#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    /// Select bibitems by ID.
    pub ids: Option<Vec<i64>>,
    /// Select bibitems by bibkey.
    pub bibkeys: Option<Vec<String>>,
    /// When true, include a further-references section rendered from transitive deps.
    #[serde(default)]
    pub include_further_refs: bool,
}

/// Response body for the render endpoint.
#[derive(Debug, Serialize)]
pub struct RenderResponseBody {
    pub main_html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub further_refs_html: Option<String>,
}

/// Error response body.
#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_bibkeys: Option<Vec<String>>,
}

// =============================================================================
// Handler
// =============================================================================

/// Render bibliography as HTML.
pub async fn render_bibitems(
    State(state): State<AppState>,
    Json(req): Json<RenderRequest>,
) -> Response {
    // Map HTTP request to process-layer selection
    let selection = if let Some(ids) = req.ids {
        RenderSelection::ByIds(ids)
    } else if let Some(bibkeys) = req.bibkeys {
        RenderSelection::ByBibkeys(bibkeys)
    } else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "Request must specify \"ids\" or \"bibkeys\"".to_string(),
            None,
            None,
        );
    };

    let pool = state.pool.pool();
    let resolver = PgBibitemResolver::new(&state.bibitem_ds);
    let entity_fetcher = PgRenderEntityFetcher::new(pool);
    let author_fetcher = PgRenderAuthorFetcher::new(pool, &state.author_ds);
    let deps_resolver = PgTransitiveDepsResolver::new(pool);

    match render_pipeline(
        &resolver,
        &entity_fetcher,
        &author_fetcher,
        &deps_resolver,
        selection,
        req.include_further_refs,
    )
    .await
    {
        Ok(resp) => (
            StatusCode::OK,
            Json(RenderResponseBody {
                main_html: resp.main_html,
                further_refs_html: resp.further_refs_html,
            }),
        )
            .into_response(),
        Err(e) => pipeline_error_to_response(e),
    }
}

fn pipeline_error_to_response(err: RenderPipelineError) -> Response {
    match err {
        RenderPipelineError::TooManyItems { requested, max } => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "too_many_items",
            format!("Requested {requested} items, maximum is {max}"),
            None,
            None,
        ),
        RenderPipelineError::MissingIds(missing) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_found",
            format!("{} requested ID(s) not found", missing.len()),
            Some(missing),
            None,
        ),
        RenderPipelineError::MissingBibkeys(missing) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_found",
            format!("{} requested bibkey(s) not found", missing.len()),
            None,
            Some(missing),
        ),
        RenderPipelineError::Internal(e) => e.into_response(),
    }
}

fn error_response(
    status: StatusCode,
    error: &'static str,
    message: String,
    missing_ids: Option<Vec<i64>>,
    missing_bibkeys: Option<Vec<String>>,
) -> Response {
    (
        status,
        Json(ErrorBody {
            error,
            message,
            missing_ids,
            missing_bibkeys,
        }),
    )
        .into_response()
}
