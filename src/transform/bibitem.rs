//! Pure transform functions for BibItem DTOs.
//!
//! NO I/O — these are pure functions that transform DTOs to entities.

use chrono::Utc;

use crate::dto::{CreateBibItem, UpdateBibItem};
use crate::entities::BibItem;

/// Transform a CreateBibItem DTO to a BibItem entity.
/// Pure function — NO I/O.
pub fn create_bibitem_transform(input: CreateBibItem) -> BibItem {
    let now = Utc::now();
    BibItem {
        id: 0, // Set by database
        bibkey: input.bibkey,
        entry_type: input.entry_type,

        // Dates
        date_year: input.date_year,
        date_year_2_hyphen: input.date_year_2_hyphen,
        date_year_2_slash: input.date_year_2_slash,
        date_month: input.date_month,
        date_day: input.date_day,
        date_is_no_date: input.date_is_no_date.unwrap_or(false),
        pubstate: input.pubstate,

        // Title
        title_latex: input.title_latex,
        title_unicode: input.title_unicode,
        title_simplified: input.title_simplified,

        // Booktitle
        booktitle_latex: input.booktitle_latex,
        booktitle_unicode: input.booktitle_unicode,
        booktitle_simplified: input.booktitle_simplified,

        // Publication info
        journal_id: input.journal_id,
        publisher_id: input.publisher_id,
        address: input.address,
        volume: input.volume,
        number: input.number,
        pages: input.pages,
        eid: input.eid,
        series_id: input.series_id,
        edition: input.edition,

        // Institutional
        institution_id: input.institution_id,
        school_id: input.school_id,
        type_field: input.type_field,

        // Identifiers
        doi: input.doi,
        url: input.url,
        eprint: input.eprint,
        urn: input.urn,

        // References
        crossref_id: input.crossref_id,

        // Issue/Notes
        issuetitle_latex: input.issuetitle_latex,
        issuetitle_unicode: input.issuetitle_unicode,
        note_latex: input.note_latex,
        note_unicode: input.note_unicode,
        extra_note_latex: input.extra_note_latex,
        extra_note_unicode: input.extra_note_unicode,

        // Metadata
        langid: input.langid,
        is_translation: input.is_translation.unwrap_or(false),
        epoch: input.epoch,
        options: input.options,
        shorthand: input.shorthand,

        // Internal tracking
        person_id: input.person_id,
        has_fulltext: input.has_fulltext.unwrap_or(false),
        fulltext_path: input.fulltext_path,

        created_at: now,
        updated_at: now,
    }
}

