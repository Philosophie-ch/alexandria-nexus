//! Export request/response types and format-agnostic domain helpers.
//!
//! Pure functions only — no I/O, no async, no format-specific serialization.
//! Serialization to external formats lives in the adapters layer.

use std::collections::HashMap;

use serde::Deserialize;

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow};
use crate::domain::{Author, AuthorRole, Keyword};

// =============================================================================
// Request types (no HTTP, just data)
// =============================================================================

/// Bibitem export format.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    #[default]
    Expanded,
    Ids,
}

/// Request body for bibitem export.
#[derive(Debug, Deserialize)]
pub struct BibitemExportRequest {
    #[serde(default)]
    pub format: ExportFormat,
    #[serde(default)]
    pub all: bool,
    pub ids: Option<Vec<i64>>,
    pub bibkeys: Option<Vec<String>>,
}

/// Request body for entity export.
#[derive(Debug, Deserialize)]
pub struct EntityExportRequest {
    #[serde(default)]
    pub all: bool,
    pub ids: Option<Vec<i64>>,
    pub keys: Option<Vec<String>>,
}

// =============================================================================
// Format-agnostic value helpers
// =============================================================================

pub fn opt_str(v: &Option<String>) -> &str {
    v.as_deref().unwrap_or("")
}

pub fn opt_i64(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

pub fn opt_i16(v: Option<i16>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

pub fn opt_display<T: std::fmt::Display>(v: &Option<T>) -> String {
    v.as_ref().map(|x| x.to_string()).unwrap_or_default()
}

// =============================================================================
// Domain-level formatting helpers
// =============================================================================

/// Format author keys for a given role, sorted by position.
pub fn format_role_ids(bib_authors: Option<&Vec<&BibitemAuthorsRow>>, role: AuthorRole) -> String {
    let role_str = role.to_string();
    bib_authors
        .map(|rows| {
            let mut filtered: Vec<&BibitemAuthorsRow> = rows
                .iter()
                .filter(|r| r.role == role_str)
                .copied()
                .collect();
            filtered.sort_by_key(|r| r.position);
            filtered
                .iter()
                .map(|r| r.author_key.to_string())
                .collect::<Vec<_>>()
                .join(";")
        })
        .unwrap_or_default()
}

/// Format author/editor/guesteditor display names for a given role.
///
/// Returns "LastName, FirstName and LastName2, FirstName2" using unicode names,
/// ordered by position.
pub fn format_role_names(
    bib_authors: Option<&Vec<&BibitemAuthorsRow>>,
    role: AuthorRole,
    authors_map: &HashMap<String, Author>,
) -> String {
    let role_str = role.to_string();
    bib_authors
        .map(|rows| {
            let mut filtered: Vec<&BibitemAuthorsRow> = rows
                .iter()
                .filter(|r| r.role == role_str)
                .copied()
                .collect();
            filtered.sort_by_key(|r| r.position);

            let names: Vec<String> = filtered
                .iter()
                .filter_map(|r| {
                    if let Some(ref variant) = r.name_variant_latex {
                        return Some(variant.clone());
                    }
                    authors_map.get(&r.author_key).map(|a| {
                        if let Some(ref mononym) = a.mononym_unicode {
                            mononym.clone()
                        } else {
                            let family = a.family_name_unicode.as_deref().unwrap_or("");
                            let given = a.given_name_unicode.as_deref().unwrap_or("");
                            if given.is_empty() {
                                family.to_string()
                            } else {
                                format!("{family}, {given}")
                            }
                        }
                    })
                })
                .collect();

            names.join(" and ")
        })
        .unwrap_or_default()
}

/// Format keyword names at a given level, semicolon-separated.
pub fn format_keywords_at_level(
    bib_keywords: Option<&Vec<&BibitemKeywordsRow>>,
    level: i16,
    keywords_map: &HashMap<String, Keyword>,
) -> String {
    bib_keywords
        .map(|rows| {
            let names: Vec<&str> = rows
                .iter()
                .filter(|r| r.keyword_level == level)
                .filter_map(|r| keywords_map.get(&r.keyword_key).map(|k| k.name.as_str()))
                .collect();
            names.join(";")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Author;
    use crate::domain::junctions::BibitemAuthorsRow;
    use chrono::Utc;

    fn make_author(key: &str, family: &str, given: &str) -> Author {
        Author {
            id: 1,
            author_key: key.to_string(),
            family_name_latex: None,
            family_name_unicode: Some(family.to_string()),
            famous_name_latex: None,
            famous_name_unicode: None,
            famous: false,
            given_name_latex: None,
            given_name_unicode: Some(given.to_string()),
            mononym_latex: None,
            mononym_unicode: None,
            name_variants_latex: None,
            name_variants_unicode: None,
            shorthand_latex: None,
            shorthand_unicode: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_row(
        bibkey: &str,
        author_key: &str,
        role: &str,
        position: i16,
        name_variant_latex: Option<&str>,
    ) -> BibitemAuthorsRow {
        BibitemAuthorsRow {
            bibkey: bibkey.to_string(),
            author_key: author_key.to_string(),
            role: role.to_string(),
            position,
            name_variant_latex: name_variant_latex.map(str::to_string),
            name_variant_unicode: None,
        }
    }

    #[test]
    fn uses_name_variant_latex_when_present() {
        let author = make_author("key1", "Smith", "John");
        let row = make_row("bib1", "key1", "author", 1, Some("Schmidt, Hans"));
        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert("key1".to_string(), author);
        assert_eq!(
            format_role_names(Some(&rows), AuthorRole::Author, &map),
            "Schmidt, Hans"
        );
    }

    #[test]
    fn falls_back_to_canonical_when_no_variant() {
        let author = make_author("key1", "Smith", "John");
        let row = make_row("bib1", "key1", "author", 1, None);
        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert("key1".to_string(), author);
        assert_eq!(
            format_role_names(Some(&rows), AuthorRole::Author, &map),
            "Smith, John"
        );
    }

    #[test]
    fn mixed_variant_and_canonical() {
        let a1 = make_author("key1", "Smith", "John");
        let a2 = make_author("key2", "Müller", "Hans");
        let row1 = make_row("bib1", "key1", "author", 1, Some("Schmidt, Johann"));
        let row2 = make_row("bib1", "key2", "author", 2, None);
        let rows = vec![&row1, &row2];
        let mut map = HashMap::new();
        map.insert("key1".to_string(), a1);
        map.insert("key2".to_string(), a2);
        assert_eq!(
            format_role_names(Some(&rows), AuthorRole::Author, &map),
            "Schmidt, Johann and Müller, Hans"
        );
    }

    #[test]
    fn mononym_used_when_no_variant() {
        let mut author = make_author("key1", "", "");
        author.mononym_unicode = Some("Aristotle".to_string());
        let row = make_row("bib1", "key1", "author", 1, None);
        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert("key1".to_string(), author);
        assert_eq!(
            format_role_names(Some(&rows), AuthorRole::Author, &map),
            "Aristotle"
        );
    }

    #[test]
    fn filters_by_role() {
        let author = make_author("key1", "Smith", "John");
        let row = make_row("bib1", "key1", "editor", 1, Some("Smith, J."));
        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert("key1".to_string(), author);
        assert_eq!(format_role_names(Some(&rows), AuthorRole::Author, &map), "");
        assert_eq!(
            format_role_names(Some(&rows), AuthorRole::Editor, &map),
            "Smith, J."
        );
    }

    #[test]
    fn empty_when_no_rows() {
        let map: HashMap<String, Author> = HashMap::new();
        assert_eq!(format_role_names(None, AuthorRole::Author, &map), "");
    }
}
