//! Author DTOs for API requests.

use serde::Deserialize;
use utoipa::ToSchema;

/// DTO for creating an author.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateAuthor {
    /// Unique identifier key for the author (required)
    pub author_key: String,

    // Given name (BibStringAttr: latex, unicode, simplified)
    pub given_name_latex: Option<String>,
    pub given_name_unicode: Option<String>,
    pub given_name_simplified: Option<String>,

    // Family name (BibStringAttr: latex, unicode, simplified)
    pub family_name_latex: Option<String>,
    pub family_name_unicode: Option<String>,
    pub family_name_simplified: Option<String>,

    // Mononym — single-name authors (Plato, Aristotle)
    pub mononym_latex: Option<String>,
    pub mononym_unicode: Option<String>,
    pub mononym_simplified: Option<String>,

    // Display shorthand
    pub shorthand_latex: Option<String>,
    pub shorthand_unicode: Option<String>,
    pub shorthand_simplified: Option<String>,

    // Famous name for profiles
    pub famous_name_latex: Option<String>,
    pub famous_name_unicode: Option<String>,
    pub famous_name_simplified: Option<String>,
}

/// DTO for updating an author. All fields are optional for partial updates.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateAuthor {
    pub author_key: Option<String>,
    pub given_name_latex: Option<String>,
    pub given_name_unicode: Option<String>,
    pub given_name_simplified: Option<String>,
    pub family_name_latex: Option<String>,
    pub family_name_unicode: Option<String>,
    pub family_name_simplified: Option<String>,
    pub mononym_latex: Option<String>,
    pub mononym_unicode: Option<String>,
    pub mononym_simplified: Option<String>,
    pub shorthand_latex: Option<String>,
    pub shorthand_unicode: Option<String>,
    pub shorthand_simplified: Option<String>,
    pub famous_name_latex: Option<String>,
    pub famous_name_unicode: Option<String>,
    pub famous_name_simplified: Option<String>,
}
