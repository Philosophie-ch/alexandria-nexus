//! School entity — maps to the `schools` table.

use chrono::{DateTime, Utc};
use hexforge::{Crud, Entity};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A school in the bibliography system (for theses).
#[derive(Entity, Crud, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "schools")]
pub struct School {
    #[entity(id)]
    pub id: i64,
    #[crud(required)]
    pub school_key: String,
    pub name_latex: String,
    pub name_unicode: String,
    pub name_simplified: String,
    pub default_address: Option<String>,

    #[crud(skip)]
    pub created_at: DateTime<Utc>,
    #[crud(skip)]
    pub updated_at: DateTime<Utc>,
}
