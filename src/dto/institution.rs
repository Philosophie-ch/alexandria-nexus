//! Institution DTOs for API requests.

use serde::Deserialize;
use utoipa::ToSchema;

/// DTO for creating an institution.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateInstitution {
    /// Unique identifier key for the institution (required)
    pub institution_key: String,

    // Name variants
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,

    /// Default address for this institution
    pub default_address: Option<String>,
}

/// DTO for updating an institution. All fields are optional for partial updates.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateInstitution {
    pub institution_key: Option<String>,
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,
    pub default_address: Option<String>,
}