/// Transform an UpdateBibItem DTO by merging with an existing BibItem.
/// Pure function — NO I/O.
pub fn update_bibitem_transform(input: UpdateBibItem, mut existing: BibItem) -> BibItem {
    // Identity
    if let Some(v) = input.bibkey {
        existing.bibkey = v;
    }
    if let Some(v) = input.entry_type {
        existing.entry_type = v;
    }

    // Dates
    if let Some(v) = input.date_year {
        existing.date_year = Some(v);
    }
    if let Some(v) = input.date_year_2_hyphen {
        existing.date_year_2_hyphen = Some(v);
    }
    if let Some(v) = input.date_year_2_slash {
        existing.date_year_2_slash = Some(v);
    }
    if let Some(v) = input.date_month {
        existing.date_month = Some(v);
    }
    if let Some(v) = input.date_day {
        existing.date_day = Some(v);
    }
    if let Some(v) = input.date_is_no_date {
        existing.date_is_no_date = v;
    }
    if let Some(v) = input.pubstate {
        existing.pubstate = Some(v);
    }

    // Title
    if let Some(v) = input.title_latex {
        existing.title_latex = v;
    }
    if let Some(v) = input.title_unicode {
        existing.title_unicode = v;
    }
    if let Some(v) = input.title_simplified {
        existing.title_simplified = v;
    }

    // Booktitle
    if let Some(v) = input.booktitle_latex {
        existing.booktitle_latex = Some(v);
    }
    if let Some(v) = input.booktitle_unicode {
        existing.booktitle_unicode = Some(v);
    }
    if let Some(v) = input.booktitle_simplified {
        existing.booktitle_simplified = Some(v);
    }

    // Publication info
    if let Some(v) = input.journal_id {
        existing.journal_id = Some(v);
    }
    if let Some(v) = input.publisher_id {
        existing.publisher_id = Some(v);
    }
    if let Some(v) = input.address {
        existing.address = Some(v);
    }
    if let Some(v) = input.volume {
        existing.volume = Some(v);
    }
    if let Some(v) = input.number {
        existing.number = Some(v);
    }
    if let Some(v) = input.pages {
        existing.pages = Some(v);
    }
    if let Some(v) = input.eid {
        existing.eid = Some(v);
    }
    if let Some(v) = input.series_id {
        existing.series_id = Some(v);
    }
    if let Some(v) = input.edition {
        existing.edition = Some(v);
    }

    // Institutional
    if let Some(v) = input.institution_id {
        existing.institution_id = Some(v);
    }
    if let Some(v) = input.school_id {
        existing.school_id = Some(v);
    }
    if let Some(v) = input.type_field {
        existing.type_field = Some(v);
    }

    // Identifiers
    if let Some(v) = input.doi {
        existing.doi = Some(v);
    }
    if let Some(v) = input.url {
        existing.url = Some(v);
    }
    if let Some(v) = input.eprint {
        existing.eprint = Some(v);
    }
    if let Some(v) = input.urn {
        existing.urn = Some(v);
    }

    // References
    if let Some(v) = input.crossref_id {
        existing.crossref_id = Some(v);
    }

    // Issue/Notes
    if let Some(v) = input.issuetitle_latex {
        existing.issuetitle_latex = Some(v);
    }
    if let Some(v) = input.issuetitle_unicode {
        existing.issuetitle_unicode = Some(v);
    }
    if let Some(v) = input.note_latex {
        existing.note_latex = Some(v);
    }
    if let Some(v) = input.note_unicode {
        existing.note_unicode = Some(v);
    }
    if let Some(v) = input.extra_note_latex {
        existing.extra_note_latex = Some(v);
    }
    if let Some(v) = input.extra_note_unicode {
        existing.extra_note_unicode = Some(v);
    }

    // Metadata
    if let Some(v) = input.langid {
        existing.langid = Some(v);
    }
    if let Some(v) = input.is_translation {
        existing.is_translation = v;
    }
    if let Some(v) = input.epoch {
        existing.epoch = Some(v);
    }
    if let Some(v) = input.options {
        existing.options = Some(v);
    }
    if let Some(v) = input.shorthand {
        existing.shorthand = Some(v);
    }

    // Internal tracking
    if let Some(v) = input.person_id {
        existing.person_id = Some(v);
    }
    if let Some(v) = input.has_fulltext {
        existing.has_fulltext = v;
    }
    if let Some(v) = input.fulltext_path {
        existing.fulltext_path = Some(v);
    }

    existing.updated_at = Utc::now();
    existing
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
            date_is_no_date: None,
            pubstate: None,
            title_latex: "Test Title".to_string(),
            title_unicode: "Test Title".to_string(),
            title_simplified: "Test Title".to_string(),
            booktitle_latex: None,
            booktitle_unicode: None,
            booktitle_simplified: None,
            journal_id: None,
            publisher_id: None,
            address: None,
            volume: None,
            number: None,
            pages: None,
            eid: None,
            series_id: None,
            edition: None,
            institution_id: None,
            school_id: None,
            type_field: None,
            doi: None,
            url: None,
            eprint: None,
            urn: None,
            crossref_id: None,
            issuetitle_latex: None,
            issuetitle_unicode: None,
            note_latex: None,
            note_unicode: None,
            extra_note_latex: None,
            extra_note_unicode: None,
            langid: None,
            is_translation: None,
            epoch: None,
            options: None,
            shorthand: None,
            person_id: None,
            has_fulltext: None,
            fulltext_path: None,
        }
    }

    #[test]
    fn test_create_bibitem_transform() {
        let input = minimal_create_input();
        let item = create_bibitem_transform(input);

        assert_eq!(item.id, 0);
        assert_eq!(item.bibkey, "Smith:2024");
        assert_eq!(item.entry_type, EntryType::Article);
        assert_eq!(item.title_latex, "Test Title");
        assert_eq!(item.date_year, Some(2024));
        assert!(!item.date_is_no_date);
        assert!(!item.is_translation);
        assert!(!item.has_fulltext);
    }

    #[test]
    fn test_update_bibitem_transform() {
        let existing = create_bibitem_transform(minimal_create_input());

        let input = UpdateBibItem {
            bibkey: None,
            entry_type: Some(EntryType::Book),
            date_year: None,
            date_year_2_hyphen: None,
            date_year_2_slash: None,
            date_month: None,
            date_day: None,
            date_is_no_date: None,
            pubstate: None,
            title_latex: Some("Updated Title".to_string()),
            title_unicode: None,
            title_simplified: None,
            booktitle_latex: None,
            booktitle_unicode: None,
            booktitle_simplified: None,
            journal_id: None,
            publisher_id: None,
            address: None,
            volume: Some("42".to_string()),
            number: None,
            pages: None,
            eid: None,
            series_id: None,
            edition: None,
            institution_id: None,
            school_id: None,
            type_field: None,
            doi: None,
            url: None,
            eprint: None,
            urn: None,
            crossref_id: None,
            issuetitle_latex: None,
            issuetitle_unicode: None,
            note_latex: None,
            note_unicode: None,
            extra_note_latex: None,
            extra_note_unicode: None,
            langid: None,
            is_translation: None,
            epoch: None,
            options: None,
            shorthand: None,
            person_id: None,
            has_fulltext: None,
            fulltext_path: None,
        };

        let updated = update_bibitem_transform(input, existing);
        assert_eq!(updated.bibkey, "Smith:2024"); // Kept
        assert_eq!(updated.entry_type, EntryType::Book); // Changed
        assert_eq!(updated.title_latex, "Updated Title"); // Changed
        assert_eq!(updated.title_unicode, "Test Title"); // Kept
        assert_eq!(updated.volume, Some("42".to_string())); // Changed
    }
}
