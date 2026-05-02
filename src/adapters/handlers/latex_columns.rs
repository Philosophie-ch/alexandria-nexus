//! Bulk LaTeX → Unicode column conversion handler.
//!
//! `POST /api/v1/admin/convert-latex-columns`

use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};

use crate::AppState;
use crate::logic::full_import::LatexConvertReport;
use crate::process::latex_columns::convert_all_columns;

/// Convert all `_latex` columns to their `_unicode` equivalents across all entity tables.
///
/// Processes 15 column pairs (bibitems × 5, authors × 5, journals/publishers/
/// institutions/schools/series × 1 each). Idempotent — safe to re-run.
///
/// `POST /api/v1/admin/convert-latex-columns`
pub async fn convert_latex_columns(
    State(state): State<AppState>,
) -> Result<Json<LatexConvertReport>, HexforgeError> {
    let fetcher = state.latex_column_fetcher();
    let writer = state.latex_column_writer();
    let latex_converter = state.latex_converter();
    let citation_resolver = state.citation_resolver();
    let report =
        convert_all_columns(&fetcher, &latex_converter, &citation_resolver, &writer).await?;
    Ok(Json(report))
}
