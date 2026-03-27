//! Institution entity — maps to the `institutions` table.

use chrono::{DateTime, Utc};
use hexforge::Entity;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// An institution in the bibliography system (for tech reports, etc.).
#[derive(Entity, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "institutions")]
pub struct Institution {
    #[entity(id)]
    pub id: i64,
    pub institution_key: String,
    pub name_latex: String,
    pub name_unicode: String,
    pub name_simplified: String,
    pub default_address: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
