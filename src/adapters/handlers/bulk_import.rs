//! Bulk import handler — PostgreSQL COPY FROM STDIN for trusted corpus data.
//!
//! `POST /api/v1/admin/bulk-import/{table}`
//!
//! Uses COPY rather than upsert. Intended for post-wipe corpus releases where
//! tables are empty and the data is trusted (no per-row validation needed).
//! Orders of magnitude faster than the regular upsert import for large tables.
//!
//! The CSV must have a header row. Only the DB columns for the given table are
//! used — extra columns (e.g. author_keys embedded in the bibitem export) are
//! silently dropped, so the caller does not need to pre-filter the CSV.

use hexforge::axum_exports::{Json, Multipart, Path, State};
use hexforge::db_exports::PgPoolCopyExt;
use hexforge::{HexforgeError, ValidationError};
use serde::Serialize;

use crate::AppState;
use crate::adapters::handlers::import::extract_csv_bytes;

// ── Allowed columns per table ─────────────────────────────────────────────────
//
// Column order here determines the column order written to the filtered CSV
// (and therefore what PostgreSQL receives via COPY). id, created_at, updated_at
// are excluded — they have DB-side defaults and must not be overridden.

const AUTHOR_COLS: &[&str] = &[
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
    "famous",
    "name_variants_latex",
    "name_variants_unicode",
];

const JOURNAL_COLS: &[&str] = &[
    "journal_key",
    "name_latex",
    "name_unicode",
    "issn_print",
    "issn_electronic",
];

const PUBLISHER_COLS: &[&str] = &[
    "publisher_key",
    "name_latex",
    "name_unicode",
    "default_address",
];

const INSTITUTION_COLS: &[&str] = &[
    "institution_key",
    "name_latex",
    "name_unicode",
    "default_address",
];

const SCHOOL_COLS: &[&str] = &["school_key", "name_latex", "name_unicode"];

const SERIES_COLS: &[&str] = &["series_key", "name_latex", "name_unicode"];

const KEYWORD_COLS: &[&str] = &["keyword_key", "name", "level"];

// Matches IDS_FORMAT_HEADER minus junction columns (author_keys, editor_keys,
// guesteditor_keys, keyword_keys) which are loaded separately via bibitem_authors/keywords.
// Fields absent from the snapshot CSV (person_key, date_month/day, date_year_2_*,
// date_is_no_date, has_fulltext, fulltext_path) are all nullable/have DB defaults.
const BIBITEM_COLS: &[&str] = &[
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
    "license",
];

const BIBITEM_AUTHORS_COLS: &[&str] = &[
    "bibkey",
    "author_key",
    "role",
    "position",
    "name_variant_latex",
    "name_variant_unicode",
];

const BIBITEM_KEYWORDS_COLS: &[&str] = &["bibkey", "keyword_key", "keyword_level"];

const BIBITEM_REFS_COLS: &[&str] = &["source_key", "target_key", "ref_type"];

const BIBITEM_NOTES_COLS: &[&str] = &[
    "bibkey",
    "note_perso",
    "note_stock",
    "note_missing",
    "change_request",
    "dltc_copyediting_note",
    "todo_general",
];

fn allowed_columns(table: &str) -> Option<&'static [&'static str]> {
    match table {
        "authors" => Some(AUTHOR_COLS),
        "journals" => Some(JOURNAL_COLS),
        "publishers" => Some(PUBLISHER_COLS),
        "institutions" => Some(INSTITUTION_COLS),
        "schools" => Some(SCHOOL_COLS),
        "series" => Some(SERIES_COLS),
        "keywords" => Some(KEYWORD_COLS),
        "bibitems" => Some(BIBITEM_COLS),
        "bibitem_authors" => Some(BIBITEM_AUTHORS_COLS),
        "bibitem_keywords" => Some(BIBITEM_KEYWORDS_COLS),
        "bibitem_refs" => Some(BIBITEM_REFS_COLS),
        "bibitem_notes" => Some(BIBITEM_NOTES_COLS),
        _ => None,
    }
}

#[derive(Serialize)]
pub struct BulkImportResponse {
    pub table: String,
    pub rows: u64,
}

