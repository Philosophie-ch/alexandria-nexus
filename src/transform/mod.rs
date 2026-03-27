//! Pure transform functions for converting DTOs to entities.
//!
//! All transform functions are pure (no I/O) — just data mapping.

mod author;
mod bibitem;
mod institution;
mod journal;
mod keyword;
mod publisher;
mod school;
mod series;

pub use author::{create_author_transform, update_author_transform};
pub use bibitem::{create_bibitem_transform, update_bibitem_transform};
pub use institution::{create_institution_transform, update_institution_transform};
pub use journal::{create_journal_transform, update_journal_transform};
pub use keyword::{create_keyword_transform, update_keyword_transform};
pub use publisher::{create_publisher_transform, update_publisher_transform};
pub use school::{create_school_transform, update_school_transform};
pub use series::{create_series_transform, update_series_transform};
