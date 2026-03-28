//! Pure validation functions for Publisher DTOs.
//!
//! NO I/O — these are pure functions that validate input data.

use hexforge::ValidationError;

use crate::entities::{CreatePublisher, UpdatePublisher};

/// Validate a CreatePublisher request.
/// Pure function — NO I/O.
///
/// A publisher must have a non-empty key and at least one name variant.
pub fn validate_create_publisher(input: &CreatePublisher) -> Result<(), ValidationError> {
    if input.publisher_key.trim().is_empty() {
        return Err(ValidationError::required("publisher_key"));
    }

    let has_name = input
        .name_latex
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
        || input
            .name_unicode
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
        || input
            .name_simplified
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());

    if !has_name {
        return Err(ValidationError::required("name"));
    }

    Ok(())
}

/// Validate an UpdatePublisher request.
/// Pure function — NO I/O.
pub fn validate_update_publisher(input: &UpdatePublisher) -> Result<(), ValidationError> {
    if let Some(ref key) = input.publisher_key
        && key.trim().is_empty()
    {
        return Err(ValidationError::invalid_value(
            "publisher_key",
            "publisher_key cannot be empty",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_publisher() {
        let input = CreatePublisher {
            publisher_key: "springer".to_string(),
            name_latex: Some("Springer".to_string()),
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_create_publisher(&input).is_ok());
    }

    #[test]
    fn test_publisher_without_key() {
        let input = CreatePublisher {
            publisher_key: String::new(),
            name_latex: Some("Springer".to_string()),
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_create_publisher(&input).is_err());
    }

    #[test]
    fn test_publisher_without_name() {
        let input = CreatePublisher {
            publisher_key: "springer".to_string(),
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_create_publisher(&input).is_err());
    }

    #[test]
    fn test_update_empty_key() {
        let input = UpdatePublisher {
            publisher_key: Some(String::new()),
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_update_publisher(&input).is_err());
    }

    #[test]
    fn test_update_valid() {
        let input = UpdatePublisher {
            publisher_key: None,
            name_latex: Some("New Name".to_string()),
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_update_publisher(&input).is_ok());
    }
}
