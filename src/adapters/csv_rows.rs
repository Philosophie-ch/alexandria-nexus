//! CSV row serialization for entity and bibitem exports.
//!
//! All functions here are pure but format-specific (CSV/text-array output).
//! They live in adapters because serializing to an external format is a
//! boundary concern — exactly like SQL or JSON.

use std::collections::HashMap;

use hexforge::HexforgeError;

use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow};
use crate::domain::{
    Author, AuthorRole, BibItem, BibitemNotes, Institution, Journal, Keyword, Publisher, School,
    Series,
};
use crate::logic::export::{
    format_keywords_at_level, format_role_ids, format_role_names, opt_display, opt_i16, opt_i32,
    opt_str,
};
use crate::logic::full_import::ExportContext;
use crate::process::export::{BibitemExpandedData, BibitemExportData};
use crate::process::full_import::FullExportData;

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
    "volume_numeric",
    "number",
    "number_numeric",
    "pages",
    "start_page",
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
    "license",
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
    "volume_numeric",
    "number",
    "number_numeric",
    "pages",
    "start_page",
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
    "license",
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
            opt_i32(bib.volume_numeric),
            opt_str(&bib.number).to_string(),
            opt_i32(bib.number_numeric),
            opt_str(&bib.pages).to_string(),
            opt_i32(bib.start_page),
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
            opt_str(&bib.license).to_string(),
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
            opt_i32(bib.volume_numeric),
            opt_str(&bib.number).to_string(),
            opt_i32(bib.number_numeric),
            opt_str(&bib.pages).to_string(),
            opt_i32(bib.start_page),
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
            opt_str(&bib.license).to_string(),
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

// ── Full-CSV export ───────────────────────────────────────────────────────────

pub const FULL_CSV_HEADERS: &str = "entry_type,bibkey,author,editor,_guesteditor,date,pubstate,title,\
booktitle,journal,publisher,institution,school,series,volume,number,pages,eid,address,type,edition,\
note,_issuetitle,_extra_note,crossref,\
_kw_level1,_kw_level2,_kw_level3,_epoch,_langid,_lang_der,_person,\
_has_link_to_full_text,shorthand,options,doi,url,eprint,urn,_license,\
_note-perso,_note-stock,_note-missing,_change-request,_dltc_copyediting_note,_to-do-general";

/// Serialise all bibitems from `data` into a UTF-8 CSV byte vector.
///
/// The column order is defined by [`FULL_CSV_HEADERS`].
pub fn full_csv_to_bytes(data: &FullExportData) -> Result<Vec<u8>, HexforgeError> {
    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record(FULL_CSV_HEADERS.split(','))
        .map_err(|e| HexforgeError::internal(format!("CSV header error: {e}")))?;
    for bib in &data.bibitems {
        let row = build_export_record(bib, &data.context);
        wtr.write_record(&row)
            .map_err(|e| HexforgeError::internal(format!("CSV write error: {e}")))?;
    }
    wtr.into_inner()
        .map_err(|e| HexforgeError::internal(format!("CSV flush error: {e}")))
}

