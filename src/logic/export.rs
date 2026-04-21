//! Export logic — pure types and row-building helpers.
//!
//! Contains request/response types, header constants, and pure
//! formatting functions used by the export process layer.
//! No async, no database, no I/O — only pure transformations.

use std::collections::HashMap;

use serde::Deserialize;

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow};
use crate::domain::{
    Author, AuthorRole, BibItem, Institution, Journal, Keyword, Publisher, School, Series,
};

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
// Row-building helpers (pure functions)
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
// Role/keyword formatting helpers (pure functions)
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

/// Format author/editor/guesteditor names for a given role.
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

// =============================================================================
// Header constants
// =============================================================================

/// IDs format header columns.
pub const IDS_FORMAT_HEADER: &[&str] = &[
    "id",
    "entry_type",
    "bibkey",
    "options",
    "shorthand",
    "date_year",
    "pubstate",
    "title_latex",
    "title_unicode",
    "booktitle_latex",
    "booktitle_unicode",
    "crossref",
    "journal_key",
    "volume",
    "number",
    "pages",
    "eid",
    "series_key",
    "address",
    "institution_key",
    "school_key",
    "publisher_key",
    "type_field",
    "edition",
    "note_latex",
    "note_unicode",
    "issuetitle_latex",
    "issuetitle_unicode",
    "extra_note_latex",
    "extra_note_unicode",
    "urn",
    "eprint",
    "doi",
    "url",
    "langid",
    "is_translation",
    "epoch",
    "author_keys",
    "editor_keys",
    "guesteditor_keys",
    "keyword_keys",
];

/// Expanded format header columns.
pub const EXPANDED_FORMAT_HEADER: &[&str] = &[
    "entry_type",
    "bibkey",
    "author",
    "editor",
    "guesteditor",
    "options",
    "shorthand",
    "date_year",
    "pubstate",
    "title_latex",
    "title_unicode",
    "booktitle_latex",
    "booktitle_unicode",
    "crossref",
    "journal",
    "volume",
    "number",
    "pages",
    "eid",
    "series",
    "address",
    "institution",
    "school",
    "publisher",
    "type_field",
    "edition",
    "note_latex",
    "note_unicode",
    "issuetitle_latex",
    "issuetitle_unicode",
    "extra_note_latex",
    "extra_note_unicode",
    "urn",
    "eprint",
    "doi",
    "url",
    "kw_level1",
    "kw_level2",
    "kw_level3",
    "epoch",
    "langid",
    "is_translation",
];

// =============================================================================
// Row-building helpers (pure, synchronous)
// =============================================================================

/// Build author data rows from pre-fetched data.
pub fn build_author_rows(authors: &[Author]) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(authors.len() + 1);
    rows.push(vec![
        "id".into(),
        "author_key".into(),
        "given_name_latex".into(),
        "given_name_unicode".into(),
        "family_name_latex".into(),
        "family_name_unicode".into(),
        "mononym_latex".into(),
        "mononym_unicode".into(),
        "shorthand_latex".into(),
        "shorthand_unicode".into(),
        "famous_name_latex".into(),
        "famous_name_unicode".into(),
    ]);
    for a in authors {
        rows.push(vec![
            a.id.to_string(),
            a.author_key.clone(),
            opt_str(&a.given_name_latex).to_string(),
            opt_str(&a.given_name_unicode).to_string(),
            opt_str(&a.family_name_latex).to_string(),
            opt_str(&a.family_name_unicode).to_string(),
            opt_str(&a.mononym_latex).to_string(),
            opt_str(&a.mononym_unicode).to_string(),
            opt_str(&a.shorthand_latex).to_string(),
            opt_str(&a.shorthand_unicode).to_string(),
            opt_str(&a.famous_name_latex).to_string(),
            opt_str(&a.famous_name_unicode).to_string(),
        ]);
    }
    rows
}

/// Build journal data rows from pre-fetched data.
pub fn build_journal_rows(journals: &[Journal]) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(journals.len() + 1);
    rows.push(vec![
        "id".into(),
        "journal_key".into(),
        "name_latex".into(),
        "name_unicode".into(),
        "issn_print".into(),
        "issn_electronic".into(),
    ]);
    for j in journals {
        rows.push(vec![
            j.id.to_string(),
            j.journal_key.clone(),
            j.name_latex.clone(),
            j.name_unicode.clone(),
            opt_str(&j.issn_print).to_string(),
            opt_str(&j.issn_electronic).to_string(),
        ]);
    }
    rows
}