pub async fn bulk_import_table(
    Path(table): Path<String>,
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<BulkImportResponse>, HexforgeError> {
    let cols = allowed_columns(&table).ok_or_else(|| {
        HexforgeError::Validation(ValidationError::custom(format!(
            "bulk-import not supported for table '{table}'"
        )))
    })?;

    let csv_bytes = extract_csv_bytes(multipart).await?;

    let filtered = filter_csv_columns(&csv_bytes, cols).map_err(|e| {
        HexforgeError::Validation(ValidationError::custom(format!("CSV column error: {e}")))
    })?;

    let col_list = cols.join(", ");
    let sql =
        format!("COPY {table} ({col_list}) FROM STDIN WITH (FORMAT CSV, HEADER TRUE, NULL '')");

    let pool = state.pool.pool();
    let mut copy_in = pool
        .copy_in_raw(&sql)
        .await
        .map_err(HexforgeError::data_source)?;
    copy_in
        .send(filtered)
        .await
        .map_err(HexforgeError::data_source)?;
    let rows = copy_in.finish().await.map_err(HexforgeError::data_source)?;

    Ok(Json(BulkImportResponse { table, rows }))
}

/// Rewrite the CSV retaining only the columns in `keep` (in `keep` order).
/// Columns not in `keep` are dropped. All columns in `keep` must be present in
/// the CSV header.
fn filter_csv_columns(input: &[u8], keep: &[&str]) -> Result<Vec<u8>, String> {
    let keep_set: std::collections::HashSet<&str> = keep.iter().copied().collect();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(input);

    let headers = reader.headers().map_err(|e| e.to_string())?.clone();
    let indices: Vec<usize> = headers
        .iter()
        .enumerate()
        .filter_map(|(i, h)| if keep_set.contains(h) { Some(i) } else { None })
        .collect();

    let present: std::collections::HashSet<&str> = indices
        .iter()
        .map(|&i| {
            headers
                .get(i)
                .expect("index derived from headers.iter().enumerate(), always in bounds")
        })
        .collect();
    let missing: Vec<&str> = keep
        .iter()
        .copied()
        .filter(|c| !present.contains(c))
        .collect();
    if !missing.is_empty() {
        return Err(format!("missing required columns: {}", missing.join(", ")));
    }

    // Build index map: column name → position in input
    let idx_map: std::collections::HashMap<&str, usize> =
        headers.iter().enumerate().map(|(i, h)| (h, i)).collect();

    let mut out = Vec::with_capacity(input.len());
    {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(true)
            .from_writer(&mut out);
        writer.write_record(keep).map_err(|e| e.to_string())?;
        for result in reader.records() {
            let record = result.map_err(|e| e.to_string())?;
            let row: Vec<&str> = keep
                .iter()
                .map(|col| idx_map.get(col).and_then(|&i| record.get(i)).unwrap_or(""))
                .collect();
            writer.write_record(&row).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_keeps_specified_columns_in_order() {
        let csv = b"a,b,c,extra\n1,2,3,x\n4,5,6,y\n";
        let result = filter_csv_columns(csv, &["c", "a"]).unwrap();
        let s = String::from_utf8(result).unwrap();
        let mut lines = s.lines();
        assert_eq!(lines.next().unwrap(), "c,a");
        assert_eq!(lines.next().unwrap(), "3,1");
        assert_eq!(lines.next().unwrap(), "6,4");
    }

    #[test]
    fn filter_drops_extra_columns_silently() {
        let csv = b"bibkey,author_keys,title_latex\nk:1,auth1,Title\n";
        let result = filter_csv_columns(csv, &["bibkey", "title_latex"]).unwrap();
        let s = String::from_utf8(result).unwrap();
        assert!(s.contains("bibkey,title_latex"));
        assert!(!s.contains("author_keys"));
        assert!(s.contains("k:1,Title"));
    }

    #[test]
    fn filter_errors_on_missing_required_column() {
        let csv = b"a,b\n1,2\n";
        let err = filter_csv_columns(csv, &["a", "c"]).unwrap_err();
        assert!(err.contains("missing required columns"), "got: {err}");
        assert!(err.contains("c"), "got: {err}");
    }

    #[test]
    fn filter_handles_empty_fields() {
        let csv = b"a,b,c\n1,,3\n";
        let result = filter_csv_columns(csv, &["a", "b", "c"]).unwrap();
        let s = String::from_utf8(result).unwrap();
        assert!(s.contains("1,,3"));
    }
}
