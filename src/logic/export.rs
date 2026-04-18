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

/// Build authors CSV from pre-fetched data.
pub fn build_authors_csv(authors: &[Author]) -> Result<Vec<u8>, ExportError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "author_key",
        "given_name_latex",
        "given_name_unicode",
        "family_name_latex",
        "family_name_unicode",
        "mononym_latex",
        "mononym_unicode",
        "shorthand_latex",
        "shorthand_unicode",
        "famous_name_latex",
        "famous_name_unicode",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for a in authors {
        wtr.write_record([
            &a.id.to_string(),
            &a.author_key,
            opt_str(&a.given_name_latex),
            opt_str(&a.given_name_unicode),
            opt_str(&a.family_name_latex),
            opt_str(&a.family_name_unicode),
            opt_str(&a.mononym_latex),
            opt_str(&a.mononym_unicode),
            opt_str(&a.shorthand_latex),
            opt_str(&a.shorthand_unicode),
            opt_str(&a.famous_name_latex),
            opt_str(&a.famous_name_unicode),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())))
}

/// Build journals CSV from pre-fetched data.
pub fn build_journals_csv(journals: &[Journal]) -> Result<Vec<u8>, ExportError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "journal_key",
        "name_latex",
        "name_unicode",
        "issn_print",
        "issn_electronic",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for j in journals {
        wtr.write_record([
            &j.id.to_string(),
            &j.journal_key,
            &j.name_latex,
            &j.name_unicode,
            opt_str(&j.issn_print),
            opt_str(&j.issn_electronic),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())))
}

/// Build publishers CSV from pre-fetched data.
pub fn build_publishers_csv(publishers: &[Publisher]) -> Result<Vec<u8>, ExportError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "publisher_key",
        "name_latex",
        "name_unicode",
        "default_address",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for p in publishers {
        wtr.write_record([
            &p.id.to_string(),
            &p.publisher_key,
            &p.name_latex,
            &p.name_unicode,
            opt_str(&p.default_address),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())))
}

/// Build institutions CSV from pre-fetched data.
pub fn build_institutions_csv(institutions: &[Institution]) -> Result<Vec<u8>, ExportError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id",
        "institution_key",
        "name_latex",
        "name_unicode",
        "default_address",
    ])
    .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for inst in institutions {
        wtr.write_record([
            &inst.id.to_string(),
            &inst.institution_key,
            &inst.name_latex,
            &inst.name_unicode,
            opt_str(&inst.default_address),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())))
}

/// Build schools CSV from pre-fetched data.
pub fn build_schools_csv(schools: &[School]) -> Result<Vec<u8>, ExportError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(["id", "school_key", "name_latex", "name_unicode"])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for s in schools {
        wtr.write_record([
            &s.id.to_string(),
            &s.school_key,
            &s.name_latex,
            &s.name_unicode,
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())))
}

/// Build series CSV from pre-fetched data.
pub fn build_series_csv(series_list: &[Series]) -> Result<Vec<u8>, ExportError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(["id", "series_key", "name_latex", "name_unicode"])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for s in series_list {
        wtr.write_record([
            &s.id.to_string(),
            &s.series_key,
            &s.name_latex,
            &s.name_unicode,
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())))
}

/// Build keywords CSV from pre-fetched data.
pub fn build_keywords_csv(keywords: &[Keyword]) -> Result<Vec<u8>, ExportError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(["id", "name", "level"])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;

    for kw in keywords {
        wtr.write_record([&kw.id.to_string(), &kw.name, &kw.level.to_string()])
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())))
}

