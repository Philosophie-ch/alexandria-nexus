//! ZIP packaging for snapshot data.
//!
//! This is a utility module (not a trait impl), so it may import from
//! other adapter sub-modules like csv_rows.

use std::collections::HashMap;
use std::io::Write;

use hexforge::HexforgeError;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::adapters::csv_rows::{
    bibitems_to_rows, build_author_rows, build_institution_rows, build_journal_rows,
    build_keyword_rows, build_publisher_rows, build_school_rows, build_series_rows,
};
use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow, BibitemRefsRow};
use crate::domain::{Author, BibItem, BibitemNotes};
use crate::logic::export::opt_str;
use crate::process::export::BibitemExportData;
use crate::process::snapshot::SnapshotData;

fn internal_err(msg: impl std::fmt::Display) -> HexforgeError {
    HexforgeError::internal(msg.to_string())
}

fn rows_to_csv_bytes(rows: Vec<Vec<String>>) -> Result<Vec<u8>, HexforgeError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    for row in &rows {
        wtr.write_record(row).map_err(internal_err)?;
    }
    wtr.into_inner().map_err(internal_err)
}

/// 2-char lowercase prefix from a bibkey (`kant:1781` → `"ka"`).
fn bibkey_prefix(bibkey: &str) -> String {
    let stem = bibkey.split(':').next().unwrap_or(bibkey);
    let lower = stem.to_lowercase();
    let mut chars = lower.chars();
    let c1 = chars.next().unwrap_or('_');
    let c2 = chars.next().unwrap_or('_');
    format!("{c1}{c2}")
}

/// 1-char lowercase prefix from an author_key.
fn author_key_prefix(author_key: &str) -> String {
    author_key
        .chars()
        .next()
        .map(|c| c.to_lowercase().to_string())
        .unwrap_or_else(|| "_".to_string())
}

fn build_bibitem_authors_csv(rows: &[&BibitemAuthorsRow]) -> Result<Vec<u8>, HexforgeError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "bibkey",
        "author_key",
        "role",
        "position",
        "name_variant_latex",
        "name_variant_unicode",
    ])
    .map_err(internal_err)?;
    for r in rows {
        wtr.write_record([
            &r.bibkey,
            &r.author_key,
            &r.role.to_string(),
            &r.position.to_string(),
            opt_str(&r.name_variant_latex),
            opt_str(&r.name_variant_unicode),
        ])
        .map_err(internal_err)?;
    }
    wtr.into_inner().map_err(internal_err)
}

fn build_bibitem_keywords_csv(rows: &[&BibitemKeywordsRow]) -> Result<Vec<u8>, HexforgeError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(["bibkey", "keyword_key", "keyword_level"])
        .map_err(internal_err)?;
    for r in rows {
        wtr.write_record([&r.bibkey, &r.keyword_key, &r.keyword_level.to_string()])
            .map_err(internal_err)?;
    }
    wtr.into_inner().map_err(internal_err)
}

fn build_bibitem_refs_csv(rows: &[BibitemRefsRow]) -> Result<Vec<u8>, HexforgeError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(["source_key", "target_key", "ref_type"])
        .map_err(internal_err)?;
    for r in rows {
        wtr.write_record([&r.source_key, &r.target_key, &r.ref_type.to_string()])
            .map_err(internal_err)?;
    }
    wtr.into_inner().map_err(internal_err)
}

fn build_bibitem_notes_csv(rows: &[BibitemNotes]) -> Result<Vec<u8>, HexforgeError> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "bibkey",
        "note_perso",
        "note_stock",
        "note_missing",
        "change_request",
        "dltc_copyediting_note",
        "todo_general",
    ])
    .map_err(internal_err)?;
    for r in rows {
        wtr.write_record([
            &r.bibkey,
            r.note_perso.as_deref().unwrap_or(""),
            r.note_stock.as_deref().unwrap_or(""),
            r.note_missing.as_deref().unwrap_or(""),
            r.change_request.as_deref().unwrap_or(""),
            r.dltc_copyediting_note.as_deref().unwrap_or(""),
            r.todo_general.as_deref().unwrap_or(""),
        ])
        .map_err(internal_err)?;
    }
    wtr.into_inner().map_err(internal_err)
}

