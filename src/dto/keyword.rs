//! Keyword DTOs for API requests.

use serde::Deserialize;
use utoipa::ToSchema;

/// DTO for creating a keyword.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateKeyword {
    pub name: String,
    pub level: i16,
}

/// DTO for updating a keyword. All fields are optional for partial updates.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateKeyword {
    pub name: Option<String>,
    pub level: Option<i16>,
}
