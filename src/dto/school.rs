//! School DTOs for API requests.

use serde::Deserialize;
use utoipa::ToSchema;

/// DTO for creating a school.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateSchool {
    /// Unique identifier key for the school (required)
    pub school_key: String,

    // Name variants
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,

    /// Default address for this school
    pub default_address: Option<String>,
}

/// DTO for updating a school. All fields are optional for partial updates.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateSchool {
    pub school_key: Option<String>,
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,
    pub default_address: Option<String>,
}
