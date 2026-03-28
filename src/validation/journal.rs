//! Pure validation functions for Journal DTOs.
//!
//! NO I/O — these are pure functions that validate input data.

use hexforge::ValidationError;

use crate::entities::{CreateJournal, UpdateJournal};

/// Validate a CreateJournal request.
/// Pure function — NO I/O.
///
/// A journal must have a non-empty key and at least one name variant.
pub fn validate_create_journal(input: &CreateJournal) -> Result<(), ValidationError> {
    if input.journal_key.trim().is_empty() {
        return Err(ValidationError::required("journal_key"));
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

    // Validate ISSN format if provided (should be XXXX-XXXX)
    if let Some(ref issn) = input.issn_print
        && !issn.is_empty()
        && !is_valid_issn(issn)
    {
        return Err(ValidationError::invalid_value(
            "issn_print",
            "ISSN must be in format XXXX-XXXX",
        ));
    }

    if let Some(ref issn) = input.issn_electronic
        && !issn.is_empty()
        && !is_valid_issn(issn)
    {
        return Err(ValidationError::invalid_value(
            "issn_electronic",
            "ISSN must be in format XXXX-XXXX",
        ));
    }

    Ok(())
}

/// Validate an UpdateJournal request.
/// Pure function — NO I/O.
pub fn validate_update_journal(input: &UpdateJournal) -> Result<(), ValidationError> {
    if let Some(ref key) = input.journal_key
        && key.trim().is_empty()
    {
        return Err(ValidationError::invalid_value(
            "journal_key",
            "journal_key cannot be empty",
        ));
    }

    // Validate ISSN format if provided
    if let Some(ref issn) = input.issn_print
        && !issn.is_empty()
        && !is_valid_issn(issn)
    {
        return Err(ValidationError::invalid_value(
            "issn_print",
            "ISSN must be in format XXXX-XXXX",
        ));
    }

    if let Some(ref issn) = input.issn_electronic
        && !issn.is_empty()
        && !is_valid_issn(issn)
    {
        return Err(ValidationError::invalid_value(
            "issn_electronic",
            "ISSN must be in format XXXX-XXXX",
        ));
    }

    Ok(())
}

/// Check if a string is a valid ISSN format (XXXX-XXXX where X is a digit or X for check digit).
fn is_valid_issn(issn: &str) -> bool {
    if issn.len() != 9 {
        return false;
    }

    let chars: Vec<char> = issn.chars().collect();

    // First 4 must be digits
    if !chars[0..4].iter().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // 5th must be hyphen
    if chars[4] != '-' {
        return false;
    }

    // Next 3 must be digits
    if !chars[5..8].iter().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // Last can be digit or X
    chars[8].is_ascii_digit() || chars[8] == 'X' || chars[8] == 'x'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_journal() {
        let input = CreateJournal {
            journal_key: "nature".to_string(),
            name_latex: Some("Nature".to_string()),
            name_unicode: None,
            name_simplified: None,
            issn_print: Some("0028-0836".to_string()),
            issn_electronic: None,
        };
        assert!(validate_create_journal(&input).is_ok());
    }

    #[test]
    fn test_journal_without_key() {
        let input = CreateJournal {
            journal_key: String::new(),
            name_latex: Some("Nature".to_string()),
            name_unicode: None,
            name_simplified: None,
            issn_print: None,
            issn_electronic: None,
        };
        assert!(validate_create_journal(&input).is_err());
    }

    #[test]
    fn test_journal_without_name() {
        let input = CreateJournal {
            journal_key: "noname".to_string(),
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
            issn_print: None,
            issn_electronic: None,
        };
        assert!(validate_create_journal(&input).is_err());
    }

    #[test]
    fn test_valid_issn() {
        assert!(is_valid_issn("0028-0836"));
        assert!(is_valid_issn("1234-567X"));
        assert!(is_valid_issn("1234-567x"));
    }

    #[test]
    fn test_invalid_issn() {
        assert!(!is_valid_issn("0028-083")); // too short
        assert!(!is_valid_issn("00280836")); // no hyphen
        assert!(!is_valid_issn("0028-08366")); // too long
        assert!(!is_valid_issn("XXXX-XXXX")); // letters in wrong place
    }
}
