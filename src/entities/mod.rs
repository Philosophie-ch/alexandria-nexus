//! Database entities — structs that map directly to database tables.
//!
//! Each entity uses `#[derive(Entity)]` to generate database implementations
//! with zero manual SQL. Field names match database column names exactly.
//! `#[derive(Crud)]` generates CreateDTO, UpdateDTO, and transform functions.

mod author;
mod bibitem;
mod db_mappings;
mod institution;
mod journal;
mod keyword;
mod publisher;
mod school;
mod series;

pub use author::{
    Author, CreateAuthor, UpdateAuthor, create_author_transform, update_author_transform,
};
pub use bibitem::{
    BibItem, CreateBibItem, UpdateBibItem, create_bib_item_transform, update_bib_item_transform,
};
pub use institution::{
    CreateInstitution, Institution, UpdateInstitution, create_institution_transform,
    update_institution_transform,
};
pub use journal::{
    CreateJournal, Journal, UpdateJournal, create_journal_transform, update_journal_transform,
};
pub use keyword::{
    CreateKeyword, Keyword, UpdateKeyword, create_keyword_transform, update_keyword_transform,
};
pub use publisher::{
    CreatePublisher, Publisher, UpdatePublisher, create_publisher_transform,
    update_publisher_transform,
};
pub use school::{
    CreateSchool, School, UpdateSchool, create_school_transform, update_school_transform,
};
pub use series::{
    CreateSeries, Series, UpdateSeries, create_series_transform, update_series_transform,
};
