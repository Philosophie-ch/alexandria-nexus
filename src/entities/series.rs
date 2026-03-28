//! Series entity — maps to the `series` table.

use chrono::{DateTime, Utc};
use hexforge::{Crud, Entity};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A publication series in the bibliography system.
#[derive(Entity, Crud, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "series")]
pub struct Series {
    #[entity(id)]
    pub id: i64,
    #[crud(required)]
    pub series_key: String,
    pub name_latex: String,
    pub name_unicode: String,
    pub name_simplified: String,

    #[crud(skip)]
    pub created_at: DateTime<Utc>,
    #[crud(skip)]
    pub updated_at: DateTime<Utc>,
}
