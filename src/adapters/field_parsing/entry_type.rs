use std::str::FromStr;

use crate::domain::EntryType;

/// Parse an entry type string from ODS/CSV format.
///
/// Strips whitespace, `@`, `{`, `}`, lowercases, and maps to [`EntryType`].
/// Returns [`EntryType::Unknown`] for unrecognized values.
pub fn parse_entry_type(text: &str) -> EntryType {
    let cleaned: String = text
        .chars()
        .filter(|c| !matches!(c, '@' | '{' | '}'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("")
        .to_lowercase();

    if cleaned.is_empty() {
        return EntryType::Unknown;
    }

    EntryType::from_str(&cleaned).unwrap_or(EntryType::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bibtex_format() {
        assert_eq!(parse_entry_type("@book{"), EntryType::Book);
        assert_eq!(parse_entry_type("@article{"), EntryType::Article);
        assert_eq!(parse_entry_type("@InCollection{"), EntryType::Incollection);
    }

    #[test]
    fn plain_lowercase() {
        assert_eq!(parse_entry_type("article"), EntryType::Article);
        assert_eq!(parse_entry_type("misc"), EntryType::Misc);
        assert_eq!(parse_entry_type("phdthesis"), EntryType::Phdthesis);
    }

    #[test]
    fn uppercase() {
        assert_eq!(parse_entry_type("ARTICLE"), EntryType::Article);
        assert_eq!(parse_entry_type("BOOK"), EntryType::Book);
    }

    #[test]
    fn with_whitespace() {
        assert_eq!(parse_entry_type(" @Book { "), EntryType::Book);
        assert_eq!(parse_entry_type("  article  "), EntryType::Article);
    }

    #[test]
    fn empty_and_unknown() {
        assert_eq!(parse_entry_type(""), EntryType::Unknown);
        assert_eq!(parse_entry_type("UNKNOWN"), EntryType::Unknown);
        assert_eq!(parse_entry_type("garbage"), EntryType::Unknown);
        assert_eq!(parse_entry_type("review"), EntryType::Unknown);
    }

    #[test]
    fn all_valid_types() {
        assert_eq!(parse_entry_type("article"), EntryType::Article);
        assert_eq!(parse_entry_type("book"), EntryType::Book);
        assert_eq!(parse_entry_type("incollection"), EntryType::Incollection);
        assert_eq!(parse_entry_type("inproceedings"), EntryType::Inproceedings);
        assert_eq!(parse_entry_type("mastersthesis"), EntryType::Mastersthesis);
        assert_eq!(parse_entry_type("misc"), EntryType::Misc);
        assert_eq!(parse_entry_type("phdthesis"), EntryType::Phdthesis);
        assert_eq!(parse_entry_type("proceedings"), EntryType::Proceedings);
        assert_eq!(parse_entry_type("techreport"), EntryType::Techreport);
        assert_eq!(parse_entry_type("unpublished"), EntryType::Unpublished);
    }
}
