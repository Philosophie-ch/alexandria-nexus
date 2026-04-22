//! CSV row serialization for entity and bibitem exports.
//!
//! All functions here are pure but format-specific (CSV/text-array output).
//! They live in adapters because serializing to an external format is a
//! boundary concern — exactly like SQL or JSON.

use std::collections::HashMap;

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow};
use crate::domain::{
    Author, AuthorRole, BibItem, Institution, Journal, Keyword, Publisher, School, Series,
};
use crate::logic::export::{
    format_keywords_at_level, format_role_ids, format_role_names, opt_display, opt_i16, opt_str,
};
use crate::process::export::{BibitemExpandedData, BibitemExportData};

// ── Header constants ──────────────────────────────────────────────────────────

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

// ── Entity row builders ───────────────────────────────────────────────────────

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
        "famous".into(),
        "name_variants_latex".into(),
        "name_variants_unicode".into(),
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
            a.famous.to_string(),
            text_array(&a.name_variants_latex),
            text_array(&a.name_variants_unicode),
        ]);
    }
    rows
}

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

pub fn build_keyword_rows(keywords: &[Keyword]) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(keywords.len() + 1);
    rows.push(vec![
        "id".into(),
        "keyword_key".into(),
        "name".into(),
        "level".into(),
    ]);
    for kw in keywords {
        rows.push(vec![
            kw.id.to_string(),
            kw.keyword_key.clone(),
            kw.name.clone(),
            kw.level.to_string(),
        ]);
    }
    rows
}

// ── Bibitem row builders ──────────────────────────────────────────────────────

pub fn bibitems_to_rows(data: BibitemExportData) -> Vec<Vec<String>> {
    match data {
        BibitemExportData::Ids {
            bibitems,
            author_rows,
            keyword_rows,
        } => build_bibitem_id_rows(&bibitems, &author_rows, &keyword_rows),
        BibitemExportData::Expanded(d) => {
            let BibitemExpandedData {
                bibitems,
                author_rows,
                keyword_rows,
                authors_map,
                journals_map,
                publishers_map,
                institutions_map,
                schools_map,
                series_map,
                crossrefs_map,
                keywords_map,
            } = *d;
            build_bibitem_expanded_rows(
                &bibitems,
                &author_rows,
                &keyword_rows,
                &authors_map,
                &journals_map,
                &publishers_map,
                &institutions_map,
                &schools_map,
                &series_map,
                &crossrefs_map,
                &keywords_map,
            )
        }
    }
}

fn build_bibitem_id_rows(
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

#[allow(clippy::too_many_arguments)]
fn build_bibitem_expanded_rows(
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
        let bib_keywords = keywords_by_bibitem.get(&bib.bibkey);

        rows.push(vec![
            bib.entry_type.to_string(),
            bib.bibkey.clone(),
            format_role_names(bib_authors, AuthorRole::Author, authors_map),
            format_role_names(bib_authors, AuthorRole::Editor, authors_map),
            format_role_names(bib_authors, AuthorRole::Guesteditor, authors_map),
            opt_str(&bib.options).to_string(),
            opt_str(&bib.shorthand).to_string(),
            opt_i16(bib.date_year),
            opt_display(&bib.pubstate),
            bib.title_latex.clone(),
            bib.title_unicode.clone().unwrap_or_default(),
            opt_str(&bib.booktitle_latex).to_string(),
            opt_str(&bib.booktitle_unicode).to_string(),
            bib.crossref
                .as_deref()
                .and_then(|k| crossrefs_map.get(k))
                .map(|b| b.bibkey.as_str())
                .unwrap_or("")
                .to_string(),
            bib.journal_key
                .as_deref()
                .and_then(|k| journals_map.get(k))
                .map(|j| j.name_unicode.as_str())
                .unwrap_or("")
                .to_string(),
            opt_str(&bib.volume).to_string(),
            opt_str(&bib.number).to_string(),
            opt_str(&bib.pages).to_string(),
            opt_str(&bib.eid).to_string(),
            bib.series_key
                .as_deref()
                .and_then(|k| series_map.get(k))
                .map(|s| s.name_unicode.as_str())
                .unwrap_or("")
                .to_string(),
            opt_str(&bib.address).to_string(),
            bib.institution_key
                .as_deref()
                .and_then(|k| institutions_map.get(k))
                .map(|i| i.name_unicode.as_str())
                .unwrap_or("")
                .to_string(),
            bib.school_key
                .as_deref()
                .and_then(|k| schools_map.get(k))
                .map(|s| s.name_unicode.as_str())
                .unwrap_or("")
                .to_string(),
            bib.publisher_key
                .as_deref()
                .and_then(|k| publishers_map.get(k))
                .map(|p| p.name_unicode.as_str())
                .unwrap_or("")
                .to_string(),
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
            format_keywords_at_level(bib_keywords, 1, keywords_map),
            format_keywords_at_level(bib_keywords, 2, keywords_map),
            format_keywords_at_level(bib_keywords, 3, keywords_map),
            opt_display(&bib.epoch),
            opt_display(&bib.langid),
            bib.is_translation.to_string(),
        ]);
    }
    rows
}

/// Serialize `Option<Vec<String>>` as a text-array literal `{v1,v2,...}`.
/// Empty string represents NULL in CSV exports.
pub fn text_array(v: &Option<Vec<String>>) -> String {
    match v {
        None => String::new(),
        Some(items) if items.is_empty() => String::new(),
        Some(items) => {
            let escaped: Vec<String> = items
                .iter()
                .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
                .collect();
            format!("{{{}}}", escaped.join(","))
        }
    }
}
