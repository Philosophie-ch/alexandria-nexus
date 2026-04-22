//! Keyword tree process — defines the contract and orchestrates keyword tree building.
//!
//! The trait `KeywordFetcher` declares WHAT data fetching needs to happen.
//! Concrete implementations live in the adapters layer.

use hexforge::HexforgeError;

use crate::domain::Keyword;
use crate::logic::keyword_tree::KeywordTree;

/// Contract for fetching all keywords in a single query.
///
/// Implementations live in the adapters layer (e.g., `PgKeywordFetcher`).
pub trait KeywordFetcher {
    fn fetch_all(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Keyword>, HexforgeError>> + Send;
}

/// Fetch all keywords in one query and partition by level into a tree.
pub async fn build_keyword_tree(
    fetcher: &impl KeywordFetcher,
) -> Result<KeywordTree, HexforgeError> {
    let all = fetcher.fetch_all().await?;
    let mut level_1 = Vec::new();
    let mut level_2 = Vec::new();
    let mut level_3 = Vec::new();
    for kw in all {
        match kw.level {
            1 => level_1.push(kw),
            2 => level_2.push(kw),
            _ => level_3.push(kw),
        }
    }
    Ok(KeywordTree {
        level_1,
        level_2,
        level_3,
    })
}
