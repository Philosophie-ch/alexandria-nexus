//! Pure validation functions for School DTOs.
//!
//! NO I/O — these are pure functions that validate input data.

use hexforge::ValidationError;

use crate::domain::{CreateSchool, UpdateSchool};

/// Validate a CreateSchool request.
/// Pure function — NO I/O.
///
/// A school must have a non-empty key and at least one name variant.
pub fn validate_create_school(input: &CreateSchool) -> Result<(), ValidationError> {
    if input.school_key.trim().is_empty() {
        return Err(ValidationError::required("school_key"));
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

/// Validate an UpdateSchool request.
/// Pure function — NO I/O.
pub fn validate_update_school(input: &UpdateSchool) -> Result<(), ValidationError> {
    if let Some(ref key) = input.school_key
        && key.trim().is_empty()
    {
        return Err(ValidationError::invalid_value(
            "school_key",
            "school_key cannot be empty",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_school() {
        let input = CreateSchool {
            school_key: "mit_linguistics".to_string(),
            name_latex: Some("MIT Linguistics".to_string()),
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_create_school(&input).is_ok());
    }

    #[test]
    fn test_school_without_key() {
        let input = CreateSchool {
            school_key: String::new(),
            name_latex: Some("MIT Linguistics".to_string()),
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_create_school(&input).is_err());
    }

    #[test]
    fn test_school_without_name() {
        let input = CreateSchool {
            school_key: "mit_linguistics".to_string(),
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_create_school(&input).is_err());
    }

    #[test]
    fn test_update_empty_key() {
        let input = UpdateSchool {
            school_key: Some(String::new()),
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_update_school(&input).is_err());
    }

    #[test]
    fn test_update_valid() {
        let input = UpdateSchool {
            school_key: None,
            name_latex: Some("New Name".to_string()),
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_update_school(&input).is_ok());
    }
}
