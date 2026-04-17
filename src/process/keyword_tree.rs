//! Keyword tree process — defines the contract and orchestrates keyword tree building.
//!
//! The trait `KeywordFetcher` declares WHAT data fetching needs to happen.
//! Concrete implementations (Postgres, etc.) live in the adapters layer.

use hexforge::HexforgeError;

use crate::domain::Keyword;
use crate::logic::keyword_tree::KeywordTree;

/// Contract for fetching keywords by level.
///
/// Implementations live in the adapters layer (e.g., `PgKeywordFetcher`).
pub trait KeywordFetcher {
    fn fetch_by_level(
        &self,
        level: i16,
    ) -> impl std::future::Future<Output = Result<Vec<Keyword>, HexforgeError>> + Send;
}

/// Fetch all keywords and organize them by level.
pub async fn build_keyword_tree(
    fetcher: &impl KeywordFetcher,
) -> Result<KeywordTree, HexforgeError> {
    let level_1 = fetcher.fetch_by_level(1).await?;
    let level_2 = fetcher.fetch_by_level(2).await?;
    let level_3 = fetcher.fetch_by_level(3).await?;

    Ok(KeywordTree {
        level_1,
        level_2,
        level_3,
    })
}
