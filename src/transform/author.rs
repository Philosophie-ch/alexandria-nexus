//! Pure transform functions for Author DTOs.
//!
//! NO I/O — these are pure functions that transform DTOs to entities.

use chrono::Utc;

use crate::dto::{CreateAuthor, UpdateAuthor};
use crate::entities::Author;

/// Transform a CreateAuthor DTO to an Author entity.
/// Pure function — NO I/O.
pub fn create_author_transform(input: CreateAuthor) -> Author {
    let now = Utc::now();
    Author {
        id: 0, // Set by database
        author_key: input.author_key,
        given_name_latex: input.given_name_latex,
        given_name_unicode: input.given_name_unicode,
        given_name_simplified: input.given_name_simplified,
        family_name_latex: input.family_name_latex,
        family_name_unicode: input.family_name_unicode,
        family_name_simplified: input.family_name_simplified,
        mononym_latex: input.mononym_latex,
        mononym_unicode: input.mononym_unicode,
        mononym_simplified: input.mononym_simplified,
        shorthand_latex: input.shorthand_latex,
        shorthand_unicode: input.shorthand_unicode,
        shorthand_simplified: input.shorthand_simplified,
        famous_name_latex: input.famous_name_latex,
        famous_name_unicode: input.famous_name_unicode,
        famous_name_simplified: input.famous_name_simplified,
        created_at: now,
        updated_at: now,
    }
}

/// Transform an UpdateAuthor DTO by merging with an existing Author.
/// Pure function — NO I/O.
pub fn update_author_transform(input: UpdateAuthor, mut existing: Author) -> Author {
    if let Some(key) = input.author_key {
        existing.author_key = key;
    }
    if let Some(v) = input.given_name_latex {
        existing.given_name_latex = Some(v);
    }
    if let Some(v) = input.given_name_unicode {
        existing.given_name_unicode = Some(v);
    }
    if let Some(v) = input.given_name_simplified {
        existing.given_name_simplified = Some(v);
    }
    if let Some(v) = input.family_name_latex {
        existing.family_name_latex = Some(v);
    }
    if let Some(v) = input.family_name_unicode {
        existing.family_name_unicode = Some(v);
    }
    if let Some(v) = input.family_name_simplified {
        existing.family_name_simplified = Some(v);
    }
    if let Some(v) = input.mononym_latex {
        existing.mononym_latex = Some(v);
    }
    if let Some(v) = input.mononym_unicode {
        existing.mononym_unicode = Some(v);
    }
    if let Some(v) = input.mononym_simplified {
        existing.mononym_simplified = Some(v);
    }
    if let Some(v) = input.shorthand_latex {
        existing.shorthand_latex = Some(v);
    }
    if let Some(v) = input.shorthand_unicode {
        existing.shorthand_unicode = Some(v);
    }
    if let Some(v) = input.shorthand_simplified {
        existing.shorthand_simplified = Some(v);
    }
    if let Some(v) = input.famous_name_latex {
        existing.famous_name_latex = Some(v);
    }
    if let Some(v) = input.famous_name_unicode {
        existing.famous_name_unicode = Some(v);
    }
    if let Some(v) = input.famous_name_simplified {
        existing.famous_name_simplified = Some(v);
    }
    existing.updated_at = Utc::now();
    existing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_author_transform() {
        let input = CreateAuthor {
            author_key: "kant-immanuel".to_string(),
            given_name_latex: Some("Immanuel".to_string()),
            given_name_unicode: Some("Immanuel".to_string()),
            given_name_simplified: Some("Immanuel".to_string()),
            family_name_latex: Some("Kant".to_string()),
            family_name_unicode: Some("Kant".to_string()),
            family_name_simplified: Some("Kant".to_string()),
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

        let author = create_author_transform(input);
        assert_eq!(author.id, 0);
        assert_eq!(author.author_key, "kant-immanuel");
        assert_eq!(author.given_name_unicode, Some("Immanuel".to_string()));
        assert_eq!(author.family_name_latex, Some("Kant".to_string()));
        assert!(author.mononym_latex.is_none());
    }

    #[test]
    fn test_update_author_transform() {
        let existing = Author {
            id: 42,
            author_key: "kant-immanuel".to_string(),
            given_name_latex: Some("Immanuel".to_string()),
            given_name_unicode: Some("Immanuel".to_string()),
            given_name_simplified: Some("Immanuel".to_string()),
            family_name_latex: Some("Kant".to_string()),
            family_name_unicode: Some("Kant".to_string()),
            family_name_simplified: Some("Kant".to_string()),
            mononym_latex: None,
            mononym_unicode: None,
            mononym_simplified: None,
            shorthand_latex: None,
            shorthand_unicode: None,
            shorthand_simplified: None,
            famous_name_latex: None,
            famous_name_unicode: None,
            famous_name_simplified: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let input = UpdateAuthor {
            author_key: None,
            given_name_latex: None,
            given_name_unicode: Some("Emmanuel".to_string()), // Change this
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

        let updated = update_author_transform(input, existing);
        assert_eq!(updated.id, 42);
        assert_eq!(updated.author_key, "kant-immanuel"); // Kept
        assert_eq!(updated.given_name_latex, Some("Immanuel".to_string())); // Kept
        assert_eq!(updated.given_name_unicode, Some("Emmanuel".to_string())); // Changed
        assert_eq!(updated.family_name_unicode, Some("Kant".to_string())); // Kept
    }
}
