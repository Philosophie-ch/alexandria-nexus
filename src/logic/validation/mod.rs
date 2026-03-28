//! Pure validation functions for API requests.
//!
//! All validation functions are pure (no I/O) and return `Result<(), ValidationError>`.

mod author;
mod bibitem;
mod institution;
mod journal;
mod keyword;
mod publisher;
mod school;
mod series;

pub use author::{validate_create_author, validate_update_author};
pub use bibitem::{validate_create_bibitem, validate_update_bibitem};
pub use institution::{validate_create_institution, validate_update_institution};
pub use journal::{validate_create_journal, validate_update_journal};
pub use keyword::{validate_create_keyword, validate_update_keyword};
pub use publisher::{validate_create_publisher, validate_update_publisher};
pub use school::{validate_create_school, validate_update_school};
pub use series::{validate_create_series, validate_update_series};
