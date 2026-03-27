//! Pure transform functions for School DTOs.
//!
//! NO I/O — these are pure functions that transform DTOs to entities.

use chrono::Utc;

use crate::dto::{CreateSchool, UpdateSchool};
use crate::entities::School;

/// Transform a CreateSchool DTO to a School entity.
/// Pure function — NO I/O.
pub fn create_school_transform(input: CreateSchool) -> School {
    let now = Utc::now();
    School {
        id: 0, // Set by database
        school_key: input.school_key,
        name_latex: input.name_latex.unwrap_or_default(),
        name_unicode: input.name_unicode.unwrap_or_default(),
        name_simplified: input.name_simplified.unwrap_or_default(),
        default_address: input.default_address,
        created_at: now,
        updated_at: now,
    }
}

/// Transform an UpdateSchool DTO by merging with an existing School.
/// Pure function — NO I/O.
pub fn update_school_transform(input: UpdateSchool, mut existing: School) -> School {
    if let Some(v) = input.school_key {
        existing.school_key = v;
    }
    if let Some(v) = input.name_latex {
        existing.name_latex = v;
    }
    if let Some(v) = input.name_unicode {
        existing.name_unicode = v;
    }
    if let Some(v) = input.name_simplified {
        existing.name_simplified = v;
    }
    if let Some(v) = input.default_address {
        existing.default_address = Some(v);
    }
    existing.updated_at = Utc::now();
    existing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_school_transform() {
        let input = CreateSchool {
            school_key: "mit_linguistics".to_string(),
            name_latex: Some("MIT Linguistics".to_string()),
            name_unicode: Some("MIT Linguistics".to_string()),
            name_simplified: Some("MIT Linguistics".to_string()),
            default_address: Some("Cambridge, MA".to_string()),
        };

        let school = create_school_transform(input);
        assert_eq!(school.id, 0);
        assert_eq!(school.school_key, "mit_linguistics");
        assert_eq!(school.name_unicode, "MIT Linguistics");
        assert_eq!(school.default_address, Some("Cambridge, MA".to_string()));
    }

    #[test]
    fn test_update_school_transform() {
        let existing = School {
            id: 42,
            school_key: "mit_linguistics".to_string(),
            name_latex: "MIT Linguistics".to_string(),
            name_unicode: "MIT Linguistics".to_string(),
            name_simplified: "MIT Linguistics".to_string(),
            default_address: Some("Cambridge, MA".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let input = UpdateSchool {
            school_key: None,
            name_latex: None,
            name_unicode: Some("MIT Dept of Linguistics".to_string()),
            name_simplified: None,
            default_address: None,
        };

        let updated = update_school_transform(input, existing);
        assert_eq!(updated.id, 42);
        assert_eq!(updated.school_key, "mit_linguistics");
        assert_eq!(updated.name_latex, "MIT Linguistics");
        assert_eq!(updated.name_unicode, "MIT Dept of Linguistics");
        assert_eq!(updated.default_address, Some("Cambridge, MA".to_string()));
    }
}