/// Build publisher data rows from pre-fetched data.
pub fn build_publisher_rows(publishers: &[Publisher]) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(publishers.len() + 1);
    rows.push(vec![
        "id".into(),
        "publisher_key".into(),
        "name_latex".into(),
        "name_unicode".into(),
        "default_address".into(),
    ]);
    for p in publishers {
        rows.push(vec![
            p.id.to_string(),
            p.publisher_key.clone(),
            p.name_latex.clone(),
            p.name_unicode.clone(),
            opt_str(&p.default_address).to_string(),
        ]);
    }
    rows
}

/// Build institution data rows from pre-fetched data.
pub fn build_institution_rows(institutions: &[Institution]) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(institutions.len() + 1);
    rows.push(vec![
        "id".into(),
        "institution_key".into(),
        "name_latex".into(),
        "name_unicode".into(),
        "default_address".into(),
    ]);
    for inst in institutions {
        rows.push(vec![
            inst.id.to_string(),
            inst.institution_key.clone(),
            inst.name_latex.clone(),
            inst.name_unicode.clone(),
            opt_str(&inst.default_address).to_string(),
        ]);
    }
    rows
}

/// Build school data rows from pre-fetched data.
pub fn build_school_rows(schools: &[School]) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(schools.len() + 1);
    rows.push(vec![
        "id".into(),
        "school_key".into(),
        "name_latex".into(),
        "name_unicode".into(),
    ]);
    for s in schools {
        rows.push(vec![
            s.id.to_string(),
            s.school_key.clone(),
            s.name_latex.clone(),
            s.name_unicode.clone(),
        ]);
    }
    rows
}

/// Build series data rows from pre-fetched data.
pub fn build_series_rows(series_list: &[Series]) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(series_list.len() + 1);
    rows.push(vec![
        "id".into(),
        "series_key".into(),
        "name_latex".into(),
        "name_unicode".into(),
    ]);
    for s in series_list {
        rows.push(vec![
            s.id.to_string(),
            s.series_key.clone(),
            s.name_latex.clone(),
            s.name_unicode.clone(),
        ]);
    }
    rows
}

/// Build keyword data rows from pre-fetched data.
pub fn build_keyword_rows(keywords: &[Keyword]) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(keywords.len() + 1);
    rows.push(vec!["id".into(), "name".into(), "level".into()]);
    for kw in keywords {
        rows.push(vec![
            kw.id.to_string(),
            kw.name.clone(),
            kw.level.to_string(),
        ]);
    }
    rows
}

