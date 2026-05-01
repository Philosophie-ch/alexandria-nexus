//! LaTeX → Unicode conversion handler.
//!
//! `POST /api/v1/admin/latex-to-unicode`
//!
//! Requires `python3` in PATH with `pylatexenc` installed.
//!
//! Error handling:
//! - python3 missing / pylatexenc not installed / timeout  → HTTP 500
//! - Individual item LaTeX parse error                     → item with status "error"

use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;
use crate::adapters::latex_to_unicode::PyConvertItem;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LatexConvertRequest {
    pub texts: Vec<String>,
}

/// One item in the batch result.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum LatexConvertItem {
    /// Conversion succeeded.
    Ok { result: String },
    /// Conversion failed for this item (e.g. invalid LaTeX syntax).
    Error { message: String },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LatexConvertResponse {
    pub results: Vec<LatexConvertItem>,
}

/// Convert a batch of LaTeX strings to their Unicode equivalents.
///
/// Results are in the same order as the input. Each item independently succeeds
/// or fails — one bad LaTeX string never blocks the rest.
///
/// `POST /api/v1/admin/latex-to-unicode`
pub async fn convert_latex_to_unicode(
    State(state): State<AppState>,
    Json(req): Json<LatexConvertRequest>,
) -> Result<Json<LatexConvertResponse>, HexforgeError> {
    let py_items = state.latex_converter().convert_batch(&req.texts).await?;

    let results = py_items
        .into_iter()
        .map(|item| match item {
            PyConvertItem::Ok { result } => LatexConvertItem::Ok { result },
            PyConvertItem::Error { message } => LatexConvertItem::Error { message },
        })
        .collect();

    Ok(Json(LatexConvertResponse { results }))
}
