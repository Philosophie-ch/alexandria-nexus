//! Pure transform functions for Publisher DTOs.
//!
//! NO I/O — these are pure functions that transform DTOs to entities.

use chrono::Utc;

use crate::dto::{CreatePublisher, UpdatePublisher};
use crate::entities::Publisher;

/// Transform a CreatePublisher DTO to a Publisher entity.
/// Pure function — NO I/O.
pub fn create_publisher_transform(input: CreatePublisher) -> Publisher {
    let now = Utc::now();
    Publisher {
        id: 0, // Set by database
        publisher_key: input.publisher_key,
        name_latex: input.name_latex.unwrap_or_default(),
        name_unicode: input.name_unicode.unwrap_or_default(),
        name_simplified: input.name_simplified.unwrap_or_default(),
        default_address: input.default_address,
        created_at: now,
        updated_at: now,
    }
}

/// Transform an UpdatePublisher DTO by merging with an existing Publisher.
/// Pure function — NO I/O.
pub fn update_publisher_transform(input: UpdatePublisher, mut existing: Publisher) -> Publisher {
    if let Some(v) = input.publisher_key {
        existing.publisher_key = v;
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
    fn test_create_publisher_transform() {
        let input = CreatePublisher {
            publisher_key: "springer".to_string(),
            name_latex: Some("Springer".to_string()),
            name_unicode: Some("Springer".to_string()),
            name_simplified: Some("Springer".to_string()),
            default_address: Some("Berlin".to_string()),
        };

        let publisher = create_publisher_transform(input);
        assert_eq!(publisher.id, 0);
        assert_eq!(publisher.publisher_key, "springer");
        assert_eq!(publisher.name_unicode, "Springer");
        assert_eq!(publisher.default_address, Some("Berlin".to_string()));
    }

    #[test]
    fn test_update_publisher_transform() {
        let existing = Publisher {
            id: 42,
            publisher_key: "springer".to_string(),
            name_latex: "Springer".to_string(),
            name_unicode: "Springer".to_string(),
            name_simplified: "Springer".to_string(),
            default_address: Some("Berlin".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let input = UpdatePublisher {
            publisher_key: None,
            name_latex: None,
            name_unicode: Some("Springer Verlag".to_string()),
            name_simplified: None,
            default_address: None,
        };

        let updated = update_publisher_transform(input, existing);
        assert_eq!(updated.id, 42);
        assert_eq!(updated.publisher_key, "springer");
        assert_eq!(updated.name_latex, "Springer");
        assert_eq!(updated.name_unicode, "Springer Verlag");
        assert_eq!(updated.default_address, Some("Berlin".to_string()));
    }
}
