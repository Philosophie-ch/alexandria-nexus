//! Query filter for bibliography items — the most complex filter.

use hexforge::Filter;
use serde::Deserialize;

/// Query parameters for filtering bibliography items.
///
/// - `entry_type` — exact match on `entry_type` (cast to enum text)
/// - `year_from` / `year_to` — range filter on `date_year`
/// - `author_id` — EXISTS subquery on `bibitem_authors` junction table
/// - `journal_id` — exact match on `journal_id`
/// - `epoch` — exact match on `epoch` (cast to enum text)
/// - `search_term` — case-insensitive substring match on `title_simplified`
#[derive(Filter, Debug, Default, Deserialize)]
pub struct BibItemQuery {
    #[query(eq_cast = "entry_type::entry_type")]
    pub entry_type: Option<String>,
    #[query(gte = "date_year")]
    pub year_from: Option<i16>,
    #[query(lte = "date_year")]
    pub year_to: Option<i16>,
    #[query(raw = "id IN (SELECT bibitem_id FROM bibitem_authors WHERE author_id = $)")]
    pub author_id: Option<i64>,
    #[query(eq = "journal_id")]
    pub journal_id: Option<i64>,
    #[query(eq_cast = "epoch::epoch")]
    pub epoch: Option<String>,
    #[query(like = "title_simplified")]
    pub search_term: Option<String>,
}
