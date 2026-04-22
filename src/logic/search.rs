//! Search types — request/response types for bibitem search.
//!
//! Pure data types only. The actual search execution lives in the handler
//! (adapter layer) since it requires DB-specific full-text search capabilities.

use serde::{Deserialize, Serialize};

use crate::domain::{BibItem, EntryType, Epoch};

/// Search request parameters.
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// Search query string (searches title, booktitle using trigram similarity).
    pub query: String,

    /// Filter by entry type.
    pub entry_type: Option<EntryType>,

    /// Filter by year range (from).
    pub year_from: Option<i16>,

    /// Filter by year range (to).
    pub year_to: Option<i16>,

    /// Filter by author ID.
    pub author_id: Option<i64>,

    /// Filter by journal ID.
    pub journal_id: Option<i64>,

    /// Filter by epoch.
    pub epoch: Option<Epoch>,

    /// Maximum number of results (default: 50, max: 100).
    #[serde(default = "default_limit")]
    pub limit: i64,

    /// Offset for pagination.
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// Search response with results and metadata.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<BibItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}
