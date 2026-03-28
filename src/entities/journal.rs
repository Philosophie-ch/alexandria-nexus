//! Journal entity — maps to the `journals` table.

use chrono::{DateTime, Utc};
use hexforge::{Crud, Entity};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A journal in the bibliography system.
#[derive(Entity, Crud, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "journals")]
pub struct Journal {
    #[entity(id)]
    pub id: i64,
    #[crud(required)]
    pub journal_key: String,
    pub name_latex: String,
    pub name_unicode: String,
    pub name_simplified: String,
    pub issn_print: Option<String>,
    pub issn_electronic: Option<String>,

    #[crud(skip)]
    pub created_at: DateTime<Utc>,
    #[crud(skip)]
    pub updated_at: DateTime<Utc>,
}
