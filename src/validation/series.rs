//! Pure validation functions for Series DTOs.
//!
//! NO I/O — these are pure functions that validate input data.

use hexforge::ValidationError;

use crate::entities::{CreateSeries, UpdateSeries};

/// Validate a CreateSeries request.
/// Pure function — NO I/O.
///
/// A series must have a non-empty key and at least one name variant.
pub fn validate_create_series(input: &CreateSeries) -> Result<(), ValidationError> {
    if input.series_key.trim().is_empty() {
        return Err(ValidationError::required("series_key"));
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

/// Validate an UpdateSeries request.
/// Pure function — NO I/O.
pub fn validate_update_series(input: &UpdateSeries) -> Result<(), ValidationError> {
    if let Some(ref key) = input.series_key
        && key.trim().is_empty()
    {
        return Err(ValidationError::invalid_value(
            "series_key",
            "series_key cannot be empty",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_series() {
        let input = CreateSeries {
            series_key: "synthese_library".to_string(),
            name_latex: Some("Synthese Library".to_string()),
            name_unicode: None,
            name_simplified: None,
        };
        assert!(validate_create_series(&input).is_ok());
    }

    #[test]
    fn test_series_without_key() {
        let input = CreateSeries {
            series_key: String::new(),
            name_latex: Some("Synthese Library".to_string()),
            name_unicode: None,
            name_simplified: None,
        };
        assert!(validate_create_series(&input).is_err());
    }

    #[test]
    fn test_series_without_name() {
        let input = CreateSeries {
            series_key: "synthese_library".to_string(),
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
        };
        assert!(validate_create_series(&input).is_err());
    }

    #[test]
    fn test_update_empty_key() {
        let input = UpdateSeries {
            series_key: Some(String::new()),
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
        };
        assert!(validate_update_series(&input).is_err());
    }

    #[test]
    fn test_update_valid() {
        let input = UpdateSeries {
            series_key: None,
            name_latex: Some("New Name".to_string()),
            name_unicode: None,
            name_simplified: None,
        };
        assert!(validate_update_series(&input).is_ok());
    }
}