/// Build bibitem data rows in IDs format (header row + data rows).
///
/// All junction data must be pre-fetched and passed in.
pub fn build_bibitem_id_rows(
    bibitems: &[BibItem],
    author_rows: &[BibitemAuthorsRow],
    keyword_rows: &[BibitemKeywordsRow],
) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(bibitems.len() + 1);
    rows.push(IDS_FORMAT_HEADER.iter().map(|s| s.to_string()).collect());

    if bibitems.is_empty() {
        return rows;
    }

    let mut authors_by_bibitem: HashMap<String, Vec<&BibitemAuthorsRow>> = HashMap::new();
    for row in author_rows {
        authors_by_bibitem
            .entry(row.bibkey.clone())
            .or_default()
            .push(row);
    }

    let mut keywords_by_bibitem: HashMap<String, Vec<&BibitemKeywordsRow>> = HashMap::new();
    for row in keyword_rows {
        keywords_by_bibitem
            .entry(row.bibkey.clone())
            .or_default()
            .push(row);
    }

    for bib in bibitems {
        let bib_authors = authors_by_bibitem.get(&bib.bibkey);
        let author_ids = format_role_ids(bib_authors, AuthorRole::Author);
        let editor_ids = format_role_ids(bib_authors, AuthorRole::Editor);
        let guesteditor_ids = format_role_ids(bib_authors, AuthorRole::Guesteditor);
        let keyword_ids = keywords_by_bibitem
            .get(&bib.bibkey)
            .map(|kw_rows| {
                let mut ids: Vec<String> =
                    kw_rows.iter().map(|r| r.keyword_key.to_string()).collect();
                ids.sort();
                ids.join(";")
            })
            .unwrap_or_default();

        rows.push(vec![
            bib.id.to_string(),
            bib.entry_type.to_string(),
            bib.bibkey.clone(),
            opt_str(&bib.options).to_string(),
            opt_str(&bib.shorthand).to_string(),
            opt_i16(bib.date_year),
            opt_display(&bib.pubstate),
            bib.title_latex.clone(),
            bib.title_unicode.clone().unwrap_or_default(),
            opt_str(&bib.booktitle_latex).to_string(),
            opt_str(&bib.booktitle_unicode).to_string(),
            opt_str(&bib.crossref).to_string(),
            opt_str(&bib.journal_key).to_string(),
            opt_str(&bib.volume).to_string(),
            opt_str(&bib.number).to_string(),
            opt_str(&bib.pages).to_string(),
            opt_str(&bib.eid).to_string(),
            opt_str(&bib.series_key).to_string(),
            opt_str(&bib.address).to_string(),
            opt_str(&bib.institution_key).to_string(),
            opt_str(&bib.school_key).to_string(),
            opt_str(&bib.publisher_key).to_string(),
            opt_str(&bib.type_field).to_string(),
            opt_str(&bib.edition).to_string(),
            opt_str(&bib.note_latex).to_string(),
            opt_str(&bib.note_unicode).to_string(),
            opt_str(&bib.issuetitle_latex).to_string(),
            opt_str(&bib.issuetitle_unicode).to_string(),
            opt_str(&bib.extra_note_latex).to_string(),
            opt_str(&bib.extra_note_unicode).to_string(),
            opt_str(&bib.urn).to_string(),
            opt_str(&bib.eprint).to_string(),
            opt_str(&bib.doi).to_string(),
            opt_str(&bib.url).to_string(),
            opt_display(&bib.langid),
            bib.is_translation.to_string(),
            opt_display(&bib.epoch),
            author_ids,
            editor_ids,
            guesteditor_ids,
            keyword_ids,
        ]);
    }
    rows
}

