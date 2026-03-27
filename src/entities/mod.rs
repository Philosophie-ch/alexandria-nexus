//! Database entities — structs that map directly to database tables.
//!
//! Each entity uses `#[derive(Entity)]` to generate database implementations
//! with zero manual SQL. Field names match database column names exactly.

mod author;
mod bibitem;
mod db_mappings;
mod institution;
mod journal;
mod keyword;
mod publisher;
mod school;
mod series;

pub use author::Author;
pub use bibitem::BibItem;
pub use institution::Institution;
pub use journal::Journal;
pub use keyword::Keyword;
pub use publisher::Publisher;
pub use school::School;
pub use series::Series;