/// Serialise a single `BibItem` into one CSV row using pre-resolved name maps.
///
/// Column order matches [`FULL_CSV_HEADERS`].
pub fn build_export_record(bib: &BibItem, ctx: &ExportContext) -> Vec<String> {
    let authors_for_role = |role: AuthorRole| -> String {
        ctx.authors_by_bib
            .get(&bib.bibkey)
            .map(|rows| {
                let mut filtered: Vec<&BibitemAuthorsRow> =
                    rows.iter().filter(|r| r.role == role).collect();
                filtered.sort_by_key(|r| r.position);
                filtered
                    .iter()
                    .filter_map(|r| ctx.author_names.get(&r.author_key))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" and ")
            })
            .unwrap_or_default()
    };

    let keywords_for_level = |level: i16| -> String {
        ctx.keywords_by_bib
            .get(&bib.bibkey)
            .map(|rows| {
                let names: Vec<&str> = rows
                    .iter()
                    .filter_map(|r| {
                        ctx.keyword_names
                            .get(&r.keyword_key)
                            .and_then(|(name, lv)| {
                                if *lv == level {
                                    Some(name.as_str())
                                } else {
                                    None
                                }
                            })
                    })
                    .collect();
                if names.is_empty() {
                    String::new()
                } else {
                    format!("{};", names.join("; "))
                }
            })
            .unwrap_or_default()
    };

    let note = |f: fn(&BibitemNotes) -> Option<String>| -> String {
        ctx.notes_by_bib
            .get(&bib.bibkey)
            .and_then(&f)
            .unwrap_or_default()
    };

    let date = format_date_for_export(bib);
    let crossref = bib.crossref.clone().unwrap_or_default();
    let person = bib
        .person_key
        .as_deref()
        .and_then(|k| ctx.author_names.get(k))
        .map(|n| format!("{n};"))
        .unwrap_or_default();

    vec![
        bib.entry_type.to_string(),
        bib.bibkey.clone(),
        authors_for_role(AuthorRole::Author),
        authors_for_role(AuthorRole::Editor),
        authors_for_role(AuthorRole::Guesteditor),
        date,
        bib.pubstate.map(|p| p.to_string()).unwrap_or_default(),
        bib.title_latex.clone(),
        bib.booktitle_latex.clone().unwrap_or_default(),
        bib.journal_key
            .as_deref()
            .and_then(|k| ctx.journal_names.get(k))
            .cloned()
            .unwrap_or_default(),
        bib.publisher_key
            .as_deref()
            .and_then(|k| ctx.publisher_names.get(k))
            .cloned()
            .unwrap_or_default(),
        bib.institution_key
            .as_deref()
            .and_then(|k| ctx.institution_names.get(k))
            .cloned()
            .unwrap_or_default(),
        bib.school_key
            .as_deref()
            .and_then(|k| ctx.school_names.get(k))
            .cloned()
            .unwrap_or_default(),
        bib.series_key
            .as_deref()
            .and_then(|k| ctx.series_names.get(k))
            .cloned()
            .unwrap_or_default(),
        bib.volume.clone().unwrap_or_default(),
        bib.number.clone().unwrap_or_default(),
        bib.pages.clone().unwrap_or_default(),
        bib.eid.clone().unwrap_or_default(),
        bib.address.clone().unwrap_or_default(),
        bib.type_field.clone().unwrap_or_default(),
        bib.edition.clone().unwrap_or_default(),
        bib.note_latex.clone().unwrap_or_default(),
        bib.issuetitle_latex.clone().unwrap_or_default(),
        bib.extra_note_latex.clone().unwrap_or_default(),
        crossref,
        keywords_for_level(1),
        keywords_for_level(2),
        keywords_for_level(3),
        bib.epoch.map(|e| e.to_string()).unwrap_or_default(),
        bib.langid.map(|l| l.to_string()).unwrap_or_default(),
        if bib.is_translation {
            "x".to_string()
        } else {
            String::new()
        },
        person,
        if bib.has_fulltext {
            "x".to_string()
        } else {
            String::new()
        },
        bib.shorthand.clone().unwrap_or_default(),
        bib.options.clone().unwrap_or_default(),
        bib.doi.clone().unwrap_or_default(),
        bib.url.clone().unwrap_or_default(),
        bib.eprint.clone().unwrap_or_default(),
        bib.urn.clone().unwrap_or_default(),
        bib.license.clone().unwrap_or_default(),
        note(|n| n.note_perso.clone()),
        note(|n| n.note_stock.clone()),
        note(|n| n.note_missing.clone()),
        note(|n| n.change_request.clone()),
        note(|n| n.dltc_copyediting_note.clone()),
        note(|n| n.todo_general.clone()),
    ]
}