/// Build bibitem data rows in expanded format from pre-fetched data.
///
/// All entity maps and junction data must be pre-fetched and passed in.
#[allow(clippy::too_many_arguments)]
pub fn build_bibitem_expanded_rows(
    bibitems: &[BibItem],
    author_rows: &[BibitemAuthorsRow],
    keyword_rows: &[BibitemKeywordsRow],
    authors_map: &HashMap<String, Author>,
    journals_map: &HashMap<String, Journal>,
    publishers_map: &HashMap<String, Publisher>,
    institutions_map: &HashMap<String, Institution>,
    schools_map: &HashMap<String, School>,
    series_map: &HashMap<String, Series>,
    crossrefs_map: &HashMap<String, BibItem>,
    keywords_map: &HashMap<String, Keyword>,
) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(bibitems.len() + 1);
    rows.push(
        EXPANDED_FORMAT_HEADER
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );

    if bibitems.is_empty() {
        return rows;
    }

    let mut authors_by_bibitem: HashMap<String, Vec<&BibitemAuthorsRow>> = HashMap::new();
    for row in author_rows {
        authors_by_bibitem
            .entry(row.bibkey.clone())
            .or_default()
            .push(row);
    }

    let mut keywords_by_bibitem: HashMap<String, Vec<&BibitemKeywordsRow>> = HashMap::new();
    for row in keyword_rows {
        keywords_by_bibitem
            .entry(row.bibkey.clone())
            .or_default()
            .push(row);
    }

    for bib in bibitems {
        let bib_authors = authors_by_bibitem.get(&bib.bibkey);

        let author_col = format_role_names(bib_authors, AuthorRole::Author, authors_map);
        let editor_col = format_role_names(bib_authors, AuthorRole::Editor, authors_map);
        let guesteditor_col = format_role_names(bib_authors, AuthorRole::Guesteditor, authors_map);

        let journal_name = bib
            .journal_key
            .as_deref()
            .and_then(|k| journals_map.get(k))
            .map(|j| j.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let publisher_name = bib
            .publisher_key
            .as_deref()
            .and_then(|k| publishers_map.get(k))
            .map(|p| p.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let institution_name = bib
            .institution_key
            .as_deref()
            .and_then(|k| institutions_map.get(k))
            .map(|i| i.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let school_name = bib
            .school_key
            .as_deref()
            .and_then(|k| schools_map.get(k))
            .map(|s| s.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let series_name = bib
            .series_key
            .as_deref()
            .and_then(|k| series_map.get(k))
            .map(|s| s.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let crossref_bibkey = bib
            .crossref
            .as_deref()
            .and_then(|k| crossrefs_map.get(k))
            .map(|b| b.bibkey.as_str())
            .unwrap_or("")
            .to_string();

        let bib_keywords = keywords_by_bibitem.get(&bib.bibkey);
        let kw_level1 = format_keywords_at_level(bib_keywords, 1, keywords_map);
        let kw_level2 = format_keywords_at_level(bib_keywords, 2, keywords_map);
        let kw_level3 = format_keywords_at_level(bib_keywords, 3, keywords_map);

        rows.push(vec![
            bib.entry_type.to_string(),
            bib.bibkey.clone(),
            author_col,
            editor_col,
            guesteditor_col,
            opt_str(&bib.options).to_string(),
            opt_str(&bib.shorthand).to_string(),
            opt_i16(bib.date_year),
            opt_display(&bib.pubstate),
            bib.title_latex.clone(),
            bib.title_unicode.clone().unwrap_or_default(),
            opt_str(&bib.booktitle_latex).to_string(),
            opt_str(&bib.booktitle_unicode).to_string(),
            crossref_bibkey,
            journal_name,
            opt_str(&bib.volume).to_string(),
            opt_str(&bib.number).to_string(),
            opt_str(&bib.pages).to_string(),
            opt_str(&bib.eid).to_string(),
            series_name,
            opt_str(&bib.address).to_string(),
            institution_name,
            school_name,
            publisher_name,
            opt_str(&bib.type_field).to_string(),
            opt_str(&bib.edition).to_string(),
            opt_str(&bib.note_latex).to_string(),
            opt_str(&bib.note_unicode).to_string(),
            opt_str(&bib.issuetitle_latex).to_string(),
            opt_str(&bib.issuetitle_unicode).to_string(),
            opt_str(&bib.extra_note_latex).to_string(),
            opt_str(&bib.extra_note_unicode).to_string(),
            opt_str(&bib.urn).to_string(),
            opt_str(&bib.eprint).to_string(),
            opt_str(&bib.doi).to_string(),
            opt_str(&bib.url).to_string(),
            kw_level1,
            kw_level2,
            kw_level3,
            opt_display(&bib.epoch),
            opt_display(&bib.langid),
            bib.is_translation.to_string(),
        ]);
    }
    rows
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

        let result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        assert_eq!(result, "Schmidt, Hans");
    }

    #[test]
    fn falls_back_to_canonical_when_no_variant() {
        let author = make_author("key1", "Smith", "John");
        let row = make_row("bib1", "key1", "author", 1, None);

        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert("key1".to_string(), author);

        let result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        assert_eq!(result, "Smith, John");
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

        let result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        assert_eq!(result, "Schmidt, Johann and Müller, Hans");
    }

    #[test]
    fn mononym_used_when_no_variant() {
        let mut author = make_author("key1", "", "");
        author.mononym_unicode = Some("Aristotle".to_string());
        let row = make_row("bib1", "key1", "author", 1, None);

        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert("key1".to_string(), author);

        let result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        assert_eq!(result, "Aristotle");
    }

    #[test]
    fn filters_by_role() {
        let author = make_author("key1", "Smith", "John");
        let row = make_row("bib1", "key1", "editor", 1, Some("Smith, J."));

        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert("key1".to_string(), author);

        let author_result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        let editor_result = format_role_names(Some(&rows), AuthorRole::Editor, &map);
        assert_eq!(author_result, "");
        assert_eq!(editor_result, "Smith, J.");
    }

    #[test]
    fn empty_when_no_rows() {
        let map: HashMap<String, Author> = HashMap::new();
        let result = format_role_names(None, AuthorRole::Author, &map);
        assert_eq!(result, "");
    }
}
