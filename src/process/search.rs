//! Search process — defines the search contract and orchestrates execution.
//!
//! The trait `BibitemSearcher` declares WHAT search needs to happen.
//! Concrete implementations live in the adapters layer.

use hexforge::HexforgeError;

use crate::logic::search::{SearchRequest, SearchResponse};

/// Contract for searching bibitems.
///
/// Implementations live in the adapters layer (e.g., `PgBibitemSearcher`).
pub trait BibitemSearcher {
    fn search(
        &self,
        request: &SearchRequest,
    ) -> impl std::future::Future<Output = Result<SearchResponse, HexforgeError>> + Send;
}

/// Execute a search query using the provided searcher implementation.
///
/// Clamps pagination parameters, then delegates to the searcher.
pub async fn perform_search(
    searcher: &impl BibitemSearcher,
    request: SearchRequest,
) -> Result<SearchResponse, HexforgeError> {
    let clamped = SearchRequest {
        limit: request.limit.clamp(1, 100),
        offset: request.offset.max(0),
        ..request
    };

    searcher.search(&clamped).await
}
