//! Pure transform functions for Journal DTOs.
//!
//! NO I/O — these are pure functions that transform DTOs to entities.

use chrono::Utc;

use crate::dto::{CreateJournal, UpdateJournal};
use crate::entities::Journal;

/// Transform a CreateJournal DTO to a Journal entity.
/// Pure function — NO I/O.
pub fn create_journal_transform(input: CreateJournal) -> Journal {
    let now = Utc::now();
    Journal {
        id: 0, // Set by database
        journal_key: input.journal_key,
        name_latex: input.name_latex.unwrap_or_default(),
        name_unicode: input.name_unicode.unwrap_or_default(),
        name_simplified: input.name_simplified.unwrap_or_default(),
        issn_print: input.issn_print,
        issn_electronic: input.issn_electronic,
        created_at: now,
        updated_at: now,
    }
}

/// Transform an UpdateJournal DTO by merging with an existing Journal.
/// Pure function — NO I/O.
pub fn update_journal_transform(input: UpdateJournal, mut existing: Journal) -> Journal {
    if let Some(v) = input.journal_key {
        existing.journal_key = v;
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
    if let Some(v) = input.issn_print {
        existing.issn_print = Some(v);
    }
    if let Some(v) = input.issn_electronic {
        existing.issn_electronic = Some(v);
    }
    existing.updated_at = Utc::now();
    existing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_journal_transform() {
        let input = CreateJournal {
            journal_key: "nature".to_string(),
            name_latex: Some("Nature".to_string()),
            name_unicode: Some("Nature".to_string()),
            name_simplified: Some("Nature".to_string()),
            issn_print: Some("0028-0836".to_string()),
            issn_electronic: None,
        };

        let journal = create_journal_transform(input);
        assert_eq!(journal.id, 0);
        assert_eq!(journal.journal_key, "nature");
        assert_eq!(journal.name_unicode, "Nature");
        assert_eq!(journal.issn_print, Some("0028-0836".to_string()));
        assert!(journal.issn_electronic.is_none());
    }

    #[test]
    fn test_update_journal_transform() {
        let existing = Journal {
            id: 42,
            journal_key: "nature".to_string(),
            name_latex: "Nature".to_string(),
            name_unicode: "Nature".to_string(),
            name_simplified: "Nature".to_string(),
            issn_print: Some("0028-0836".to_string()),
            issn_electronic: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let input = UpdateJournal {
            journal_key: None,
            name_latex: None,
            name_unicode: None,
            name_simplified: None,
            issn_print: None,
            issn_electronic: Some("1476-4687".to_string()),
        };

        let updated = update_journal_transform(input, existing);
        assert_eq!(updated.id, 42);
        assert_eq!(updated.journal_key, "nature");
        assert_eq!(updated.name_unicode, "Nature");
        assert_eq!(updated.issn_print, Some("0028-0836".to_string()));
        assert_eq!(updated.issn_electronic, Some("1476-4687".to_string()));
    }
}