/// Package all snapshot data into a ZIP archive.
///
/// Directory structure mirrors the data repo layout:
/// - `author/{x}.csv` — split by first char of author_key
/// - `bibitem/{xy}.csv` — split by 2-char prefix of bibkey
/// - `{small_table}/all.csv` — journal, publisher, institution, school, series, keyword
/// - `bibitem_authors/{xy}.csv` — split by bibitem's bibkey prefix
/// - `bibitem_keywords/{xy}.csv` — same split
/// - `bibitem_refs/all.csv`
/// - `bibitem_notes/all.csv`
pub fn build_snapshot_zip(data: SnapshotData) -> Result<Vec<u8>, HexforgeError> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // ── Small tables (single file each) ───────────────────────────────────────
    let small: &[(&str, Vec<u8>)] = &[
        (
            "journal/all.csv",
            rows_to_csv_bytes(build_journal_rows(&data.journals))?,
        ),
        (
            "publisher/all.csv",
            rows_to_csv_bytes(build_publisher_rows(&data.publishers))?,
        ),
        (
            "institution/all.csv",
            rows_to_csv_bytes(build_institution_rows(&data.institutions))?,
        ),
        (
            "school/all.csv",
            rows_to_csv_bytes(build_school_rows(&data.schools))?,
        ),
        (
            "series/all.csv",
            rows_to_csv_bytes(build_series_rows(&data.series))?,
        ),
        (
            "keyword/all.csv",
            rows_to_csv_bytes(build_keyword_rows(&data.keywords))?,
        ),
        (
            "bibitem_refs/all.csv",
            build_bibitem_refs_csv(&data.bibitem_refs)?,
        ),
        (
            "bibitem_notes/all.csv",
            build_bibitem_notes_csv(&data.bibitem_notes)?,
        ),
    ];

    for (path, bytes) in small {
        zip.start_file(*path, opts).map_err(internal_err)?;
        zip.write_all(bytes).map_err(internal_err)?;
    }

    // ── Authors — split by 1-char prefix ──────────────────────────────────────
    let mut authors_by_prefix: HashMap<String, Vec<&Author>> = HashMap::new();
    for a in &data.authors {
        authors_by_prefix
            .entry(author_key_prefix(&a.author_key))
            .or_default()
            .push(a);
    }
    let mut author_prefixes: Vec<&String> = authors_by_prefix.keys().collect();
    author_prefixes.sort();
    for prefix in author_prefixes {
        let rows = authors_by_prefix[prefix].as_slice();
        let owned: Vec<Author> = rows.iter().map(|a| (*a).clone()).collect();
        let bytes = rows_to_csv_bytes(build_author_rows(&owned))?;
        let path = format!("author/{prefix}.csv");
        zip.start_file(&path, opts).map_err(internal_err)?;
        zip.write_all(&bytes).map_err(internal_err)?;
    }

    // ── Bibitems — split by 2-char prefix ─────────────────────────────────────
    let mut bibitems_by_prefix: HashMap<String, Vec<&BibItem>> = HashMap::new();
    for b in &data.bibitems {
        bibitems_by_prefix
            .entry(bibkey_prefix(&b.bibkey))
            .or_default()
            .push(b);
    }
    let mut ba_by_prefix: HashMap<String, Vec<&BibitemAuthorsRow>> = HashMap::new();
    for r in &data.bibitem_authors {
        ba_by_prefix
            .entry(bibkey_prefix(&r.bibkey))
            .or_default()
            .push(r);
    }
    let mut bk_by_prefix: HashMap<String, Vec<&BibitemKeywordsRow>> = HashMap::new();
    for r in &data.bibitem_keywords {
        bk_by_prefix
            .entry(bibkey_prefix(&r.bibkey))
            .or_default()
            .push(r);
    }

    let mut bib_prefixes: Vec<&String> = bibitems_by_prefix.keys().collect();
    bib_prefixes.sort();
    for prefix in bib_prefixes {
        let bib_rows = bibitems_by_prefix[prefix].as_slice();
        let owned: Vec<BibItem> = bib_rows.iter().map(|b| (*b).clone()).collect();
        let author_rows = ba_by_prefix
            .get(prefix)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let kw_rows = bk_by_prefix
            .get(prefix)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let author_owned: Vec<BibitemAuthorsRow> =
            author_rows.iter().map(|r| (*r).clone()).collect();
        let kw_owned: Vec<BibitemKeywordsRow> = kw_rows.iter().map(|r| (*r).clone()).collect();

        let bib_bytes = rows_to_csv_bytes(bibitems_to_rows(BibitemExportData::Ids {
            bibitems: owned,
            author_rows: author_owned,
            keyword_rows: kw_owned,
        }))?;
        let bib_path = format!("bibitem/{prefix}.csv");
        zip.start_file(&bib_path, opts).map_err(internal_err)?;
        zip.write_all(&bib_bytes).map_err(internal_err)?;

        let ba_bytes = build_bibitem_authors_csv(author_rows)?;
        let ba_path = format!("bibitem_authors/{prefix}.csv");
        zip.start_file(&ba_path, opts).map_err(internal_err)?;
        zip.write_all(&ba_bytes).map_err(internal_err)?;

        let bk_bytes = build_bibitem_keywords_csv(kw_rows)?;
        let bk_path = format!("bibitem_keywords/{prefix}.csv");
        zip.start_file(&bk_path, opts).map_err(internal_err)?;
        zip.write_all(&bk_bytes).map_err(internal_err)?;
    }

    let cursor = zip.finish().map_err(internal_err)?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use chrono::Utc;

    use super::*;
    use crate::domain::{BibItem, EntryType};
    use crate::process::snapshot::SnapshotData;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn empty_snapshot() -> SnapshotData {
        SnapshotData {
            authors: vec![],
            journals: vec![],
            publishers: vec![],
            institutions: vec![],
            schools: vec![],
            series: vec![],
            keywords: vec![],
            bibitems: vec![],
            bibitem_authors: vec![],
            bibitem_keywords: vec![],
            bibitem_refs: vec![],
            bibitem_notes: vec![],
        }
    }

    fn make_author(id: i64, author_key: &str, family_name: &str) -> Author {
        Author {
            id,
            author_key: author_key.to_string(),
            family_name_latex: Some(family_name.to_string()),
            family_name_unicode: Some(family_name.to_string()),
            given_name_latex: None,
            given_name_unicode: None,
            mononym_latex: None,
            mononym_unicode: None,
            shorthand_latex: None,
            shorthand_unicode: None,
            famous_name_latex: None,
            famous_name_unicode: None,
            famous: false,
            name_variants_latex: None,
            name_variants_unicode: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_bibitem(id: i64, bibkey: &str) -> BibItem {
        BibItem {
            id,
            bibkey: bibkey.to_string(),
            entry_type: EntryType::Book,
            title_latex: "Title".to_string(),
            title_unicode: None,
            date_year: None,
            date_year_2_hyphen: None,
            date_year_2_slash: None,
            date_month: None,
            date_day: None,
            date_is_no_date: false,
            pubstate: None,
            booktitle_latex: None,
            booktitle_unicode: None,
            journal_key: None,
            publisher_key: None,
            address: None,
            volume: None,
            number: None,
            pages: None,
            eid: None,
            series_key: None,
            edition: None,
            institution_key: None,
            school_key: None,
            type_field: None,
            doi: None,
            url: None,
            eprint: None,
            urn: None,
            crossref: None,
            issuetitle_latex: None,
            issuetitle_unicode: None,
            note_latex: None,
            note_unicode: None,
            extra_note_latex: None,
            extra_note_unicode: None,
            langid: None,
            is_translation: false,
            epoch: None,
            options: None,
            shorthand: None,
            person_key: None,
            has_fulltext: false,
            fulltext_path: None,
            license: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn zip_file_names(bytes: &[u8]) -> Vec<String> {
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        names.sort();
        names
    }

    fn zip_file_content(bytes: &[u8], path: &str) -> String {
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut file = archive
            .by_name(path)
            .unwrap_or_else(|_| panic!("'{path}' not found in ZIP"));
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        content
    }

    // ── Prefix helpers ────────────────────────────────────────────────────────

    #[test]
    fn bibkey_prefix_two_char() {
        assert_eq!(bibkey_prefix("kant:1781"), "ka");
        assert_eq!(bibkey_prefix("aristotle:350"), "ar");
        assert_eq!(bibkey_prefix("AB:2020"), "ab");
    }

    #[test]
    fn bibkey_prefix_short_stem() {
        assert_eq!(bibkey_prefix("a:2020"), "a_");
        assert_eq!(bibkey_prefix(":2020"), "__");
    }

    #[test]
    fn bibkey_prefix_no_colon() {
        assert_eq!(bibkey_prefix("abc"), "ab");
    }

    #[test]
    fn author_key_prefix_first_char() {
        assert_eq!(author_key_prefix("kant"), "k");
        assert_eq!(author_key_prefix("Aristotle"), "a");
    }

    #[test]
    fn author_key_prefix_empty() {
        assert_eq!(author_key_prefix(""), "_");
    }

    // ── ZIP structure ─────────────────────────────────────────────────────────

    #[test]
    fn empty_snapshot_produces_valid_zip() {
        let bytes = build_snapshot_zip(empty_snapshot()).unwrap();
        let names = zip_file_names(&bytes);
        for table in &[
            "journal",
            "publisher",
            "institution",
            "school",
            "series",
            "keyword",
        ] {
            assert!(
                names.contains(&format!("{table}/all.csv")),
                "missing {table}/all.csv"
            );
        }
        assert!(names.contains(&"bibitem_refs/all.csv".to_string()));
        assert!(names.contains(&"bibitem_notes/all.csv".to_string()));
        assert_eq!(names.len(), 8);
    }

    #[test]
    fn snapshot_splits_authors_by_prefix() {
        let mut data = empty_snapshot();
        data.authors = vec![
            make_author(1, "kant", "Kant"),
            make_author(2, "aristotle", "Aristotle"),
        ];
        let bytes = build_snapshot_zip(data).unwrap();
        let names = zip_file_names(&bytes);
        assert!(names.contains(&"author/k.csv".to_string()));
        assert!(names.contains(&"author/a.csv".to_string()));
    }

    #[test]
    fn snapshot_splits_bibitems_by_prefix() {
        let mut data = empty_snapshot();
        data.bibitems = vec![
            make_bibitem(1, "kant:1781"),
            make_bibitem(2, "aristotle:350"),
        ];
        let bytes = build_snapshot_zip(data).unwrap();
        let names = zip_file_names(&bytes);
        assert!(names.contains(&"bibitem/ka.csv".to_string()));
        assert!(names.contains(&"bibitem/ar.csv".to_string()));
    }

    #[test]
    fn snapshot_junction_files_follow_bibitem_prefix() {
        let mut data = empty_snapshot();
        data.bibitems = vec![make_bibitem(1, "kant:1781")];
        let bytes = build_snapshot_zip(data).unwrap();
        let names = zip_file_names(&bytes);
        assert!(names.contains(&"bibitem_authors/ka.csv".to_string()));
        assert!(names.contains(&"bibitem_keywords/ka.csv".to_string()));
    }

    // ── CSV content ───────────────────────────────────────────────────────────

    #[test]
    fn snapshot_bibitem_csv_has_ids_format_headers() {
        let mut data = empty_snapshot();
        data.bibitems = vec![make_bibitem(1, "kant:1781")];
        let bytes = build_snapshot_zip(data).unwrap();
        let csv = zip_file_content(&bytes, "bibitem/ka.csv");
        let header = csv.lines().next().unwrap();
        assert!(header.contains("bibkey"), "missing bibkey column");
        assert!(header.contains("entry_type"), "missing entry_type column");
        assert!(header.contains("author_keys"), "missing author_keys column");
        assert!(
            header.contains("keyword_keys"),
            "missing keyword_keys column"
        );
    }

    #[test]
    fn snapshot_bibitem_csv_contains_data_row() {
        let mut data = empty_snapshot();
        data.bibitems = vec![make_bibitem(42, "kant:1781")];
        let bytes = build_snapshot_zip(data).unwrap();
        let csv = zip_file_content(&bytes, "bibitem/ka.csv");
        assert!(csv.contains("kant:1781"), "bibkey not found in CSV");
        assert!(csv.contains("42"), "id not found in CSV");
        assert!(csv.contains("book"), "entry_type not found in CSV");
    }

    #[test]
    fn snapshot_author_csv_has_correct_headers_and_content() {
        let mut data = empty_snapshot();
        data.authors = vec![make_author(7, "kant", "Kant")];
        let bytes = build_snapshot_zip(data).unwrap();
        let csv = zip_file_content(&bytes, "author/k.csv");
        let header = csv.lines().next().unwrap();
        assert!(header.contains("author_key"), "missing author_key column");
        assert!(
            header.contains("family_name_latex"),
            "missing family_name_latex column"
        );
        assert!(csv.contains("kant"), "author_key not found in CSV");
        assert!(csv.contains("Kant"), "family_name not found in CSV");
    }

    #[test]
    fn snapshot_bibitem_authors_csv_has_correct_headers() {
        let mut data = empty_snapshot();
        data.bibitems = vec![make_bibitem(1, "kant:1781")];
        data.bibitem_authors = vec![BibitemAuthorsRow {
            bibkey: "kant:1781".to_string(),
            author_key: "kant".to_string(),
            role: crate::domain::AuthorRole::Author,
            position: 1,
            name_variant_latex: None,
            name_variant_unicode: None,
        }];
        let bytes = build_snapshot_zip(data).unwrap();
        let csv = zip_file_content(&bytes, "bibitem_authors/ka.csv");
        let header = csv.lines().next().unwrap();
        assert!(header.contains("bibkey"));
        assert!(header.contains("author_key"));
        assert!(header.contains("role"));
        assert!(header.contains("position"));
        assert!(csv.contains("kant"), "author_key not found in CSV");
        assert!(csv.contains("author"), "role not found in CSV");
    }
}
