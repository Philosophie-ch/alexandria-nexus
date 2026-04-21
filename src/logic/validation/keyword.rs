//! Pure validation functions for Keyword DTOs.
//!
//! NO I/O — these are pure functions that validate input data.
//!
//! Keywords are flat — each has a name and level (1-3), no parent hierarchy.

use hexforge::ValidationError;

use crate::domain::{CreateKeyword, UpdateKeyword};

/// Validate a CreateKeyword request.
/// Pure function — NO I/O.
pub fn validate_create_keyword(input: &CreateKeyword) -> Result<(), ValidationError> {
    // Name is required
    if input.name.trim().is_empty() {
        return Err(ValidationError::required("name"));
    }

    // Level must be 1-3
    if !(1..=3).contains(&input.level) {
        return Err(ValidationError::invalid_value(
            "level",
            "level must be between 1 and 3",
        ));
    }

    Ok(())
}

/// Validate an UpdateKeyword request.
/// Pure function — NO I/O.
pub fn validate_update_keyword(input: &UpdateKeyword) -> Result<(), ValidationError> {
    // If name is provided, it must not be empty
    if let Some(ref name) = input.name
        && name.trim().is_empty()
    {
        return Err(ValidationError::invalid_value(
            "name",
            "name cannot be empty",
        ));
    }

    // If level is provided, it must be 1-3
    if let Some(level) = input.level
        && !(1..=3).contains(&level)
    {
        return Err(ValidationError::invalid_value(
            "level",
            "level must be between 1 and 3",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_level_1_keyword() {
        let input = CreateKeyword {
            keyword_key: "1:Philosophy".to_string(),
            name: "Philosophy".to_string(),
            level: 1,
        };
        assert!(validate_create_keyword(&input).is_ok());
    }

    #[test]
    fn test_valid_level_3_keyword() {
        let input = CreateKeyword {
            keyword_key: "3:Metaethics".to_string(),
            name: "Metaethics".to_string(),
            level: 3,
        };
        assert!(validate_create_keyword(&input).is_ok());
    }

    #[test]
    fn test_invalid_level_zero() {
        let input = CreateKeyword {
            keyword_key: "0:Test".to_string(),
            name: "Test".to_string(),
            level: 0,
        };
        assert!(validate_create_keyword(&input).is_err());
    }

    #[test]
    fn test_invalid_level_four() {
        let input = CreateKeyword {
            keyword_key: "4:Test".to_string(),
            name: "Test".to_string(),
            level: 4,
        };
        assert!(validate_create_keyword(&input).is_err());
    }

    #[test]
    fn test_empty_name() {
        let input = CreateKeyword {
            keyword_key: "1:".to_string(),
            name: String::new(),
            level: 1,
        };
        assert!(validate_create_keyword(&input).is_err());
    }

    #[test]
    fn test_whitespace_only_name() {
        let input = CreateKeyword {
            keyword_key: "1:   ".to_string(),
            name: "   ".to_string(),
            level: 1,
        };
        assert!(validate_create_keyword(&input).is_err());
    }

    #[test]
    fn test_update_empty_name() {
        let input = UpdateKeyword {
            keyword_key: None,
            name: Some(String::new()),
            level: None,
        };
        assert!(validate_update_keyword(&input).is_err());
    }

    #[test]
    fn test_update_invalid_level() {
        let input = UpdateKeyword {
            keyword_key: None,
            name: None,
            level: Some(5),
        };
        assert!(validate_update_keyword(&input).is_err());
    }

    #[test]
    fn test_update_valid() {
        let input = UpdateKeyword {
            keyword_key: None,
            name: Some("Western Philosophy".to_string()),
            level: None,
        };
        assert!(validate_update_keyword(&input).is_ok());
    }
}
