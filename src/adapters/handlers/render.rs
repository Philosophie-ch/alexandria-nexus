//! Render handler: thin HTTP wrapper for bibliography rendering.
//!
//! `POST /api/v1/render`
//!
//! Accepts a JSON request body with either `ids` or `bibkeys` to select bibitems.
//! Returns rendered HTML bibliography sorted by author, year, bibkey.

use hexforge::axum_exports::{IntoResponse, Json, Response, State, StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;
use crate::process::render::{MissingItems, RenderPipelineError, RenderSelection, render_pipeline};

// =============================================================================
// Request / response types
// =============================================================================

/// Request body for the render endpoint.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RenderRequest {
    /// Select bibitems by ID.
    pub ids: Option<Vec<i64>>,
    /// Select bibitems by bibkey.
    pub bibkeys: Option<Vec<String>>,
    /// When true, fetch transitive deps (junction-table further refs) into `further_refs_html`.
    /// Inline citations (`\cite` in titles/notes) are always resolved and may independently
    /// populate `further_refs_html` regardless of this flag.
    #[serde(default)]
    pub include_further_refs: bool,
}

/// Response body for the render endpoint.
#[derive(Debug, Serialize, ToSchema)]
pub struct RenderResponseBody {
    pub main_html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub further_refs_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_bibkeys: Option<Vec<String>>,
}

/// Error response body.
#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
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
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: "bad_request",
                message: "Request must specify \"ids\" or \"bibkeys\"".to_string(),
            }),
        )
            .into_response();
    };

    let resolver = state.bibitem_resolver();
    let entity_fetcher = state.render_entity_fetcher();
    let author_fetcher = state.render_author_fetcher();
    let deps_resolver = state.transitive_deps_resolver();

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
        Ok(resp) => {
            let (missing_ids, missing_bibkeys) = match resp.missing {
                MissingItems::Ids(ids) => (Some(ids), None),
                MissingItems::Bibkeys(keys) => (None, Some(keys)),
            };
            (
                StatusCode::OK,
                Json(RenderResponseBody {
                    main_html: resp.main_html,
                    further_refs_html: resp.further_refs_html,
                    missing_ids,
                    missing_bibkeys,
                }),
            )
                .into_response()
        }
        Err(e) => pipeline_error_to_response(e),
    }
}

fn pipeline_error_to_response(err: RenderPipelineError) -> Response {
    match err {
        RenderPipelineError::TooManyItems { requested, max } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorBody {
                error: "too_many_items",
                message: format!("Requested {requested} items, maximum is {max}"),
            }),
        )
            .into_response(),
        RenderPipelineError::Internal(e) => e.into_response(),
    }
}
