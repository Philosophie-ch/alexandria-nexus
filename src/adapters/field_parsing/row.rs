use std::collections::HashMap;
use std::str::FromStr;

use crate::domain::{Epoch, LangId, License, PubState};
use crate::logic::full_import::{
    FieldError, ParsedAuthor, ParsedBibRow, ParsedDate, ParsedKeywords, RowParseResult,
};

use super::author::{parse_authors, parse_person};
use super::bibkey::parse_bibkey;
use super::date::parse_date;
use super::entry_type::parse_entry_type;
use super::keywords::parse_keyword_list;
use super::pages::parse_pages;

// =============================================================================
// CSV header handling
// =============================================================================

/// Column index mapping with header normalization (hyphens → underscores).
pub struct CsvHeaders {
    map: HashMap<String, usize>,
}

impl CsvHeaders {
    pub fn from_headers(headers: &[&str]) -> Self {
        let mut map = HashMap::new();
        for (i, field) in headers.iter().enumerate() {
            let normalized = normalize_header(field);
            map.insert(normalized, i);
        }
        CsvHeaders { map }
    }

    pub fn get(&self, name: &str) -> Option<usize> {
        self.map.get(name).copied()
    }
}

/// Normalize a CSV header: trim, replace hyphens with underscores.
pub fn normalize_header(header: &str) -> String {
    header.trim().replace('-', "_")
}

// =============================================================================
// Field extraction helpers
// =============================================================================

