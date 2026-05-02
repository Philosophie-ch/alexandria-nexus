use std::future::Future;

use hexforge::HexforgeError;
use serde::Serialize;

use crate::logic::pages::compute_start_page;

#[derive(Debug, Serialize)]
pub struct ComputeStartPagesReport {
    pub updated: usize,
}

pub struct BibitemPagesRow {
    pub id: i64,
    pub pages: Option<String>,
}

pub struct StartPageUpdate {
    pub id: i64,
    pub start_page: Option<i32>,
}

/// Fetch all (id, pages) pairs from the bibitems table.
pub trait BibitemPagesFetcher: Send + Sync {
    fn fetch_pages(
        &self,
    ) -> impl Future<Output = Result<Vec<BibitemPagesRow>, HexforgeError>> + Send;
}

/// Write computed start_page values back to bibitems.
pub trait StartPageWriter: Send + Sync {
    fn write_start_pages(
        &self,
        updates: &[StartPageUpdate],
    ) -> impl Future<Output = Result<usize, HexforgeError>> + Send;
}

/// Compute and persist `start_page` for every bibitem from its `pages` string.
///
/// Idempotent — safe to re-run. Overwrites any existing `start_page` values.
pub async fn compute_start_pages(
    fetcher: &impl BibitemPagesFetcher,
    writer: &impl StartPageWriter,
) -> Result<ComputeStartPagesReport, HexforgeError> {
    let rows = fetcher.fetch_pages().await?;
    let updates: Vec<StartPageUpdate> = rows
        .into_iter()
        .map(|row| StartPageUpdate {
            id: row.id,
            start_page: compute_start_page(row.pages.as_deref()),
        })
        .collect();
    let updated = writer.write_start_pages(&updates).await?;
    Ok(ComputeStartPagesReport { updated })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockFetcher(Vec<BibitemPagesRow>);

    impl MockFetcher {
        fn from(pairs: Vec<(i64, Option<&str>)>) -> Self {
            Self(
                pairs
                    .into_iter()
                    .map(|(id, pages)| BibitemPagesRow {
                        id,
                        pages: pages.map(str::to_owned),
                    })
                    .collect(),
            )
        }
    }

    impl BibitemPagesFetcher for MockFetcher {
        async fn fetch_pages(&self) -> Result<Vec<BibitemPagesRow>, HexforgeError> {
            Ok(self
                .0
                .iter()
                .map(|r| BibitemPagesRow {
                    id: r.id,
                    pages: r.pages.clone(),
                })
                .collect())
        }
    }

    #[derive(Default)]
    struct RecordingWriter {
        calls: std::sync::Mutex<Vec<StartPageUpdate>>,
    }

    impl StartPageWriter for RecordingWriter {
        async fn write_start_pages(
            &self,
            updates: &[StartPageUpdate],
        ) -> Result<usize, HexforgeError> {
            let n = updates.len();
            self.calls
                .lock()
                .unwrap()
                .extend(updates.iter().map(|u| StartPageUpdate {
                    id: u.id,
                    start_page: u.start_page,
                }));
            Ok(n)
        }
    }

    fn recorded(writer: &RecordingWriter) -> Vec<(i64, Option<i32>)> {
        writer
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|u| (u.id, u.start_page))
            .collect()
    }

    #[tokio::test]
    async fn numeric_pages_compute_start_page() {
        let fetcher = MockFetcher::from(vec![(1, Some("123--456"))]);
        let writer = RecordingWriter::default();
        let report = compute_start_pages(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(1, Some(123))]);
        assert_eq!(report.updated, 1);
    }

    #[tokio::test]
    async fn roman_numeral_pages_converted() {
        let fetcher = MockFetcher::from(vec![(2, Some("xii--xiv"))]);
        let writer = RecordingWriter::default();
        compute_start_pages(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(2, Some(12))]);
    }

    #[tokio::test]
    async fn null_pages_produces_null_start_page() {
        let fetcher = MockFetcher::from(vec![(3, None)]);
        let writer = RecordingWriter::default();
        compute_start_pages(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(3, None)]);
    }

    #[tokio::test]
    async fn unparseable_pages_produces_null_start_page() {
        let fetcher = MockFetcher::from(vec![(4, Some("e12936"))]);
        let writer = RecordingWriter::default();
        compute_start_pages(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(4, None)]);
    }

    #[tokio::test]
    async fn empty_input_produces_zero_updated() {
        let fetcher = MockFetcher::from(vec![]);
        let writer = RecordingWriter::default();
        let report = compute_start_pages(&fetcher, &writer).await.unwrap();
        assert_eq!(report.updated, 0);
        assert!(writer.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn multi_entry_pages_picks_minimum() {
        let fetcher = MockFetcher::from(vec![(5, Some("200--300, 50"))]);
        let writer = RecordingWriter::default();
        compute_start_pages(&fetcher, &writer).await.unwrap();
        assert_eq!(recorded(&writer), [(5, Some(50))]);
    }
}
