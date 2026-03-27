//! Journal DTOs for API requests.

use serde::Deserialize;
use utoipa::ToSchema;

/// DTO for creating a journal.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateJournal {
    /// Unique identifier key for the journal (required)
    pub journal_key: String,

    // Name variants
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,

    // ISSN numbers
    pub issn_print: Option<String>,
    pub issn_electronic: Option<String>,
}

/// DTO for updating a journal. All fields are optional for partial updates.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateJournal {
    pub journal_key: Option<String>,
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,
    pub issn_print: Option<String>,
    pub issn_electronic: Option<String>,
}
