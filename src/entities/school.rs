//! School entity — maps to the `schools` table.

use chrono::{DateTime, Utc};
use hexforge::Entity;
use hexforge::sqlx_exports::FromRow;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A school in the bibliography system (for theses).
#[derive(Entity, FromRow, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "schools")]
pub struct School {
    #[entity(id)]
    pub id: i64,
    pub school_key: String,
    pub name_latex: String,
    pub name_unicode: String,
    pub name_simplified: String,
    pub default_address: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