fn get_field(record: &[&str], headers: &CsvHeaders, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|idx| record.get(idx))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn get_field_raw(record: &[&str], headers: &CsvHeaders, name: &str) -> String {
    headers
        .get(name)
        .and_then(|idx| record.get(idx))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

// =============================================================================
// Row parser
// =============================================================================

/// Parse a single row (as a slice of field strings) into a [`ParsedBibRow`].
pub fn parse_row(headers: &CsvHeaders, record: &[&str]) -> RowParseResult {
    let mut errors: Vec<FieldError> = Vec::new();

    // --- Required: bibkey ---
    let bibkey_raw = get_field(record, headers, "bibkey");
    let bibkey = match bibkey_raw {
        Some(raw) => {
            if let Err(e) = parse_bibkey(&raw) {
                errors.push(FieldError {
                    field: "bibkey".to_string(),
                    error: e,
                });
            }
            raw
        }
        None => {
            errors.push(FieldError {
                field: "bibkey".to_string(),
                error: "missing required field".to_string(),
            });
            return RowParseResult::Err {
                bibkey: None,
                errors,
            };
        }
    };

    // --- Required: entry_type ---
    let entry_type = parse_entry_type(&get_field_raw(record, headers, "entry_type"));

    // --- Required: title ---
    let title = match get_field(record, headers, "title") {
        Some(t) => t,
        None => {
            errors.push(FieldError {
                field: "title".to_string(),
                error: "missing required field".to_string(),
            });
            String::new()
        }
    };

    // --- Authors / editors / guesteditors ---
    let authors = parse_people_field(record, headers, "author", &mut errors);
    let editors = parse_people_field(record, headers, "editor", &mut errors);
    let guesteditors = parse_people_field(record, headers, "_guesteditor", &mut errors);

    // --- Person ---
    let person = match parse_person(&get_field_raw(record, headers, "_person")) {
        Ok(p) => p,
        Err(e) => {
            errors.push(FieldError {
                field: "_person".to_string(),
                error: e,
            });
            None
        }
    };

    // --- Date ---
    let date = match parse_date(&get_field_raw(record, headers, "date")) {
        Ok(d) => d,
        Err(e) => {
            errors.push(FieldError {
                field: "date".to_string(),
                error: e,
            });
            ParsedDate::NoDate
        }
    };

    // --- Pubstate (default to None on invalid) ---
    let pubstate = get_field(record, headers, "pubstate").and_then(|s| PubState::from_str(&s).ok());

    // --- Pages ---
    let pages = match parse_pages(&get_field_raw(record, headers, "pages")) {
        Ok(p) => p,
        Err(e) => {
            errors.push(FieldError {
                field: "pages".to_string(),
                error: e,
            });
            None
        }
    };

    // --- Keywords ---
    let keywords = ParsedKeywords {
        level_1: parse_keyword_list(&get_field_raw(record, headers, "_kw_level1")),
        level_2: parse_keyword_list(&get_field_raw(record, headers, "_kw_level2")),
        level_3: parse_keyword_list(&get_field_raw(record, headers, "_kw_level3")),
    };

    // --- Entity name references ---
    let journal_name = get_field(record, headers, "journal");
    let publisher_name = get_field(record, headers, "publisher");
    let institution_name = get_field(record, headers, "institution");
    let school_name = get_field(record, headers, "school");
    let series_name = get_field(record, headers, "series");

    // --- Crossref ---
    let crossref_bibkey = match get_field(record, headers, "crossref") {
        Some(ref raw) => match parse_bibkey(raw) {
            Ok(_) => Some(raw.clone()),
            Err(e) => {
                errors.push(FieldError {
                    field: "crossref".to_string(),
                    error: e,
                });
                None
            }
        },
        None => None,
    };

    // --- Metadata enums (default to None on invalid) ---
    let epoch = get_field(record, headers, "_epoch").and_then(|s| Epoch::from_str(&s).ok());
    let langid = get_field(record, headers, "_langid").and_then(|s| LangId::from_str(&s).ok());

    // --- Booleans ---
    let is_translation = get_field(record, headers, "_lang_der").is_some();
    let has_fulltext = get_field(record, headers, "_has_link_to_full_text").is_some();
    let license = get_field(record, headers, "_license").and_then(|s| License::from_str(&s).ok());

    // --- Simple string fields ---
    let booktitle = get_field(record, headers, "booktitle");
    let volume = get_field(record, headers, "volume");
    let number = get_field(record, headers, "number");
    let eid = get_field(record, headers, "eid");
    let address = get_field(record, headers, "address");
    let type_field = get_field(record, headers, "type");
    let edition = get_field(record, headers, "edition");
    let note = get_field(record, headers, "note");
    let issuetitle = get_field(record, headers, "_issuetitle");
    let extra_note = get_field(record, headers, "_extra_note");
    let shorthand = get_field(record, headers, "shorthand");
    let note_perso = get_field(record, headers, "_note_perso");
    let note_stock = get_field(record, headers, "_note_stock");
    let note_missing = get_field(record, headers, "_note_missing");
    let change_request = get_field(record, headers, "_change_request");
    let dltc_copyediting_note = get_field(record, headers, "_dltc_copyediting_note");
    let todo_general = get_field(record, headers, "_to_do_general");
    let options = get_field(record, headers, "options");
    let doi = get_field(record, headers, "doi");
    let url = get_field(record, headers, "url");
    let eprint = get_field(record, headers, "eprint");
    let urn = get_field(record, headers, "urn");

    // --- Return ---
    if !errors.is_empty() {
        return RowParseResult::Err {
            bibkey: Some(bibkey),
            errors,
        };
    }

    RowParseResult::Ok(Box::new(ParsedBibRow {
        bibkey,
        entry_type,
        authors,
        editors,
        guesteditors,
        person,
        date,
        pubstate,
        title,
        booktitle,
        journal_name,
        publisher_name,
        institution_name,
        school_name,
        series_name,
        crossref_bibkey,
        keywords,
        volume,
        number,
        pages,
        eid,
        address,
        type_field,
        edition,
        note,
        issuetitle,
        extra_note,
        shorthand,
        note_perso,
        note_stock,
        note_missing,
        change_request,
        dltc_copyediting_note,
        todo_general,
        options,
        doi,
        url,
        eprint,
        urn,
        epoch,
        langid,
        is_translation,
        has_fulltext,
        license,
    }))
}

// =============================================================================
// Helpers
// =============================================================================

fn parse_people_field(
    record: &[&str],
    headers: &CsvHeaders,
    field_name: &str,
    errors: &mut Vec<FieldError>,
) -> Vec<ParsedAuthor> {
    let raw = get_field_raw(record, headers, field_name);
    if raw.is_empty() {
        return vec![];
    }
    match parse_authors(&raw) {
        Ok(authors) => authors,
        Err(e) => {
            errors.push(FieldError {
                field: field_name.to_string(),
                error: e,
            });
            vec![]
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EntryType;

    fn run_parse(headers: &str, row: &str) -> RowParseResult {
        let data = format!("{headers}\n{row}");
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(data.as_bytes());
        let raw_headers = rdr.headers().unwrap().clone();
        let header_fields: Vec<&str> = raw_headers.iter().collect();
        let h = CsvHeaders::from_headers(&header_fields);
        let record = rdr.records().next().unwrap().unwrap();
        let fields: Vec<&str> = record.iter().collect();
        parse_row(&h, &fields)
    }

    #[test]
    fn full_valid_row() {
        let result = run_parse(
            "entry_type,bibkey,author,editor,date,title,journal,pages,_kw-level1,pubstate,_epoch,_langid",
            "@article{,kant:1781,\"Kant, Immanuel\",,1781,Critique of Pure Reason,Mind,123--456,epistemology; metaphysics,published,enlightenment,english",
        );
        match result {
            RowParseResult::Ok(parsed) => {
                assert_eq!(parsed.bibkey, "kant:1781");
                assert_eq!(parsed.entry_type, EntryType::Article);
                assert_eq!(parsed.title, "Critique of Pure Reason");
                assert_eq!(parsed.authors.len(), 1);
                assert_eq!(parsed.authors[0].display_name(), "Kant, Immanuel");
                assert_eq!(parsed.journal_name.as_deref(), Some("Mind"));
                assert_eq!(parsed.pages.as_deref(), Some("123--456"));
                assert_eq!(parsed.keywords.level_1, vec!["epistemology", "metaphysics"]);
                assert_eq!(parsed.pubstate, Some(PubState::Published));
                assert_eq!(parsed.epoch, Some(Epoch::Enlightenment));
                assert_eq!(parsed.langid, Some(LangId::English));
            }
            RowParseResult::Err { errors, .. } => {
                panic!("expected Ok, got errors: {errors:?}");
            }
        }
    }

    #[test]
    fn missing_bibkey() {
        match run_parse("entry_type,bibkey,title", "@book{,,Some Title") {
            RowParseResult::Err { errors, .. } => {
                assert!(errors.iter().any(|e| e.field == "bibkey"));
            }
            RowParseResult::Ok(_) => panic!("expected error for missing bibkey"),
        }
    }

    #[test]
    fn invalid_date_only() {
        match run_parse(
            "entry_type,bibkey,title,date",
            "@book{,kant:1781,Some Title,not-valid-date",
        ) {
            RowParseResult::Err { errors, .. } => {
                assert!(errors.iter().any(|e| e.field == "date"));
                assert!(!errors.iter().any(|e| e.field == "bibkey"));
                assert!(!errors.iter().any(|e| e.field == "title"));
            }
            RowParseResult::Ok(_) => panic!("expected error for invalid date"),
        }
    }

    #[test]
    fn invalid_pages_single_hyphen() {
        match run_parse(
            "entry_type,bibkey,title,pages",
            "@book{,kant:1781,Some Title,123-456",
        ) {
            RowParseResult::Err { errors, .. } => {
                assert!(errors.iter().any(|e| e.field == "pages"));
            }
            RowParseResult::Ok(_) => panic!("expected error for single-hyphen pages"),
        }
    }

    #[test]
    fn header_normalization() {
        match run_parse(
            "_kw-level1,_kw-level2,_kw-level3,entry_type,bibkey,title",
            "epistemology,logic,,@book{,kant:1781,Title",
        ) {
            RowParseResult::Ok(parsed) => {
                assert_eq!(parsed.keywords.level_1, vec!["epistemology"]);
                assert_eq!(parsed.keywords.level_2, vec!["logic"]);
                assert!(parsed.keywords.level_3.is_empty());
            }
            RowParseResult::Err { errors, .. } => {
                panic!("expected Ok, got errors: {errors:?}");
            }
        }
    }

    #[test]
    fn boolean_fields() {
        match run_parse(
            "entry_type,bibkey,title,_lang-der,_has-link-to-full-text",
            "@book{,kant:1781,Title,x,yes",
        ) {
            RowParseResult::Ok(parsed) => {
                assert!(parsed.is_translation);
                assert!(parsed.has_fulltext);
            }
            RowParseResult::Err { errors, .. } => {
                panic!("expected Ok, got errors: {errors:?}");
            }
        }
    }

    #[test]
    fn boolean_fields_empty() {
        match run_parse(
            "entry_type,bibkey,title,_lang-der,_has-link-to-full-text",
            "@book{,kant:1781,Title,,",
        ) {
            RowParseResult::Ok(parsed) => {
                assert!(!parsed.is_translation);
                assert!(!parsed.has_fulltext);
            }
            RowParseResult::Err { errors, .. } => {
                panic!("expected Ok, got errors: {errors:?}");
            }
        }
    }

    #[test]
    fn invalid_pubstate_defaults_to_none() {
        match run_parse(
            "entry_type,bibkey,title,pubstate",
            "@book{,kant:1781,Title,invalid_state",
        ) {
            RowParseResult::Ok(parsed) => {
                assert_eq!(parsed.pubstate, None);
            }
            RowParseResult::Err { errors, .. } => {
                panic!("expected Ok, got errors: {errors:?}");
            }
        }
    }

    #[test]
    fn crossref_validation() {
        match run_parse(
            "entry_type,bibkey,title,crossref",
            "@incollection{,smith:2024,Chapter Title,kant:1781",
        ) {
            RowParseResult::Ok(parsed) => {
                assert_eq!(parsed.crossref_bibkey.as_deref(), Some("kant:1781"));
            }
            RowParseResult::Err { errors, .. } => {
                panic!("expected Ok, got errors: {errors:?}");
            }
        }
    }

    #[test]
    fn unknown_columns_ignored() {
        match run_parse(
            "entry_type,bibkey,title,_further_refs,_depends_on",
            "@book{,kant:1781,Title,\"smith:2024, plato:-380\",other:2000",
        ) {
            RowParseResult::Ok(parsed) => {
                assert_eq!(parsed.bibkey, "kant:1781");
            }
            RowParseResult::Err { errors, .. } => {
                panic!("expected Ok, got errors: {errors:?}");
            }
        }
    }
}
