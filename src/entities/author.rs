//! Author entity — maps to the `authors` table.

use chrono::{DateTime, Utc};
use hexforge::Entity;
use hexforge::sqlx_exports::FromRow;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// An author in the bibliography system.
///
/// Authors may have a given + family name, a mononym (e.g., Plato),
/// or both. Each name component has LaTeX, Unicode, and simplified variants.
#[derive(Entity, FromRow, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "authors")]
pub struct Author {
    #[entity(id)]
    pub id: i64,
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

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
