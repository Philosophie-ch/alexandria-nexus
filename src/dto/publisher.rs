//! Publisher DTOs for API requests.

use serde::Deserialize;
use utoipa::ToSchema;

/// DTO for creating a publisher.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreatePublisher {
    /// Unique identifier key for the publisher (required)
    pub publisher_key: String,

    // Name variants
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,

    /// Default address for this publisher
    pub default_address: Option<String>,
}

/// DTO for updating a publisher. All fields are optional for partial updates.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdatePublisher {
    pub publisher_key: Option<String>,
    pub name_latex: Option<String>,
    pub name_unicode: Option<String>,
    pub name_simplified: Option<String>,
    pub default_address: Option<String>,
}
