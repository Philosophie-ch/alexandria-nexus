//! Pure transform functions for Series DTOs.
//!
//! NO I/O — these are pure functions that transform DTOs to entities.

use chrono::Utc;

use crate::dto::{CreateSeries, UpdateSeries};
use crate::entities::Series;

/// Transform a CreateSeries DTO to a Series entity.
/// Pure function — NO I/O.
pub fn create_series_transform(input: CreateSeries) -> Series {
    let now = Utc::now();
    Series {
        id: 0, // Set by database
        series_key: input.series_key,
        name_latex: input.name_latex.unwrap_or_default(),
        name_unicode: input.name_unicode.unwrap_or_default(),
        name_simplified: input.name_simplified.unwrap_or_default(),
        created_at: now,
        updated_at: now,
    }
}

/// Transform an UpdateSeries DTO by merging with an existing Series.
/// Pure function — NO I/O.
pub fn update_series_transform(input: UpdateSeries, mut existing: Series) -> Series {
    if let Some(v) = input.series_key {
        existing.series_key = v;
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
    existing.updated_at = Utc::now();
    existing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_series_transform() {
        let input = CreateSeries {
            series_key: "synthese_library".to_string(),
            name_latex: Some("Synthese Library".to_string()),
            name_unicode: Some("Synthese Library".to_string()),
            name_simplified: Some("Synthese Library".to_string()),
        };

        let series = create_series_transform(input);
        assert_eq!(series.id, 0);
        assert_eq!(series.series_key, "synthese_library");
        assert_eq!(series.name_unicode, "Synthese Library");
    }

    #[test]
    fn test_update_series_transform() {
        let existing = Series {
            id: 42,
            series_key: "synthese_library".to_string(),
            name_latex: "Synthese Library".to_string(),
            name_unicode: "Synthese Library".to_string(),
            name_simplified: "Synthese Library".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let input = UpdateSeries {
            series_key: None,
            name_latex: None,
            name_unicode: Some("Synthese Library Series".to_string()),
            name_simplified: None,
        };

        let updated = update_series_transform(input, existing);
        assert_eq!(updated.id, 42);
        assert_eq!(updated.series_key, "synthese_library");
        assert_eq!(updated.name_latex, "Synthese Library");
        assert_eq!(updated.name_unicode, "Synthese Library Series");
    }
}
