//! LaTeX → Unicode batch conversion — process layer.
//!
//! Defines the `LatexBatchConverter` trait and a simple orchestration function
//! that converts multiple independent batches of strings. No column or table
//! knowledge lives here — that belongs in the adapter layer.

use std::future::Future;

use hexforge::HexforgeError;

use crate::logic::full_import::ConvertOutcome;

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

/// Convert multiple independent batches in order.
///
/// Makes one `converter.convert()` call per batch and collects the results.
/// Returns one `Vec<ConvertOutcome>` per input batch in the same order.
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
