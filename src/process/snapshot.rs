//! Snapshot process — defines traits for full-database fetching.
//!
//! The snapshot endpoint generates a ZIP of all data tables. This module
//! defines the I/O contracts. CSV serialization and ZIP packaging live in
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

/// Fetch all data for a snapshot (sequential — each table is independent).
pub async fn fetch_snapshot(fetcher: &impl SnapshotFetcher) -> Result<SnapshotData, HexforgeError> {
    Ok(SnapshotData {
        authors: fetcher.fetch_authors().await?,
        journals: fetcher.fetch_journals().await?,
        publishers: fetcher.fetch_publishers().await?,
        institutions: fetcher.fetch_institutions().await?,
        schools: fetcher.fetch_schools().await?,
        series: fetcher.fetch_series().await?,
        keywords: fetcher.fetch_keywords().await?,
        bibitems: fetcher.fetch_bibitems().await?,
        bibitem_authors: fetcher.fetch_bibitem_authors().await?,
        bibitem_keywords: fetcher.fetch_bibitem_keywords().await?,
        bibitem_refs: fetcher.fetch_bibitem_refs().await?,
        bibitem_notes: fetcher.fetch_bibitem_notes().await?,
    })
}
