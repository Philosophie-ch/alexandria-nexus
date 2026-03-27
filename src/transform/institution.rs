//! Pure transform functions for Institution DTOs.
//!
//! NO I/O — these are pure functions that transform DTOs to entities.

use chrono::Utc;

use crate::dto::{CreateInstitution, UpdateInstitution};
use crate::entities::Institution;

/// Transform a CreateInstitution DTO to an Institution entity.
/// Pure function — NO I/O.
pub fn create_institution_transform(input: CreateInstitution) -> Institution {
    let now = Utc::now();
    Institution {
        id: 0, // Set by database
        institution_key: input.institution_key,
        name_latex: input.name_latex.unwrap_or_default(),
        name_unicode: input.name_unicode.unwrap_or_default(),
        name_simplified: input.name_simplified.unwrap_or_default(),
        default_address: input.default_address,
        created_at: now,
        updated_at: now,
    }
}

/// Transform an UpdateInstitution DTO by merging with an existing Institution.
/// Pure function — NO I/O.
pub fn update_institution_transform(
    input: UpdateInstitution,
    mut existing: Institution,
) -> Institution {
    if let Some(v) = input.institution_key {
        existing.institution_key = v;
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
    fn test_create_institution_transform() {
        let input = CreateInstitution {
            institution_key: "csli_stanford".to_string(),
            name_latex: Some("CSLI".to_string()),
            name_unicode: Some("CSLI".to_string()),
            name_simplified: Some("CSLI".to_string()),
            default_address: Some("Stanford, CA".to_string()),
        };

        let institution = create_institution_transform(input);
        assert_eq!(institution.id, 0);
        assert_eq!(institution.institution_key, "csli_stanford");
        assert_eq!(institution.name_unicode, "CSLI");
        assert_eq!(
            institution.default_address,
            Some("Stanford, CA".to_string())
        );
    }

    #[test]
    fn test_update_institution_transform() {
        let existing = Institution {
            id: 42,
            institution_key: "csli_stanford".to_string(),
            name_latex: "CSLI".to_string(),
            name_unicode: "CSLI".to_string(),
            name_simplified: "CSLI".to_string(),
            default_address: Some("Stanford, CA".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let input = UpdateInstitution {
            institution_key: None,
            name_latex: None,
            name_unicode: Some("CSLI Publications".to_string()),
            name_simplified: None,
            default_address: None,
        };

        let updated = update_institution_transform(input, existing);
        assert_eq!(updated.id, 42);
        assert_eq!(updated.institution_key, "csli_stanford");
        assert_eq!(updated.name_latex, "CSLI");
        assert_eq!(updated.name_unicode, "CSLI Publications");
        assert_eq!(updated.default_address, Some("Stanford, CA".to_string()));
    }
}
