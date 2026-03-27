//! Pure transform functions for Keyword DTOs.
//!
//! NO I/O — these are pure functions that transform DTOs to entities.
//! Keywords are flat — no parent hierarchy.

use chrono::Utc;

use crate::dto::{CreateKeyword, UpdateKeyword};
use crate::entities::Keyword;

/// Transform a CreateKeyword DTO to a Keyword entity.
/// Pure function — NO I/O.
pub fn create_keyword_transform(input: CreateKeyword) -> Keyword {
    Keyword {
        id: 0, // Set by database
        name: input.name,
        level: input.level,
        created_at: Utc::now(),
    }
}

/// Transform an UpdateKeyword DTO by merging with an existing Keyword.
/// Pure function — NO I/O.
pub fn update_keyword_transform(input: UpdateKeyword, mut existing: Keyword) -> Keyword {
    if let Some(v) = input.name {
        existing.name = v;
    }
    if let Some(v) = input.level {
        existing.level = v;
    }
    existing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_keyword_transform() {
        let input = CreateKeyword {
            name: "Philosophy".to_string(),
            level: 1,
        };

        let keyword = create_keyword_transform(input);
        assert_eq!(keyword.id, 0);
        assert_eq!(keyword.name, "Philosophy");
        assert_eq!(keyword.level, 1);
    }

    #[test]
    fn test_update_keyword_transform() {
        let existing = Keyword {
            id: 42,
            name: "Philosophy".to_string(),
            level: 1,
            created_at: Utc::now(),
        };

        let input = UpdateKeyword {
            name: Some("Western Philosophy".to_string()),
            level: None,
        };

        let updated = update_keyword_transform(input, existing);
        assert_eq!(updated.id, 42);
        assert_eq!(updated.name, "Western Philosophy");
        assert_eq!(updated.level, 1);
    }
}
