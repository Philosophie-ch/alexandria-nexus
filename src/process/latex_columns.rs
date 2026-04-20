//! LaTeX → Unicode batch conversion — process layer.
//!
//! Defines the `LatexBatchConverter` trait, column fetcher/writer traits, and the
//! `convert_all_columns` orchestration function.  No SQL, no HTTP — only abstract contracts.

use std::future::Future;

use hexforge::HexforgeError;

use crate::logic::full_import::{
    ColumnConvertResult, ConvertOutcome, LatexConvertError, LatexConvertReport,
};
use crate::process::latex_citations::{CitationResolver, pre_compile_citations};

/// Contract for converting a batch of LaTeX strings to Unicode.
///
/// Per-item failures must be encoded as `ConvertOutcome::Err` (never abort
/// the whole batch). Subprocess-level failures propagate as `Err`.
pub trait LatexBatchConverter: Send + Sync {
    fn convert(
        &self,
        texts: Vec<String>,
    ) -> impl Future<Output = Result<Vec<ConvertOutcome>, HexforgeError>> + Send;
}

/// One column's worth of (id, latex_text) rows, identified by table + unicode column name.
pub struct ColumnBatch {
    pub table: &'static str,
    /// The unicode target column (used for reporting and writing).
    pub column: &'static str,
    pub rows: Vec<(i64, String)>,
}

/// Contract for fetching all latex column batches across all entity tables.
pub trait LatexColumnFetcher: Send + Sync {
    fn fetch_all_latex_columns(
        &self,
    ) -> impl Future<Output = Result<Vec<ColumnBatch>, HexforgeError>> + Send;
}

/// Contract for writing converted unicode values back to a column.
pub trait LatexColumnWriter: Send + Sync {
    fn write_unicode_column(
        &self,
        table: &'static str,
        column: &'static str,
        rows: &[(i64, String)],
    ) -> impl Future<Output = Result<usize, HexforgeError>> + Send;
}

/// Convert multiple independent batches in order.
pub async fn convert_batches(
    converter: &impl LatexBatchConverter,
    batches: Vec<Vec<String>>,
) -> Result<Vec<Vec<ConvertOutcome>>, HexforgeError> {
    let mut results = Vec::with_capacity(batches.len());
    for batch in batches {
        results.push(converter.convert(batch).await?);
    }
    Ok(results)
}

/// Convert all latex columns to unicode across all entity tables.
///
/// Pipeline per column:
/// 1. Fetch (id, latex) rows
/// 2. Pre-compile `\cite*{...}` commands into plain text (one batch DB call for all columns)
/// 3. Convert via `pylatexenc` subprocess
/// 4. Write back unicode values
/// 5. Accumulate stats + errors into `LatexConvertReport`
pub async fn convert_all_columns(
    fetcher: &impl LatexColumnFetcher,
    converter: &impl LatexBatchConverter,
    citation_resolver: &impl CitationResolver,
    writer: &impl LatexColumnWriter,
) -> Result<LatexConvertReport, HexforgeError> {
    let batches = fetcher.fetch_all_latex_columns().await?;

    // Flatten all texts from all columns for a single citation-resolve call
    let all_texts: Vec<String> = batches
        .iter()
        .flat_map(|b| b.rows.iter().map(|(_, t)| t.clone()))
        .collect();

    let (pre_compiled, missing_citation_keys) =
        pre_compile_citations(&all_texts, citation_resolver).await?;

    // Split pre-compiled texts back into per-column slices
    let mut offset = 0;
    let mut per_batch: Vec<Vec<String>> = Vec::with_capacity(batches.len());
    for batch in &batches {
        let n = batch.rows.len();
        per_batch.push(pre_compiled[offset..offset + n].to_vec());
        offset += n;
    }

    // Convert via subprocess
    let all_outcomes = convert_batches(converter, per_batch).await?;

    // Process outcomes into per-column updates and errors (no I/O)
    let mut per_batch_updates: Vec<Vec<(i64, String)>> = Vec::with_capacity(batches.len());
    let mut errors: Vec<LatexConvertError> = Vec::new();

    for (batch, outcomes) in batches.iter().zip(all_outcomes) {
        let mut ok_updates: Vec<(i64, String)> = Vec::new();
        for ((id, _), outcome) in batch.rows.iter().zip(outcomes) {
            match outcome {
                ConvertOutcome::Ok(unicode) => ok_updates.push((*id, unicode)),
                ConvertOutcome::Err { message, .. } => errors.push(LatexConvertError {
                    table: batch.table,
                    column: batch.column,
                    id: *id,
                    error: message,
                }),
            }
        }
        per_batch_updates.push(ok_updates);
    }

    // Write all columns concurrently
    let write_counts =
        futures::future::try_join_all(batches.iter().zip(per_batch_updates.iter()).map(
            |(batch, updates)| writer.write_unicode_column(batch.table, batch.column, updates),
        ))
        .await?;

    let total_updated: usize = write_counts.iter().sum();
    let column_results: Vec<ColumnConvertResult> = batches
        .iter()
        .zip(write_counts)
        .map(|(batch, updated)| ColumnConvertResult {
            table: batch.table,
            column: batch.column,
            updated,
        })
        .collect();

    Ok(LatexConvertReport {
        columns: column_results,
        total_updated,
        errors,
        missing_citation_keys,
    })
}
