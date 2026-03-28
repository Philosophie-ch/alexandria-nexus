//! Domain layer — pure types and entities.
//!
//! Contains business types (enums, validated types) and entity structs.
//! Entity annotations (#[derive(Entity)], #[derive(Crud)]) are metadata —
//! they don't introduce I/O or framework coupling.

pub mod enums;
pub mod bib_string;
pub mod projections;

mod author;
mod bibitem;
mod institution;
mod journal;
mod keyword;
mod publisher;
mod school;
mod series;

pub use enums::*;
pub use bib_string::*;

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
