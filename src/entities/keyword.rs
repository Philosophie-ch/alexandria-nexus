//! Keyword entity — maps to the `keywords` table.

use chrono::{DateTime, Utc};
use hexforge::Entity;
use hexforge::sqlx_exports::FromRow;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A keyword with a hierarchical level (1-3).
#[derive(Entity, FromRow, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "keywords")]
pub struct Keyword {
    #[entity(id)]
    pub id: i64,
    pub name: String,
    pub level: i16,
    pub created_at: DateTime<Utc>,
}