/// Build bibitems CSV in IDs format from pre-fetched data.
///
/// All junction data must be pre-fetched and passed in.
pub fn build_bibitems_ids_csv(
    bibitems: &[BibItem],
    author_rows: &[BibitemAuthorsRow],
    keyword_rows: &[BibitemKeywordsRow],
) -> Result<Vec<u8>, ExportError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(IDS_FORMAT_HEADER)
        .map_err(|e| HexforgeError::internal(e.to_string()))?;

    if bibitems.is_empty() {
        return wtr
            .into_inner()
            .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())));
    }

    // Group authors by bibitem_id
    let mut authors_by_bibitem: HashMap<i64, Vec<&BibitemAuthorsRow>> = HashMap::new();
    for row in author_rows {
        authors_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    // Group keywords by bibitem_id
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
            .map(|rows| {
                let mut ids: Vec<String> = rows.iter().map(|r| r.keyword_id.to_string()).collect();
                ids.sort();
                ids.join(";")
            })
            .unwrap_or_default();

        wtr.write_record([
            &bib.id.to_string(),
            &bib.entry_type.to_string(),
            &bib.bibkey,
            opt_str(&bib.options),
            opt_str(&bib.shorthand),
            &opt_i16(bib.date_year),
            &opt_display(&bib.pubstate),
            &bib.title_latex,
            &bib.title_unicode,
            opt_str(&bib.booktitle_latex),
            opt_str(&bib.booktitle_unicode),
            &opt_i64(bib.crossref_id),
            &opt_i64(bib.journal_id),
            opt_str(&bib.volume),
            opt_str(&bib.number),
            opt_str(&bib.pages),
            opt_str(&bib.eid),
            &opt_i64(bib.series_id),
            opt_str(&bib.address),
            &opt_i64(bib.institution_id),
            &opt_i64(bib.school_id),
            &opt_i64(bib.publisher_id),
            opt_str(&bib.type_field),
            opt_str(&bib.edition),
            opt_str(&bib.note_latex),
            opt_str(&bib.note_unicode),
            opt_str(&bib.issuetitle_latex),
            opt_str(&bib.issuetitle_unicode),
            opt_str(&bib.extra_note_latex),
            opt_str(&bib.extra_note_unicode),
            opt_str(&bib.urn),
            opt_str(&bib.eprint),
            opt_str(&bib.doi),
            opt_str(&bib.url),
            &opt_display(&bib.langid),
            &bib.is_translation.to_string(),
            &opt_display(&bib.epoch),
            &author_ids,
            &editor_ids,
            &guesteditor_ids,
            &keyword_ids,
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())))
}

/// Build bibitems CSV in expanded format from pre-fetched data.
///
/// All entity maps and junction data must be pre-fetched and passed in.
#[allow(clippy::too_many_arguments)]
pub fn build_bibitems_expanded_csv(
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
) -> Result<Vec<u8>, ExportError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(EXPANDED_FORMAT_HEADER)
        .map_err(|e| HexforgeError::internal(e.to_string()))?;

    if bibitems.is_empty() {
        return wtr
            .into_inner()
            .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())));
    }

    // Group junction data by bibitem_id
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
            .unwrap_or("");

        let publisher_name = bib
            .publisher_id
            .and_then(|id| publishers_map.get(&id))
            .map(|p| p.name_unicode.as_str())
            .unwrap_or("");

        let institution_name = bib
            .institution_id
            .and_then(|id| institutions_map.get(&id))
            .map(|i| i.name_unicode.as_str())
            .unwrap_or("");

        let school_name = bib
            .school_id
            .and_then(|id| schools_map.get(&id))
            .map(|s| s.name_unicode.as_str())
            .unwrap_or("");

        let series_name = bib
            .series_id
            .and_then(|id| series_map.get(&id))
            .map(|s| s.name_unicode.as_str())
            .unwrap_or("");

        let crossref_bibkey = bib
            .crossref_id
            .and_then(|id| crossrefs_map.get(&id))
            .map(|b| b.bibkey.as_str())
            .unwrap_or("");

        // Keywords by level
        let bib_keywords = keywords_by_bibitem.get(&bib.id);
        let kw_level1 = format_keywords_at_level(bib_keywords, 1, keywords_map);
        let kw_level2 = format_keywords_at_level(bib_keywords, 2, keywords_map);
        let kw_level3 = format_keywords_at_level(bib_keywords, 3, keywords_map);

        wtr.write_record([
            &bib.entry_type.to_string(),
            &bib.bibkey,
            &author_col,
            &editor_col,
            &guesteditor_col,
            opt_str(&bib.options),
            opt_str(&bib.shorthand),
            &opt_i16(bib.date_year),
            &opt_display(&bib.pubstate),
            &bib.title_latex,
            &bib.title_unicode,
            opt_str(&bib.booktitle_latex),
            opt_str(&bib.booktitle_unicode),
            crossref_bibkey,
            journal_name,
            opt_str(&bib.volume),
            opt_str(&bib.number),
            opt_str(&bib.pages),
            opt_str(&bib.eid),
            series_name,
            opt_str(&bib.address),
            institution_name,
            school_name,
            publisher_name,
            opt_str(&bib.type_field),
            opt_str(&bib.edition),
            opt_str(&bib.note_latex),
            opt_str(&bib.note_unicode),
            opt_str(&bib.issuetitle_latex),
            opt_str(&bib.issuetitle_unicode),
            opt_str(&bib.extra_note_latex),
            opt_str(&bib.extra_note_unicode),
            opt_str(&bib.urn),
            opt_str(&bib.eprint),
            opt_str(&bib.doi),
            opt_str(&bib.url),
            &kw_level1,
            &kw_level2,
            &kw_level3,
            &opt_display(&bib.epoch),
            &opt_display(&bib.langid),
            &bib.is_translation.to_string(),
        ])
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| ExportError::Internal(HexforgeError::internal(e.to_string())))
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
