use std::future::Future;

use hexforge::HexforgeError;
use serde::Serialize;

use crate::logic::pages::{compute_start_page, extract_leading_integer};

#[derive(Debug, Serialize)]
pub struct ComputeNumericFieldsReport {
    pub updated: usize,
}

pub struct BibitemTextFieldsRow {
    pub id: i64,
    pub pages: Option<String>,
    pub volume: Option<String>,
    pub number: Option<String>,
}

pub struct NumericFieldsUpdate {
    pub id: i64,
    pub start_page: Option<i32>,
    pub volume_numeric: Option<i32>,
    pub number_numeric: Option<i32>,
}

pub trait BibitemTextFieldsFetcher: Send + Sync {
    fn fetch_text_fields(
        &self,
    ) -> impl Future<Output = Result<Vec<BibitemTextFieldsRow>, HexforgeError>> + Send;
}

pub trait NumericFieldsWriter: Send + Sync {
    fn write_numeric_fields(
        &self,
        updates: &[NumericFieldsUpdate],
    ) -> impl Future<Output = Result<usize, HexforgeError>> + Send;
}

pub async fn compute_numeric_fields(
    fetcher: &impl BibitemTextFieldsFetcher,
    writer: &impl NumericFieldsWriter,
) -> Result<ComputeNumericFieldsReport, HexforgeError> {
    let rows = fetcher.fetch_text_fields().await?;
    let updates: Vec<NumericFieldsUpdate> = rows
        .into_iter()
        .map(|row| NumericFieldsUpdate {
            id: row.id,
            start_page: compute_start_page(row.pages.as_deref()),
            volume_numeric: extract_leading_integer(row.volume.as_deref()),
            number_numeric: extract_leading_integer(row.number.as_deref()),
        })
        .collect();
    let updated = writer.write_numeric_fields(&updates).await?;
    Ok(ComputeNumericFieldsReport { updated })
}

#[cfg(test)]
mod tests {
    use super::*;

    type TextFieldsInput<'a> = (i64, Option<&'a str>, Option<&'a str>, Option<&'a str>);
    type RecordedRow = (i64, Option<i32>, Option<i32>, Option<i32>);

    struct MockFetcher(Vec<BibitemTextFieldsRow>);

    impl MockFetcher {
        fn new(rows: Vec<TextFieldsInput<'_>>) -> Self {
            Self(
                rows.into_iter()
                    .map(|(id, pages, volume, number)| BibitemTextFieldsRow {
                        id,
                        pages: pages.map(str::to_owned),
                        volume: volume.map(str::to_owned),
                        number: number.map(str::to_owned),
                    })
                    .collect(),
            )
        }
    }

    impl BibitemTextFieldsFetcher for MockFetcher {
        async fn fetch_text_fields(&self) -> Result<Vec<BibitemTextFieldsRow>, HexforgeError> {
            Ok(self
                .0
                .iter()
                .map(|r| BibitemTextFieldsRow {
                    id: r.id,
                    pages: r.pages.clone(),
                    volume: r.volume.clone(),
                    number: r.number.clone(),
                })
                .collect())
        }
    }

    #[derive(Default)]
    struct RecordingWriter {
        calls: std::sync::Mutex<Vec<NumericFieldsUpdate>>,
    }

    impl NumericFieldsWriter for RecordingWriter {
        async fn write_numeric_fields(
            &self,
            updates: &[NumericFieldsUpdate],
        ) -> Result<usize, HexforgeError> {
            let n = updates.len();
            self.calls
                .lock()
                .unwrap()
                .extend(updates.iter().map(|u| NumericFieldsUpdate {
                    id: u.id,
                    start_page: u.start_page,
                    volume_numeric: u.volume_numeric,
                    number_numeric: u.number_numeric,
                }));
            Ok(n)
        }
    }

    fn recorded(writer: &RecordingWriter) -> Vec<RecordedRow> {
        writer
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|u| (u.id, u.start_page, u.volume_numeric, u.number_numeric))
            .collect()
    }

    #[tokio::test]
    async fn computes_all_three_fields() {
        let fetcher = MockFetcher::new(vec![(1, Some("123--456"), Some("10"), Some("3/4"))]);
        let writer = RecordingWriter::default();
        let report = compute_numeric_fields(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(1, Some(123), Some(10), Some(3))]);
        assert_eq!(report.updated, 1);
    }

    #[tokio::test]
    async fn null_fields_produce_null_numerics() {
        let fetcher = MockFetcher::new(vec![(2, None, None, None)]);
        let writer = RecordingWriter::default();
        compute_numeric_fields(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(2, None, None, None)]);
    }

    #[tokio::test]
    async fn empty_input_produces_zero_updated() {
        let fetcher = MockFetcher::new(vec![]);
        let writer = RecordingWriter::default();
        let report = compute_numeric_fields(&fetcher, &writer).await.unwrap();
        assert_eq!(report.updated, 0);
        assert!(writer.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn roman_volume_and_range_number() {
        let fetcher = MockFetcher::new(vec![(3, Some("xii--xiv"), Some("III"), Some("1--3"))]);
        let writer = RecordingWriter::default();
        compute_numeric_fields(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(3, Some(12), Some(3), Some(1))]);
    }

    #[tokio::test]
    async fn series_volume_encoding() {
        let fetcher = MockFetcher::new(vec![(4, None, Some("s2-4"), Some("suppl., 2"))]);
        let writer = RecordingWriter::default();
        compute_numeric_fields(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(4, None, Some(2004), Some(2))]);
    }

    #[tokio::test]
    async fn unparseable_text_produces_null() {
        let fetcher = MockFetcher::new(vec![(
            5,
            Some("e12936"),
            Some("special issue"),
            Some("suppl."),
        )]);
        let writer = RecordingWriter::default();
        compute_numeric_fields(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(5, None, None, None)]);
    }
}
