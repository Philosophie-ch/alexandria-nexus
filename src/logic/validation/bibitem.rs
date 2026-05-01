//! Pure validation functions for BibItem DTOs.
//!
//! NO I/O — these are pure functions that validate input data.

use hexforge::ValidationError;

use crate::domain::{CreateBibItem, UpdateBibItem};

/// Validate a CreateBibItem request.
/// Pure function — NO I/O.
pub fn validate_create_bibitem(input: &CreateBibItem) -> Result<(), ValidationError> {
    // Bibkey: required, non-empty
    if input.bibkey.trim().is_empty() {
        return Err(ValidationError::required("bibkey"));
    }

    // Title: latex is required and must be non-empty
    if input.title_latex.trim().is_empty() {
        return Err(ValidationError::required("title_latex"));
    }

    // Date validation: if year_2_* set, year must be set
    if (input.date_year_2_hyphen.is_some() || input.date_year_2_slash.is_some())
        && input.date_year.is_none()
    {
        return Err(ValidationError::invalid_value(
            "date_year",
            "year required when year_2_hyphen or year_2_slash is set",
        ));
    }

    // Date validation: can't have both hyphen and slash year ranges
    if input.date_year_2_hyphen.is_some() && input.date_year_2_slash.is_some() {
        return Err(ValidationError::invalid_value(
            "date_year_2_hyphen",
            "cannot have both hyphen and slash year ranges",
        ));
    }

    // Validate month range if provided
    if let Some(month) = input.date_month
        && !(1..=12).contains(&month)
    {
        return Err(ValidationError::invalid_value(
            "date_month",
            format!("month must be between 1 and 12, got {month}"),
        ));
    }

    // Validate day range if provided
    if let Some(day) = input.date_day
        && !(1..=31).contains(&day)
    {
        return Err(ValidationError::invalid_value(
            "date_day",
            format!("day must be between 1 and 31, got {day}"),
        ));
    }

    Ok(())
}

/// Validate an UpdateBibItem request.
/// Pure function — NO I/O.
pub fn validate_update_bibitem(input: &UpdateBibItem) -> Result<(), ValidationError> {
    // If bibkey is being updated, it must not be empty
    if let Some(ref bibkey) = input.bibkey
        && bibkey.trim().is_empty()
    {
        return Err(ValidationError::invalid_value(
            "bibkey",
            "bibkey cannot be empty",
        ));
    }

    // Validate month range if provided
    if let Some(month) = input.date_month
        && !(1..=12).contains(&month)
    {
        return Err(ValidationError::invalid_value(
            "date_month",
            format!("month must be between 1 and 12, got {month}"),
        ));
    }

    // Validate day range if provided
    if let Some(day) = input.date_day
        && !(1..=31).contains(&day)
    {
        return Err(ValidationError::invalid_value(
            "date_day",
            format!("day must be between 1 and 31, got {day}"),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EntryType;

    fn minimal_create_input() -> CreateBibItem {
        CreateBibItem {
            bibkey: "Smith:2024".to_string(),
            entry_type: EntryType::Article,
            date_year: Some(2024),
            date_year_2_hyphen: None,
            date_year_2_slash: None,
            date_month: None,
            date_day: None,
            date_is_no_date: false,
            pubstate: None,
            title_latex: "Test Title".to_string(),
            title_unicode: None,
            booktitle_latex: None,
            booktitle_unicode: None,
            journal_key: None,
            publisher_key: None,
            address: None,
            volume: None,
            number: None,
            pages: None,
            eid: None,
            series_key: None,
            edition: None,
            institution_key: None,
            school_key: None,
            type_field: None,
            doi: None,
            url: None,
            eprint: None,
            urn: None,
            crossref: None,
            issuetitle_latex: None,
            issuetitle_unicode: None,
            note_latex: None,
            note_unicode: None,
            extra_note_latex: None,
            extra_note_unicode: None,
            langid: None,
            is_translation: false,
            epoch: None,
            options: None,
            shorthand: None,
            person_key: None,
            has_fulltext: false,
            fulltext_path: None,
            license: None,
        }
    }

    #[test]
    fn test_valid_create_bibitem() {
        let input = minimal_create_input();
        assert!(validate_create_bibitem(&input).is_ok());
    }

    #[test]
    fn test_missing_bibkey() {
        let mut input = minimal_create_input();
        input.bibkey = String::new();
        assert!(validate_create_bibitem(&input).is_err());
    }

    #[test]
    fn test_missing_title() {
        let mut input = minimal_create_input();
        input.title_latex = String::new();
        assert!(validate_create_bibitem(&input).is_err());
    }

    #[test]
    fn test_invalid_month() {
        let mut input = minimal_create_input();
        input.date_month = Some(13);
        assert!(validate_create_bibitem(&input).is_err());
    }

    #[test]
    fn test_invalid_day() {
        let mut input = minimal_create_input();
        input.date_day = Some(32);
        assert!(validate_create_bibitem(&input).is_err());
    }

    #[test]
    fn test_both_year_ranges() {
        let mut input = minimal_create_input();
        input.date_year_2_hyphen = Some(2025);
        input.date_year_2_slash = Some(2025);
        assert!(validate_create_bibitem(&input).is_err());
    }

    #[test]
    fn test_year_range_without_year() {
        let mut input = minimal_create_input();
        input.date_year = None;
        input.date_year_2_hyphen = Some(2025);
        assert!(validate_create_bibitem(&input).is_err());
    }
}
