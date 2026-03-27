//! Series entity — maps to the `series` table.

use chrono::{DateTime, Utc};
use hexforge::Entity;
use hexforge::sqlx_exports::FromRow;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A publication series in the bibliography system.
#[derive(Entity, FromRow, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "series")]
pub struct Series {
    #[entity(id)]
    pub id: i64,
    pub series_key: String,
    pub name_latex: String,
    pub name_unicode: String,
    pub name_simplified: String,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
