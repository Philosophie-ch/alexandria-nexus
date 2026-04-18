//! Bulk LaTeX → Unicode column converter.
//!
//! Knows which tables and columns to convert; delegates the actual string
//! conversion to the process layer via `LatexBatchConverter`.

use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgPool, query, query_as};

use crate::logic::full_import::{
    ColumnConvertResult, ConvertOutcome, LatexConvertError, LatexConvertReport,
};
use crate::process::latex_columns::{LatexBatchConverter, convert_batches};

/// `(table, latex_col, unicode_col)` — all 15 pairs to process.
const COLUMN_SPECS: &[(&str, &str, &str)] = &[
    ("bibitems", "title_latex", "title_unicode"),
    ("bibitems", "booktitle_latex", "booktitle_unicode"),
    ("bibitems", "note_latex", "note_unicode"),
    ("bibitems", "issuetitle_latex", "issuetitle_unicode"),
    ("bibitems", "extra_note_latex", "extra_note_unicode"),
    ("authors", "family_name_latex", "family_name_unicode"),
    ("authors", "given_name_latex", "given_name_unicode"),
    ("authors", "mononym_latex", "mononym_unicode"),
    ("authors", "famous_name_latex", "famous_name_unicode"),
    ("authors", "shorthand_latex", "shorthand_unicode"),
    ("journals", "name_latex", "name_unicode"),
    ("publishers", "name_latex", "name_unicode"),
    ("institutions", "name_latex", "name_unicode"),
    ("schools", "name_latex", "name_unicode"),
    ("series", "name_latex", "name_unicode"),
];

#[derive(Debug, FromRow)]
struct LatexRow {
    id: i64,
    latex: String,
}

pub struct PgLatexColumnConverter<'a> {
    pool: &'a PgPool,
}

impl<'a> PgLatexColumnConverter<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Fetch all non-null latex values for one column.
    async fn fetch_column(
        &self,
        table: &'static str,
        latex_col: &'static str,
    ) -> Result<Vec<LatexRow>, HexforgeError> {
        let sql =
            format!("SELECT id, {latex_col} AS latex FROM {table} WHERE {latex_col} IS NOT NULL");
        query_as::<_, LatexRow>(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)
    }

    /// Write converted unicode values back to one column.
    async fn update_column(
        &self,
        table: &'static str,
        unicode_col: &'static str,
        updates: &[(i64, String)],
    ) -> Result<(), HexforgeError> {
        for (id, value) in updates {
            let sql = format!("UPDATE {table} SET {unicode_col} = $1 WHERE id = $2");
            query(&sql)
                .bind(value)
                .bind(id)
                .execute(self.pool)
                .await
                .map_err(HexforgeError::data_source)?;
        }
        Ok(())
    }

    /// Convert all 15 columns in order and return a full report.
    pub async fn convert_all_columns(
        &self,
        converter: &impl LatexBatchConverter,
    ) -> Result<LatexConvertReport, HexforgeError> {
        // 1. Fetch all columns' data
        let mut all_ids: Vec<Vec<i64>> = Vec::with_capacity(COLUMN_SPECS.len());
        let mut all_texts: Vec<Vec<String>> = Vec::with_capacity(COLUMN_SPECS.len());

        for &(table, latex_col, _unicode_col) in COLUMN_SPECS {
            let rows = self.fetch_column(table, latex_col).await?;
            let (ids, texts): (Vec<i64>, Vec<String>) =
                rows.into_iter().map(|r| (r.id, r.latex)).unzip();
            all_ids.push(ids);
            all_texts.push(texts);
        }

        // 2. Convert all batches via process layer
        let all_outcomes = convert_batches(converter, all_texts).await?;

        // 3. Write back results, build report
        let mut column_results = Vec::with_capacity(COLUMN_SPECS.len());
        let mut errors: Vec<LatexConvertError> = Vec::new();
        let mut total_updated = 0usize;

        for ((&(table, _latex_col, unicode_col), ids), outcomes) in
            COLUMN_SPECS.iter().zip(all_ids.iter()).zip(all_outcomes)
        {
            let mut ok_updates: Vec<(i64, String)> = Vec::new();

            for (&id, outcome) in ids.iter().zip(outcomes) {
                match outcome {
                    ConvertOutcome::Ok(result) => ok_updates.push((id, result)),
                    ConvertOutcome::Err {
                        original: _,
                        message,
                    } => {
                        errors.push(LatexConvertError {
                            table,
                            column: unicode_col,
                            id,
                            error: message,
                        });
                    }
                }
            }

            let updated = ok_updates.len();
            self.update_column(table, unicode_col, &ok_updates).await?;
            total_updated += updated;
            column_results.push(ColumnConvertResult {
                table,
                column: unicode_col,
                updated,
            });
        }

        Ok(LatexConvertReport {
            columns: column_results,
            total_updated,
            errors,
        })
    }
}
