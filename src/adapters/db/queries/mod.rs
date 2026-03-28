//! Query filters for list endpoints.
//!
//! Each query struct is deserialized from URL query parameters and implements
//! [`hexforge::QueryFilter`] to generate PostgreSQL WHERE clauses automatically.
//! The [`DataStore`](hexforge::DataStore) uses these to filter `find_all`, `count`,
//! and `stream` without any manual SQL in the handler layer.

mod author;
mod bibitem;
mod institution;
mod journal;
mod keyword;
mod publisher;
mod school;
mod series;

pub use author::AuthorQuery;
pub use bibitem::BibItemQuery;
pub use institution::InstitutionQuery;
pub use journal::JournalQuery;
pub use keyword::KeywordQuery;
pub use publisher::PublisherQuery;
pub use school::SchoolQuery;
pub use series::SeriesQuery;
