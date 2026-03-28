//! Pure validation functions for Author DTOs.
//!
//! NO I/O — these are pure functions that validate input data.

use hexforge::ValidationError;

use crate::entities::{CreateAuthor, UpdateAuthor};

/// Validate a CreateAuthor request.
/// Pure function — NO I/O.
///
/// An author must have either:
/// - A family name (for "Family, Given" format)
/// - OR a mononym (for single-named authors like "Aristotle")
pub fn validate_create_author(input: &CreateAuthor) -> Result<(), ValidationError> {
    let has_family_name = input
        .family_name_latex
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
        || input
            .family_name_unicode
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
        || input
            .family_name_simplified
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());

    let has_mononym = input
        .mononym_latex
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
        || input
            .mononym_unicode
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
        || input
            .mononym_simplified
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());

    if !has_family_name && !has_mononym {
        return Err(ValidationError::custom(
            "author must have either a family_name or a mononym",
        ));
    }

    Ok(())
}

/// Validate an UpdateAuthor request.
/// Pure function — NO I/O.
///
/// For updates, we allow partial updates so no fields are strictly required.
/// The invariant (family_name OR mononym) is checked at the domain level.
pub fn validate_update_author(_input: &UpdateAuthor) -> Result<(), ValidationError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_author_with_family_name() {
        let input = CreateAuthor {
            author_key: "kant".to_string(),
            given_name_latex: Some("Immanuel".to_string()),
            given_name_unicode: None,
            given_name_simplified: None,
            family_name_latex: Some("Kant".to_string()),
            family_name_unicode: None,
            family_name_simplified: None,
            mononym_latex: None,
            mononym_unicode: None,
            mononym_simplified: None,
            shorthand_latex: None,
            shorthand_unicode: None,
            shorthand_simplified: None,
            famous_name_latex: None,
            famous_name_unicode: None,
            famous_name_simplified: None,
        };
        assert!(validate_create_author(&input).is_ok());
    }

    #[test]
    fn test_valid_author_with_mononym() {
        let input = CreateAuthor {
            author_key: "aristotle".to_string(),
            given_name_latex: None,
            given_name_unicode: None,
            given_name_simplified: None,
            family_name_latex: None,
            family_name_unicode: None,
            family_name_simplified: None,
            mononym_latex: Some("Aristotle".to_string()),
            mononym_unicode: None,
            mononym_simplified: None,
            shorthand_latex: None,
            shorthand_unicode: None,
            shorthand_simplified: None,
            famous_name_latex: None,
            famous_name_unicode: None,
            famous_name_simplified: None,
        };
        assert!(validate_create_author(&input).is_ok());
    }

    #[test]
    fn test_invalid_author_no_name() {
        let input = CreateAuthor {
            author_key: "nobody".to_string(),
            given_name_latex: Some("Just".to_string()),
            given_name_unicode: None,
            given_name_simplified: None,
            family_name_latex: None,
            family_name_unicode: None,
            family_name_simplified: None,
            mononym_latex: None,
            mononym_unicode: None,
            mononym_simplified: None,
            shorthand_latex: None,
            shorthand_unicode: None,
            shorthand_simplified: None,
            famous_name_latex: None,
            famous_name_unicode: None,
            famous_name_simplified: None,
        };
        assert!(validate_create_author(&input).is_err());
    }
}
