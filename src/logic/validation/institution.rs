//! Pure validation functions for Institution DTOs.
//!
//! NO I/O — these are pure functions that validate input data.

use hexforge::ValidationError;

use crate::domain::{CreateInstitution, UpdateInstitution};

/// Validate a CreateInstitution request.
/// Pure function — NO I/O.
///
/// An institution must have a non-empty key and at least one name variant.
pub fn validate_create_institution(input: &CreateInstitution) -> Result<(), ValidationError> {
    if input.institution_key.trim().is_empty() {
        return Err(ValidationError::required("institution_key"));
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

/// Validate an UpdateInstitution request.
/// Pure function — NO I/O.
pub fn validate_update_institution(input: &UpdateInstitution) -> Result<(), ValidationError> {
    if let Some(ref key) = input.institution_key
        && key.trim().is_empty()
    {
        return Err(ValidationError::invalid_value(
            "institution_key",
            "institution_key cannot be empty",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_institution() {
        let input = CreateInstitution {
            institution_key: "csli_stanford".to_string(),
            name_latex: Some("CSLI".to_string()),
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_create_institution(&input).is_ok());
    }

    #[test]
    fn test_institution_without_key() {
        let input = CreateInstitution {
            institution_key: String::new(),
            name_latex: Some("CSLI".to_string()),
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_create_institution(&input).is_err());
    }

    #[test]
    fn test_institution_without_name() {
        let input = CreateInstitution {
            institution_key: "csli_stanford".to_string(),
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_create_institution(&input).is_err());
    }

    #[test]
    fn test_update_empty_key() {
        let input = UpdateInstitution {
            institution_key: Some(String::new()),
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_update_institution(&input).is_err());
    }

    #[test]
    fn test_update_valid() {
        let input = UpdateInstitution {
            institution_key: None,
            name_latex: Some("New Name".to_string()),
            name_unicode: None,
            name_simplified: None,
            default_address: None,
        };
        assert!(validate_update_institution(&input).is_ok());
    }
}
