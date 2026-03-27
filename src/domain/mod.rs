//! Domain layer — pure types with no I/O dependencies.
//!
//! These are the canonical business types used throughout the application:
//! enums, value objects, and string attribute types.

mod bib_string;
mod enums;

pub use bib_string::BibStringAttr;
pub use enums::{AuthorRole, EntryType, Epoch, LangId, PermissionLevel, PubState, RefType};
