//! Bulk LaTeX → Unicode column conversion handler.
//!
//! `POST /api/v1/admin/convert-latex-columns`

use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};

use crate::AppState;
use crate::adapters::latex_citations::PgCitationResolver;
use crate::adapters::latex_columns::PgLatexColumnConverter;
use crate::adapters::latex_to_unicode::PyLatexConverter;
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
    let pool = state.pool.pool();
    let pg = PgLatexColumnConverter::new(pool);
    let citation_resolver = PgCitationResolver::new(pool);
    let report = convert_all_columns(&pg, &PyLatexConverter, &citation_resolver, &pg).await?;
    Ok(Json(report))
}
