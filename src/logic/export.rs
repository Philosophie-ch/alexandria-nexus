//! Export logic — pure types and CSV formatting helpers.
//!
//! Contains request/response types, CSV header constants, and pure
//! formatting functions used by the export process layer.
//! No async, no database, no I/O — only pure transformations.

use std::collections::HashMap;

use hexforge::HexforgeError;
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
// Error result types (structured, not HTTP responses)
// =============================================================================

/// Export error that the adapter layer converts into HTTP responses.
#[derive(Debug)]
pub enum ExportError {
    /// Requested IDs were not found.
    MissingIds(Vec<i64>),
    /// Requested keys were not found.
    MissingKeys(Vec<String>),
    /// Requested bibkeys were not found.
    MissingBibkeys(Vec<String>),
    /// Bad request (no selection criteria provided).
    BadRequest,
    /// Internal error.
    Internal(HexforgeError),
}

impl From<HexforgeError> for ExportError {
    fn from(e: HexforgeError) -> Self {
        ExportError::Internal(e)
    }
}

// =============================================================================
// CSV formatting helpers (pure functions)
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

/// Format author IDs for a given role, sorted by position.
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
                .map(|r| r.author_id.to_string())
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
    authors_map: &HashMap<i64, Author>,
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
                    authors_map.get(&r.author_id).map(|a| {
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
    keywords_map: &HashMap<i64, Keyword>,
) -> String {
    bib_keywords
        .map(|rows| {
            let names: Vec<&str> = rows
                .iter()
                .filter(|r| r.keyword_level == level)
                .filter_map(|r| keywords_map.get(&r.keyword_id).map(|k| k.name.as_str()))
                .collect();
            names.join(";")
        })
        .unwrap_or_default()
}

// =============================================================================
// CSV header constants
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
    "crossref_id",
    "journal_id",
    "volume",
    "number",
    "pages",
    "eid",
    "series_id",
    "address",
    "institution_id",
    "school_id",
    "publisher_id",
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
    "author_ids",
    "editor_ids",
    "guesteditor_ids",
    "keyword_ids",
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
// CSV building helpers (pure, synchronous)
// =============================================================================

/// Build authors CSV rows from pre-fetched data (header row + data rows).
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

/// Build journals CSV rows from pre-fetched data (header row + data rows).
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

/// Build publishers CSV rows from pre-fetched data (header row + data rows).
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

/// Build institutions CSV rows from pre-fetched data (header row + data rows).
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

/// Build schools CSV rows from pre-fetched data (header row + data rows).
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

/// Build series CSV rows from pre-fetched data (header row + data rows).
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

/// Build keywords CSV rows from pre-fetched data (header row + data rows).
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

/// Build bibitems CSV rows in IDs format (header row + data rows).
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

    let mut authors_by_bibitem: HashMap<i64, Vec<&BibitemAuthorsRow>> = HashMap::new();
    for row in author_rows {
        authors_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    let mut keywords_by_bibitem: HashMap<i64, Vec<&BibitemKeywordsRow>> = HashMap::new();
    for row in keyword_rows {
        keywords_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    for bib in bibitems {
        let bib_authors = authors_by_bibitem.get(&bib.id);
        let author_ids = format_role_ids(bib_authors, AuthorRole::Author);
        let editor_ids = format_role_ids(bib_authors, AuthorRole::Editor);
        let guesteditor_ids = format_role_ids(bib_authors, AuthorRole::Guesteditor);
        let keyword_ids = keywords_by_bibitem
            .get(&bib.id)
            .map(|kw_rows| {
                let mut ids: Vec<String> =
                    kw_rows.iter().map(|r| r.keyword_id.to_string()).collect();
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
            bib.title_unicode.clone(),
            opt_str(&bib.booktitle_latex).to_string(),
            opt_str(&bib.booktitle_unicode).to_string(),
            opt_i64(bib.crossref_id),
            opt_i64(bib.journal_id),
            opt_str(&bib.volume).to_string(),
            opt_str(&bib.number).to_string(),
            opt_str(&bib.pages).to_string(),
            opt_str(&bib.eid).to_string(),
            opt_i64(bib.series_id),
            opt_str(&bib.address).to_string(),
            opt_i64(bib.institution_id),
            opt_i64(bib.school_id),
            opt_i64(bib.publisher_id),
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

/// Build bibitems CSV rows in expanded format from pre-fetched data (header row + data rows).
///
/// All entity maps and junction data must be pre-fetched and passed in.
#[allow(clippy::too_many_arguments)]
pub fn build_bibitem_expanded_rows(
    bibitems: &[BibItem],
    author_rows: &[BibitemAuthorsRow],
    keyword_rows: &[BibitemKeywordsRow],
    authors_map: &HashMap<i64, Author>,
    journals_map: &HashMap<i64, Journal>,
    publishers_map: &HashMap<i64, Publisher>,
    institutions_map: &HashMap<i64, Institution>,
    schools_map: &HashMap<i64, School>,
    series_map: &HashMap<i64, Series>,
    crossrefs_map: &HashMap<i64, BibItem>,
    keywords_map: &HashMap<i64, Keyword>,
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

    let mut authors_by_bibitem: HashMap<i64, Vec<&BibitemAuthorsRow>> = HashMap::new();
    for row in author_rows {
        authors_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    let mut keywords_by_bibitem: HashMap<i64, Vec<&BibitemKeywordsRow>> = HashMap::new();
    for row in keyword_rows {
        keywords_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    for bib in bibitems {
        let bib_authors = authors_by_bibitem.get(&bib.id);

        let author_col = format_role_names(bib_authors, AuthorRole::Author, authors_map);
        let editor_col = format_role_names(bib_authors, AuthorRole::Editor, authors_map);
        let guesteditor_col = format_role_names(bib_authors, AuthorRole::Guesteditor, authors_map);

        let journal_name = bib
            .journal_id
            .and_then(|id| journals_map.get(&id))
            .map(|j| j.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let publisher_name = bib
            .publisher_id
            .and_then(|id| publishers_map.get(&id))
            .map(|p| p.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let institution_name = bib
            .institution_id
            .and_then(|id| institutions_map.get(&id))
            .map(|i| i.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let school_name = bib
            .school_id
            .and_then(|id| schools_map.get(&id))
            .map(|s| s.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let series_name = bib
            .series_id
            .and_then(|id| series_map.get(&id))
            .map(|s| s.name_unicode.as_str())
            .unwrap_or("")
            .to_string();
        let crossref_bibkey = bib
            .crossref_id
            .and_then(|id| crossrefs_map.get(&id))
            .map(|b| b.bibkey.as_str())
            .unwrap_or("")
            .to_string();

        let bib_keywords = keywords_by_bibitem.get(&bib.id);
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
            bib.title_unicode.clone(),
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

    fn make_author(id: i64, family: &str, given: &str) -> Author {
        Author {
            id,
            author_key: format!("key{id}"),
            family_name_latex: None,
            family_name_unicode: Some(family.to_string()),
            famous_name_latex: None,
            famous_name_unicode: None,
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
        bibitem_id: i64,
        author_id: i64,
        role: &str,
        position: i16,
        name_variant_latex: Option<&str>,
    ) -> BibitemAuthorsRow {
        BibitemAuthorsRow {
            bibitem_id,
            author_id,
            role: role.to_string(),
            position,
            name_variant_latex: name_variant_latex.map(str::to_string),
            name_variant_unicode: None,
        }
    }

    #[test]
    fn uses_name_variant_latex_when_present() {
        let author = make_author(1, "Smith", "John");
        let row = make_row(10, 1, "author", 1, Some("Schmidt, Hans"));

        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert(1, author);

        let result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        assert_eq!(result, "Schmidt, Hans");
    }

    #[test]
    fn falls_back_to_canonical_when_no_variant() {
        let author = make_author(1, "Smith", "John");
        let row = make_row(10, 1, "author", 1, None);

        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert(1, author);

        let result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        assert_eq!(result, "Smith, John");
    }

    #[test]
    fn mixed_variant_and_canonical() {
        let a1 = make_author(1, "Smith", "John");
        let a2 = make_author(2, "Müller", "Hans");
        let row1 = make_row(10, 1, "author", 1, Some("Schmidt, Johann"));
        let row2 = make_row(10, 2, "author", 2, None);

        let rows = vec![&row1, &row2];
        let mut map = HashMap::new();
        map.insert(1, a1);
        map.insert(2, a2);

        let result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        assert_eq!(result, "Schmidt, Johann and Müller, Hans");
    }

    #[test]
    fn mononym_used_when_no_variant() {
        let mut author = make_author(1, "", "");
        author.mononym_unicode = Some("Aristotle".to_string());
        let row = make_row(10, 1, "author", 1, None);

        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert(1, author);

        let result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        assert_eq!(result, "Aristotle");
    }

    #[test]
    fn filters_by_role() {
        let author = make_author(1, "Smith", "John");
        let row = make_row(10, 1, "editor", 1, Some("Smith, J."));

        let rows = vec![&row];
        let mut map = HashMap::new();
        map.insert(1, author);

        let author_result = format_role_names(Some(&rows), AuthorRole::Author, &map);
        let editor_result = format_role_names(Some(&rows), AuthorRole::Editor, &map);
        assert_eq!(author_result, "");
        assert_eq!(editor_result, "Smith, J.");
    }

    #[test]
    fn empty_when_no_rows() {
        let map: HashMap<i64, Author> = HashMap::new();
        let result = format_role_names(None, AuthorRole::Author, &map);
        assert_eq!(result, "");
    }
}
