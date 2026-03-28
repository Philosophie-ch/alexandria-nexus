//! Keyword entity — maps to the `keywords` table.

use chrono::{DateTime, Utc};
use hexforge::{Crud, Entity};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A keyword with a hierarchical level (1-3).
#[derive(Entity, Crud, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "keywords")]
pub struct Keyword {
    #[entity(id)]
    pub id: i64,
    #[crud(required)]
    pub name: String,
    #[crud(required)]
    pub level: i16,
    #[crud(skip)]
    pub created_at: DateTime<Utc>,
}
