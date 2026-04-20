//! Snapshot process — defines traits for full-database fetching.
//!
//! The snapshot endpoint generates a ZIP of all data tables. This module
//! defines the I/O contracts. Serialization and ZIP packaging live in
//! the adapter layer.
//!
//! No AppState, no PgPool, no sqlx, no SQL — only abstract contracts.

use std::future::Future;

use hexforge::HexforgeError;

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow, BibitemRefsRow};
use crate::domain::{
    Author, BibItem, BibitemNotes, Institution, Journal, Keyword, Publisher, School, Series,
};

// =============================================================================
// Data container
// =============================================================================

/// All data needed for a full snapshot, pre-fetched from the database.
pub struct SnapshotData {
    pub authors: Vec<Author>,
    pub journals: Vec<Journal>,
    pub publishers: Vec<Publisher>,
    pub institutions: Vec<Institution>,
    pub schools: Vec<School>,
    pub series: Vec<Series>,
    pub keywords: Vec<Keyword>,
    pub bibitems: Vec<BibItem>,
    pub bibitem_authors: Vec<BibitemAuthorsRow>,
    pub bibitem_keywords: Vec<BibitemKeywordsRow>,
    pub bibitem_refs: Vec<BibitemRefsRow>,
    pub bibitem_notes: Vec<BibitemNotes>,
}

// =============================================================================
// Trait contract
// =============================================================================

/// Contract for fetching all tables required for a snapshot.
pub trait SnapshotFetcher: Send + Sync {
    fn fetch_authors(&self) -> impl Future<Output = Result<Vec<Author>, HexforgeError>> + Send;
    fn fetch_journals(&self) -> impl Future<Output = Result<Vec<Journal>, HexforgeError>> + Send;
    fn fetch_publishers(
        &self,
    ) -> impl Future<Output = Result<Vec<Publisher>, HexforgeError>> + Send;
    fn fetch_institutions(
        &self,
    ) -> impl Future<Output = Result<Vec<Institution>, HexforgeError>> + Send;
    fn fetch_schools(&self) -> impl Future<Output = Result<Vec<School>, HexforgeError>> + Send;
    fn fetch_series(&self) -> impl Future<Output = Result<Vec<Series>, HexforgeError>> + Send;
    fn fetch_keywords(&self) -> impl Future<Output = Result<Vec<Keyword>, HexforgeError>> + Send;
    fn fetch_bibitems(&self) -> impl Future<Output = Result<Vec<BibItem>, HexforgeError>> + Send;
    fn fetch_bibitem_authors(
        &self,
    ) -> impl Future<Output = Result<Vec<BibitemAuthorsRow>, HexforgeError>> + Send;
    fn fetch_bibitem_keywords(
        &self,
    ) -> impl Future<Output = Result<Vec<BibitemKeywordsRow>, HexforgeError>> + Send;
    fn fetch_bibitem_refs(
        &self,
    ) -> impl Future<Output = Result<Vec<BibitemRefsRow>, HexforgeError>> + Send;
    fn fetch_bibitem_notes(
        &self,
    ) -> impl Future<Output = Result<Vec<BibitemNotes>, HexforgeError>> + Send;
}

// =============================================================================
// Orchestration
// =============================================================================

/// Fetch all data for a snapshot — all 12 tables are independent, run concurrently.
pub async fn fetch_snapshot(fetcher: &impl SnapshotFetcher) -> Result<SnapshotData, HexforgeError> {
    let (
        authors,
        journals,
        publishers,
        institutions,
        schools,
        series,
        keywords,
        bibitems,
        bibitem_authors,
        bibitem_keywords,
        bibitem_refs,
        bibitem_notes,
    ) = tokio::try_join!(
        fetcher.fetch_authors(),
        fetcher.fetch_journals(),
        fetcher.fetch_publishers(),
        fetcher.fetch_institutions(),
        fetcher.fetch_schools(),
        fetcher.fetch_series(),
        fetcher.fetch_keywords(),
        fetcher.fetch_bibitems(),
        fetcher.fetch_bibitem_authors(),
        fetcher.fetch_bibitem_keywords(),
        fetcher.fetch_bibitem_refs(),
        fetcher.fetch_bibitem_notes(),
    )?;
    Ok(SnapshotData {
        authors,
        journals,
        publishers,
        institutions,
        schools,
        series,
        keywords,
        bibitems,
        bibitem_authors,
        bibitem_keywords,
        bibitem_refs,
        bibitem_notes,
    })
}
