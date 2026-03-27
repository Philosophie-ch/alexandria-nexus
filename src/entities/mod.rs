//! Database entities — structs that map directly to PostgreSQL tables.
//!
//! Each entity uses `#[derive(Entity)]` to generate `PgEntity` implementations
//! with zero manual SQL. Field names match database column names exactly.

mod author;
mod bibitem;
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
