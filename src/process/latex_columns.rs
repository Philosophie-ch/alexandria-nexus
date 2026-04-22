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
        updates: &[(i64, String)],
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
    let mut per_batch_texts: Vec<Vec<String>> = Vec::with_capacity(batches.len());
    for batch in &batches {
        let n = batch.rows.len();
        per_batch_texts.push(pre_compiled[offset..offset + n].to_vec());
        offset += n;
    }

    // Convert via subprocess
    let all_outcomes = convert_batches(converter, per_batch_texts).await?;

    // Process outcomes into per-column updates and errors (no I/O).
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

    // Write columns sequentially to avoid row-lock deadlocks when multiple columns
    // belong to the same table (e.g. the 5 bibitems unicode columns).
    let mut write_counts: Vec<usize> = Vec::with_capacity(batches.len());
    for (batch, updates) in batches.iter().zip(per_batch_updates.iter()) {
        let n = writer
            .write_unicode_column(batch.table, batch.column, updates)
            .await?;
        write_counts.push(n);
    }

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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use hexforge::HexforgeError;

    use super::*;
    use crate::logic::latex_citations::CitationData;
    use crate::process::latex_citations::CitationResolver;

    // ── Mock implementations ─────────────────────────────────────────────────

    struct MockFetcher(Vec<ColumnBatch>);

    impl LatexColumnFetcher for MockFetcher {
        async fn fetch_all_latex_columns(&self) -> Result<Vec<ColumnBatch>, HexforgeError> {
            Ok(self
                .0
                .iter()
                .map(|b| ColumnBatch {
                    table: b.table,
                    column: b.column,
                    rows: b.rows.clone(),
                })
                .collect())
        }
    }

    /// Identity converter: returns each input text unchanged as ConvertOutcome::Ok.
    struct IdentityConverter;

    impl LatexBatchConverter for IdentityConverter {
        async fn convert(&self, texts: Vec<String>) -> Result<Vec<ConvertOutcome>, HexforgeError> {
            Ok(texts.into_iter().map(ConvertOutcome::Ok).collect())
        }
    }

    struct MockResolver(HashMap<String, CitationData>);

    impl CitationResolver for MockResolver {
        async fn resolve_bibkeys(
            &self,
            keys: &[String],
        ) -> Result<HashMap<String, CitationData>, HexforgeError> {
            Ok(keys
                .iter()
                .filter_map(|k| {
                    self.0.get(k).map(|d| {
                        (
                            k.clone(),
                            CitationData {
                                author: d.author.clone(),
                                year: d.year,
                            },
                        )
                    })
                })
                .collect())
        }
    }

    type WriteCall = (String, String, Vec<(i64, String)>);

    /// Records every write call in order so tests can assert on the sequence.
    #[derive(Clone, Default)]
    struct RecordingWriter {
        calls: Arc<Mutex<Vec<WriteCall>>>,
    }

    impl LatexColumnWriter for RecordingWriter {
        async fn write_unicode_column(
            &self,
            table: &'static str,
            column: &'static str,
            updates: &[(i64, String)],
        ) -> Result<usize, HexforgeError> {
            self.calls.lock().unwrap().push((
                table.to_string(),
                column.to_string(),
                updates.to_vec(),
            ));
            Ok(updates.len())
        }
    }

    fn cd(author: &str, year: i16) -> CitationData {
        CitationData {
            author: Some(author.to_string()),
            year: Some(year),
        }
    }

    fn s(v: &str) -> String {
        v.to_string()
    }

    // ── Tests ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn clean_text_goes_to_ok_updates() {
        let fetcher = MockFetcher(vec![ColumnBatch {
            table: "journals",
            column: "name_unicode",
            rows: vec![(1, s("Journal of Philosophy"))],
        }]);
        let writer = RecordingWriter::default();
        let resolver = MockResolver(HashMap::new());

        convert_all_columns(&fetcher, &IdentityConverter, &resolver, &writer)
            .await
            .unwrap();

        let calls = writer.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        let (_, _, updates) = &calls[0];
        assert_eq!(updates, &[(1i64, s("Journal of Philosophy"))]);
    }

    #[tokio::test]
    async fn cite_command_substituted_and_stored() {
        let mut db = HashMap::new();
        db.insert(s("smith:2000"), cd("Smith", 2000));
        let fetcher = MockFetcher(vec![ColumnBatch {
            table: "bibitems",
            column: "title_unicode",
            rows: vec![
                (10, s(r"Review of \citet{smith:2000}")),
                (11, s("Plain title")),
            ],
        }]);
        let writer = RecordingWriter::default();

        convert_all_columns(&fetcher, &IdentityConverter, &MockResolver(db), &writer)
            .await
            .unwrap();

        let calls = writer.calls.lock().unwrap();
        let (_, _, updates) = &calls[0];
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0], (10i64, s("Review of Smith (2000)")));
        assert_eq!(updates[1], (11i64, s("Plain title")));
    }

    #[tokio::test]
    async fn missing_cite_key_renders_partial_text_and_reported() {
        let fetcher = MockFetcher(vec![ColumnBatch {
            table: "bibitems",
            column: "title_unicode",
            rows: vec![(5, s(r"See \citet{ghost:1999}."))],
        }]);
        let writer = RecordingWriter::default();
        let resolver = MockResolver(HashMap::new());

        let report = convert_all_columns(&fetcher, &IdentityConverter, &resolver, &writer)
            .await
            .unwrap();

        let calls = writer.calls.lock().unwrap();
        let (_, _, updates) = &calls[0];
        assert_eq!(updates[0], (5i64, s("See .")));
        assert!(report.missing_citation_keys.contains(&s("ghost:1999")));
    }

    #[tokio::test]
    async fn converter_error_is_collected_not_written() {
        struct ErrorConverter;
        impl LatexBatchConverter for ErrorConverter {
            async fn convert(
                &self,
                texts: Vec<String>,
            ) -> Result<Vec<ConvertOutcome>, HexforgeError> {
                Ok(texts
                    .into_iter()
                    .map(|t| ConvertOutcome::Err {
                        original: t,
                        message: s("bad latex"),
                    })
                    .collect())
            }
        }

        let fetcher = MockFetcher(vec![ColumnBatch {
            table: "journals",
            column: "name_unicode",
            rows: vec![(99, s("broken{latex"))],
        }]);
        let writer = RecordingWriter::default();

        let report = convert_all_columns(
            &fetcher,
            &ErrorConverter,
            &MockResolver(HashMap::new()),
            &writer,
        )
        .await
        .unwrap();

        let calls = writer.calls.lock().unwrap();
        let (_, _, updates) = &calls[0];
        assert!(updates.is_empty());
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].id, 99);
        assert_eq!(report.errors[0].error, "bad latex");
    }

    #[tokio::test]
    async fn multi_column_rows_sliced_correctly() {
        let fetcher = MockFetcher(vec![
            ColumnBatch {
                table: "authors",
                column: "family_name_unicode",
                rows: vec![(1, s("Müller")), (2, s("García"))],
            },
            ColumnBatch {
                table: "authors",
                column: "given_name_unicode",
                rows: vec![(1, s("Hans")), (2, s("Luis"))],
            },
        ]);
        let writer = RecordingWriter::default();

        convert_all_columns(
            &fetcher,
            &IdentityConverter,
            &MockResolver(HashMap::new()),
            &writer,
        )
        .await
        .unwrap();

        let calls = writer.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);

        let (_, col0, updates0) = &calls[0];
        assert_eq!(col0, "family_name_unicode");
        assert_eq!(updates0, &[(1i64, s("Müller")), (2i64, s("García"))]);

        let (_, col1, updates1) = &calls[1];
        assert_eq!(col1, "given_name_unicode");
        assert_eq!(updates1, &[(1i64, s("Hans")), (2i64, s("Luis"))]);
    }

    #[tokio::test]
    async fn writes_are_sequential_in_batch_order() {
        let fetcher = MockFetcher(vec![
            ColumnBatch {
                table: "t",
                column: "col_a",
                rows: vec![(1, s("a"))],
            },
            ColumnBatch {
                table: "t",
                column: "col_b",
                rows: vec![(2, s("b"))],
            },
            ColumnBatch {
                table: "t",
                column: "col_c",
                rows: vec![(3, s("c"))],
            },
        ]);
        let writer = RecordingWriter::default();

        convert_all_columns(
            &fetcher,
            &IdentityConverter,
            &MockResolver(HashMap::new()),
            &writer,
        )
        .await
        .unwrap();

        let calls = writer.calls.lock().unwrap();
        let columns: Vec<&str> = calls.iter().map(|(_, c, _)| c.as_str()).collect();
        assert_eq!(columns, ["col_a", "col_b", "col_c"]);
    }

    #[tokio::test]
    async fn total_updated_counts_all_ok_rows() {
        let mut db = HashMap::new();
        db.insert(s("k:1"), cd("K", 2001));
        let fetcher = MockFetcher(vec![ColumnBatch {
            table: "bibitems",
            column: "title_unicode",
            rows: vec![(1, s(r"\citet{k:1}")), (2, s("Clean title"))],
        }]);
        let writer = RecordingWriter::default();

        let report = convert_all_columns(&fetcher, &IdentityConverter, &MockResolver(db), &writer)
            .await
            .unwrap();

        assert_eq!(report.total_updated, 2);
        assert_eq!(report.columns[0].updated, 2);
    }

    #[tokio::test]
    async fn empty_batches_produce_empty_report() {
        let fetcher = MockFetcher(vec![]);
        let writer = RecordingWriter::default();

        let report = convert_all_columns(
            &fetcher,
            &IdentityConverter,
            &MockResolver(HashMap::new()),
            &writer,
        )
        .await
        .unwrap();

        assert!(writer.calls.lock().unwrap().is_empty());
        assert_eq!(report.total_updated, 0);
        assert!(report.errors.is_empty());
        assert!(report.missing_citation_keys.is_empty());
    }
}