fn format_date_for_export(bib: &BibItem) -> String {
    if bib.date_is_no_date {
        return "no date".to_string();
    }
    match (bib.date_year, bib.date_month, bib.date_day) {
        (Some(y), Some(m), Some(d)) => format!("{y}-{m:02}-{d:02}"),
        (Some(y), _, _) => {
            if let Some(y2) = bib.date_year_2_hyphen {
                format!("{y}-{y2}")
            } else if let Some(y2) = bib.date_year_2_slash {
                format!("{y}/{y2}")
            } else {
                y.to_string()
            }
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::domain::{BibItem, EntryType};
    use crate::process::full_import::FullExportData;

    fn empty_context() -> ExportContext {
        ExportContext {
            author_names: HashMap::new(),
            journal_names: HashMap::new(),
            publisher_names: HashMap::new(),
            institution_names: HashMap::new(),
            school_names: HashMap::new(),
            series_names: HashMap::new(),
            keyword_names: HashMap::new(),
            bibkey_by_id: HashMap::new(),
            authors_by_bib: HashMap::new(),
            keywords_by_bib: HashMap::new(),
            notes_by_bib: HashMap::new(),
        }
    }

    fn minimal_bib(bibkey: &str) -> BibItem {
        BibItem {
            id: 1,
            bibkey: bibkey.to_string(),
            entry_type: EntryType::Article,
            title_latex: "Test Title".to_string(),
            date_is_no_date: false,
            has_fulltext: false,
            is_translation: false,
            address: None,
            booktitle_latex: None,
            booktitle_unicode: None,
            crossref: None,
            date_day: None,
            date_month: None,
            date_year: None,
            date_year_2_hyphen: None,
            date_year_2_slash: None,
            doi: None,
            edition: None,
            eid: None,
            epoch: None,
            eprint: None,
            extra_note_latex: None,
            extra_note_unicode: None,
            fulltext_path: None,
            institution_key: None,
            issuetitle_latex: None,
            issuetitle_unicode: None,
            journal_key: None,
            langid: None,
            license: None,
            note_latex: None,
            note_unicode: None,
            number: None,
            options: None,
            pages: None,
            start_page: None,
            person_key: None,
            publisher_key: None,
            pubstate: None,
            school_key: None,
            series_key: None,
            shorthand: None,
            title_unicode: None,
            type_field: None,
            url: None,
            urn: None,
            volume: None,
            volume_numeric: None,
            number_numeric: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn full_csv_headers_column_count() {
        let cols: Vec<&str> = FULL_CSV_HEADERS.split(',').collect();
        assert_eq!(cols.len(), 46);
        assert_eq!(cols[0], "entry_type");
        assert_eq!(cols[1], "bibkey");
        assert_eq!(cols[45], "_to-do-general");
    }

    #[test]
    fn full_csv_to_bytes_empty_data_produces_header_only() {
        let data = FullExportData {
            bibitems: vec![],
            context: empty_context(),
        };
        let bytes = full_csv_to_bytes(&data).expect("should not fail on empty data");
        let csv = String::from_utf8(bytes).unwrap();
        let mut lines = csv.lines();
        let header = lines.next().expect("header line");
        assert!(header.starts_with("entry_type,bibkey"));
        assert!(lines.next().is_none(), "no data rows expected");
    }

    #[test]
    fn full_csv_to_bytes_single_row() {
        let data = FullExportData {
            bibitems: vec![minimal_bib("test2024")],
            context: empty_context(),
        };
        let bytes = full_csv_to_bytes(&data).expect("should not fail");
        let csv = String::from_utf8(bytes).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2, "header + one data row");
        assert!(lines[1].contains("test2024"));
        assert!(lines[1].contains("article"));
    }

    #[test]
    fn build_export_record_column_count_matches_headers() {
        let bib = minimal_bib("test-bib");
        let ctx = empty_context();
        let row = build_export_record(&bib, &ctx);
        let col_count = FULL_CSV_HEADERS.split(',').count();
        assert_eq!(
            row.len(),
            col_count,
            "row column count must match header count"
        );
    }

    #[test]
    fn build_export_record_basic_fields() {
        let bib = minimal_bib("smith2023");
        let ctx = empty_context();
        let row = build_export_record(&bib, &ctx);
        assert_eq!(row[0], "article", "entry_type");
        assert_eq!(row[1], "smith2023", "bibkey");
        assert_eq!(row[7], "Test Title", "title");
    }

    #[test]
    fn build_export_record_fulltext_flag() {
        let mut bib = minimal_bib("link-bib");
        bib.has_fulltext = true;
        let row = build_export_record(&bib, &empty_context());
        // _has_link_to_full_text is at index 32 (0-based)
        assert_eq!(row[32], "x");
    }

    #[test]
    fn build_export_record_translation_flag() {
        let mut bib = minimal_bib("trans-bib");
        bib.is_translation = true;
        let row = build_export_record(&bib, &empty_context());
        // _lang_der is at index 30 (0-based)
        assert_eq!(row[30], "x");
    }
}
