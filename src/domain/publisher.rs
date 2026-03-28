//! Publisher entity — maps to the `publishers` table.

use chrono::{DateTime, Utc};
use hexforge::{Crud, Entity};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A publisher in the bibliography system.
#[derive(Entity, Crud, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "publishers")]
pub struct Publisher {
    #[entity(id)]
    pub id: i64,
    #[crud(required)]
    pub publisher_key: String,
    pub name_latex: String,
    pub name_unicode: String,
    pub name_simplified: String,
    pub default_address: Option<String>,

    #[crud(skip)]
    pub created_at: DateTime<Utc>,
    #[crud(skip)]
    pub updated_at: DateTime<Utc>,
}
