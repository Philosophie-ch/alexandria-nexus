//! DTOs (Data Transfer Objects) for API requests.
//!
//! Create DTOs define the fields a client provides to create a new entity.
//! Update DTOs use `Option<T>` for all fields to support partial updates.

mod author;
mod bibitem;
mod institution;
mod journal;
mod keyword;
mod publisher;
mod school;
mod series;

pub use author::{CreateAuthor, UpdateAuthor};
pub use bibitem::{CreateBibItem, UpdateBibItem};
pub use institution::{CreateInstitution, UpdateInstitution};
pub use journal::{CreateJournal, UpdateJournal};
pub use keyword::{CreateKeyword, UpdateKeyword};
pub use publisher::{CreatePublisher, UpdatePublisher};
pub use school::{CreateSchool, UpdateSchool};
pub use series::{CreateSeries, UpdateSeries};
