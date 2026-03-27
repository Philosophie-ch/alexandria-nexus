//! Series DTOs for API requests.

use serde::Deserialize;
use utoipa::ToSchema;

/// DTO for creating a series.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateSeries {
    /// Unique identifier key for the series (required)
    pub series_key: String,

    // Name variants
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,
}

/// DTO for updating a series. All fields are optional for partial updates.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateSeries {
    pub series_key: Option<String>,
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,
}
