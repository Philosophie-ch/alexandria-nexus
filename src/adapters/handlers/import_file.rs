//! File import handler for CSV and spreadsheet uploads.
//!
//! `POST /api/v1/admin/import/file`
//!
//! Accepts multipart form data with a file upload.
//! Supports CSV, ODS, XLSX, and XLS formats.
//! Requires Admin permission.

use axum::extract::Multipart;
use hexforge::axum_exports::{Json, State};
use hexforge::{DataSource, HexforgeError, ValidationError};
use serde::Serialize;

use crate::domain::EntryType;
use crate::domain::{BibItem, CreateBibItem, create_bib_item_transform};
use crate::state::AppState;
use crate::validation::validate_create_bibitem;

/// Response for file import.
#[derive(Debug, Serialize)]
pub struct FileImportResponse {
    /// Number of successfully imported items.
    pub imported: usize,
    /// Number of failed items.
    pub failed: usize,
    /// Parse warnings (rows that had issues but were still parsed).
    pub parse_warnings: Vec<FileImportWarning>,
    /// Import errors (rows that failed to import).
    pub import_errors: Vec<FileImportError>,
    /// Successfully imported items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<BibItem>>,
}

/// Warning from parsing.
#[derive(Debug, Serialize)]
pub struct FileImportWarning {
    pub row: usize,
    pub message: String,
}

/// Error from import.
#[derive(Debug, Serialize)]
pub struct FileImportError {
    pub row: usize,
    pub bibkey: String,
    pub error: String,
}

/// Supported file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileFormat {
    Csv,
    Ods,
    Xlsx,
    Xls,
}

impl FileFormat {
    fn from_content_type(content_type: Option<&str>) -> Option<Self> {
        match content_type {
            Some("text/csv") | Some("application/csv") => Some(Self::Csv),
            Some("application/vnd.oasis.opendocument.spreadsheet") => Some(Self::Ods),
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet") => {
                Some(Self::Xlsx)
            }
            Some("application/vnd.ms-excel") => Some(Self::Xls),
            _ => None,
        }
    }

    fn from_extension(filename: &str) -> Option<Self> {
        let ext = filename.rsplit('.').next()?.to_lowercase();
        match ext.as_str() {
            "csv" => Some(Self::Csv),
            "ods" => Some(Self::Ods),
            "xlsx" => Some(Self::Xlsx),
            "xls" => Some(Self::Xls),
            _ => None,
        }
    }
}

/// Import bibitems from a file upload.
///
/// `POST /api/v1/admin/import/file`
///
/// This handler:
/// 1. Accepts multipart form data with a file field
/// 2. Detects format from content-type or file extension
/// 3. Parses the file into CreateBibItem DTOs
/// 4. Validates and transforms each item
/// 5. Inserts valid items using the DataSource
/// 6. Returns counts and detailed results
pub async fn import_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<FileImportResponse>, HexforgeError> {
    // Find the file field
    let mut file_data: Option<(Vec<u8>, FileFormat)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let content_type = field.content_type().map(|s| s.to_string());
            let filename = field.file_name().unwrap_or("").to_string();

            let data = field
                .bytes()
                .await
                .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?;

            let format = FileFormat::from_content_type(content_type.as_deref())
                .or_else(|| FileFormat::from_extension(&filename))
                .ok_or_else(|| {
                    HexforgeError::Validation(ValidationError::custom(
                        "Unsupported file format. Use CSV, ODS, XLSX, or XLS.",
                    ))
                })?;

            file_data = Some((data.to_vec(), format));
            break;
        }
    }

    let (data, format) = file_data.ok_or_else(|| {
        HexforgeError::Validation(ValidationError::custom(
            "No file field found in request. Send a multipart form with a 'file' field.",
        ))
    })?;

    // Parse the file into rows
    let (parsed_items, parse_warnings) = match format {
        FileFormat::Csv => parse_csv_data(&data)?,
        FileFormat::Ods | FileFormat::Xlsx | FileFormat::Xls => parse_spreadsheet_data(&data)?,
    };

    if parsed_items.is_empty() {
        return Ok(Json(FileImportResponse {
            imported: 0,
            failed: 0,
            parse_warnings,
            import_errors: vec![],
            items: None,
        }));
    }

    // Validate and transform items
    let mut valid_items = Vec::new();
    let mut import_errors = Vec::new();

    for (idx, dto) in parsed_items.iter().enumerate() {
        let row_num = idx + 2; // Row 1 is headers

        if let Err(e) = validate_create_bibitem(dto) {
            import_errors.push(FileImportError {
                row: row_num,
                bibkey: dto.bibkey.clone(),
                error: e.to_string(),
            });
            continue;
        }

        let bibitem = create_bib_item_transform(dto.clone());
        valid_items.push((row_num, dto.bibkey.clone(), bibitem));
    }

    if valid_items.is_empty() {
        return Ok(Json(FileImportResponse {
            imported: 0,
            failed: import_errors.len(),
            parse_warnings,
            import_errors,
            items: None,
        }));
    }

    // Insert valid items
    let mut imported_items = Vec::with_capacity(valid_items.len());

    for (row_num, bibkey, bibitem) in valid_items {
        match state.bibitem_ds.insert(bibitem).await {
            Ok(inserted) => {
                imported_items.push(inserted);
            }
            Err(e) => {
                let error_msg = e.to_string();
                let formatted_msg =
                    if error_msg.contains("duplicate key") || error_msg.contains("23505") {
                        format!("Duplicate bibkey: {bibkey}")
                    } else {
                        error_msg
                    };

                import_errors.push(FileImportError {
                    row: row_num,
                    bibkey,
                    error: formatted_msg,
                });
            }
        }
    }

    Ok(Json(FileImportResponse {
        imported: imported_items.len(),
        failed: import_errors.len(),
        parse_warnings,
        import_errors,
        items: if imported_items.is_empty() {
            None
        } else {
            Some(imported_items)
        },
    }))
}

// ============================================================================
// CSV Parsing
// ============================================================================

fn parse_csv_data(
    data: &[u8],
) -> Result<(Vec<CreateBibItem>, Vec<FileImportWarning>), HexforgeError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(std::io::Cursor::new(data));

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
        .iter()
        .map(|s| s.trim().to_lowercase().replace([' ', '-'], "_"))
        .collect();

    let mapping = build_column_mapping(&headers)?;

    let mut items = Vec::new();
    let mut warnings = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        let row_num = idx + 2; // Row 1 is header
        match result {
            Ok(record) => {
                let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                match build_create_bibitem(&row, &mapping, row_num) {
                    Ok((item, row_warnings)) => {
                        warnings.extend(row_warnings);
                        items.push(item);
                    }
                    Err(msg) => {
                        warnings.push(FileImportWarning {
                            row: row_num,
                            message: msg,
                        });
                    }
                }
            }
            Err(e) => {
                warnings.push(FileImportWarning {
                    row: row_num,
                    message: format!("CSV parse error: {e}"),
                });
            }
        }
    }

    Ok((items, warnings))
}

// ============================================================================
// Spreadsheet Parsing (ODS, XLSX, XLS)
// ============================================================================

fn parse_spreadsheet_data(
    data: &[u8],
) -> Result<(Vec<CreateBibItem>, Vec<FileImportWarning>), HexforgeError> {
    use calamine::{Reader, Sheets, open_workbook_auto_from_rs};

    let cursor = std::io::Cursor::new(data);
    let mut workbook: Sheets<_> = open_workbook_auto_from_rs(cursor)
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?;

    // Get the first sheet
    let sheet_names = workbook.sheet_names().to_vec();
    let sheet_name = sheet_names.first().ok_or_else(|| {
        HexforgeError::Validation(ValidationError::custom("Empty workbook: no sheets found"))
    })?;

    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?;

    let mut rows_iter = range.rows();

    // First row is headers
    let header_row = rows_iter.next().ok_or_else(|| {
        HexforgeError::Validation(ValidationError::custom("Empty sheet: no header row"))
    })?;

    let headers: Vec<String> = header_row
        .iter()
        .map(|cell| {
            cell.to_string()
                .trim()
                .to_lowercase()
                .replace([' ', '-'], "_")
        })
        .collect();

    let mapping = build_column_mapping(&headers)?;

    let mut items = Vec::new();
    let mut warnings = Vec::new();

    for (idx, row_data) in rows_iter.enumerate() {
        let row_num = idx + 2;
        let row: Vec<String> = row_data.iter().map(|cell| cell.to_string()).collect();

        // Skip empty rows
        if row.iter().all(|s| s.trim().is_empty()) {
            continue;
        }

        match build_create_bibitem(&row, &mapping, row_num) {
            Ok((item, row_warnings)) => {
                warnings.extend(row_warnings);
                items.push(item);
            }
            Err(msg) => {
                warnings.push(FileImportWarning {
                    row: row_num,
                    message: msg,
                });
            }
        }
    }

    Ok((items, warnings))
}

// ============================================================================
// Column mapping and row parsing
// ============================================================================

/// Column indices for spreadsheet fields.
#[derive(Debug, Default)]
struct ColumnMapping {
    bibkey: Option<usize>,
    entry_type: Option<usize>,
    pubstate: Option<usize>,
    date_year: Option<usize>,
    date_year_2_hyphen: Option<usize>,
    date_year_2_slash: Option<usize>,
    date_month: Option<usize>,
    date_day: Option<usize>,
    date_is_no_date: Option<usize>,
    title_latex: Option<usize>,
    title_unicode: Option<usize>,
    title_simplified: Option<usize>,
    booktitle_latex: Option<usize>,
    booktitle_unicode: Option<usize>,
    booktitle_simplified: Option<usize>,
    journal_id: Option<usize>,
    publisher_id: Option<usize>,
    address: Option<usize>,
    volume: Option<usize>,
    number: Option<usize>,
    pages: Option<usize>,
    eid: Option<usize>,
    series_id: Option<usize>,
    edition: Option<usize>,
    institution_id: Option<usize>,
    school_id: Option<usize>,
    type_field: Option<usize>,
    doi: Option<usize>,
    url: Option<usize>,
    eprint: Option<usize>,
    urn: Option<usize>,
    crossref_id: Option<usize>,
    issuetitle_latex: Option<usize>,
    issuetitle_unicode: Option<usize>,
    note_latex: Option<usize>,
    note_unicode: Option<usize>,
    extra_note_latex: Option<usize>,
    extra_note_unicode: Option<usize>,
    langid: Option<usize>,
    is_translation: Option<usize>,
    epoch: Option<usize>,
    options: Option<usize>,
    shorthand: Option<usize>,
    person_id: Option<usize>,
    has_fulltext: Option<usize>,
    fulltext_path: Option<usize>,
}

fn build_column_mapping(headers: &[String]) -> Result<ColumnMapping, HexforgeError> {
    let mut mapping = ColumnMapping::default();

    for (idx, header) in headers.iter().enumerate() {
        match header.as_str() {
            "bibkey" => mapping.bibkey = Some(idx),
            "entry_type" | "type" => mapping.entry_type = Some(idx),
            "pubstate" => mapping.pubstate = Some(idx),
            "date_year" | "year" => mapping.date_year = Some(idx),
            "date_year_2_hyphen" => mapping.date_year_2_hyphen = Some(idx),
            "date_year_2_slash" => mapping.date_year_2_slash = Some(idx),
            "date_month" | "month" => mapping.date_month = Some(idx),
            "date_day" | "day" => mapping.date_day = Some(idx),
            "date_is_no_date" | "no_date" => mapping.date_is_no_date = Some(idx),
            "title_latex" | "title" => mapping.title_latex = Some(idx),
            "title_unicode" => mapping.title_unicode = Some(idx),
            "title_simplified" => mapping.title_simplified = Some(idx),
            "booktitle_latex" | "booktitle" => mapping.booktitle_latex = Some(idx),
            "booktitle_unicode" => mapping.booktitle_unicode = Some(idx),
            "booktitle_simplified" => mapping.booktitle_simplified = Some(idx),
            "journal_id" => mapping.journal_id = Some(idx),
            "publisher_id" => mapping.publisher_id = Some(idx),
            "address" => mapping.address = Some(idx),
            "volume" => mapping.volume = Some(idx),
            "number" => mapping.number = Some(idx),
            "pages" => mapping.pages = Some(idx),
            "eid" => mapping.eid = Some(idx),
            "series_id" => mapping.series_id = Some(idx),
            "edition" => mapping.edition = Some(idx),
            "institution_id" => mapping.institution_id = Some(idx),
            "school_id" => mapping.school_id = Some(idx),
            "type_field" => mapping.type_field = Some(idx),
            "doi" => mapping.doi = Some(idx),
            "url" => mapping.url = Some(idx),
            "eprint" => mapping.eprint = Some(idx),
            "urn" => mapping.urn = Some(idx),
            "crossref_id" => mapping.crossref_id = Some(idx),
            "issuetitle_latex" | "issuetitle" => mapping.issuetitle_latex = Some(idx),
            "issuetitle_unicode" => mapping.issuetitle_unicode = Some(idx),
            "note_latex" | "note" => mapping.note_latex = Some(idx),
            "note_unicode" => mapping.note_unicode = Some(idx),
            "extra_note_latex" | "extra_note" => mapping.extra_note_latex = Some(idx),
            "extra_note_unicode" => mapping.extra_note_unicode = Some(idx),
            "langid" | "language" => mapping.langid = Some(idx),
            "is_translation" => mapping.is_translation = Some(idx),
            "epoch" => mapping.epoch = Some(idx),
            "options" => mapping.options = Some(idx),
            "shorthand" => mapping.shorthand = Some(idx),
            "person_id" => mapping.person_id = Some(idx),
            "has_fulltext" => mapping.has_fulltext = Some(idx),
            "fulltext_path" => mapping.fulltext_path = Some(idx),
            _ => {} // Unknown columns are ignored
        }
    }

    if mapping.bibkey.is_none() {
        return Err(HexforgeError::Validation(ValidationError::custom(
            "Missing required column: bibkey",
        )));
    }

    Ok(mapping)
}

/// Get a string value from a row at the given column index.
fn get_string(row: &[String], idx: Option<usize>) -> Option<String> {
    idx.and_then(|i| row.get(i))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Parse an i16 from a row.
fn parse_i16_col(row: &[String], idx: Option<usize>) -> Option<i16> {
    get_string(row, idx).and_then(|s| s.parse().ok())
}

/// Parse an i64 from a row.
fn parse_i64_col(row: &[String], idx: Option<usize>) -> Option<i64> {
    get_string(row, idx).and_then(|s| s.parse().ok())
}

/// Parse a boolean from a row.
fn parse_bool_col(row: &[String], idx: Option<usize>) -> Option<bool> {
    get_string(row, idx)
        .map(|s| matches!(s.to_lowercase().as_str(), "true" | "1" | "yes" | "y" | "x"))
}

/// Build a CreateBibItem from a parsed row.
fn build_create_bibitem(
    row: &[String],
    mapping: &ColumnMapping,
    row_num: usize,
) -> Result<(CreateBibItem, Vec<FileImportWarning>), String> {
    let mut warnings = Vec::new();

    let bibkey =
        get_string(row, mapping.bibkey).ok_or_else(|| format!("Row {row_num}: missing bibkey"))?;

    // Parse entry_type (required for CreateBibItem)
    let entry_type = get_string(row, mapping.entry_type)
        .map(|s| {
            let result: Result<EntryType, _> = s.parse();
            match result {
                Ok(et) => et,
                Err(_) => {
                    warnings.push(FileImportWarning {
                        row: row_num,
                        message: format!("Unknown entry_type '{s}', using Unknown"),
                    });
                    EntryType::Unknown
                }
            }
        })
        .unwrap_or(EntryType::Unknown);

    // Parse optional enums
    let pubstate = get_string(row, mapping.pubstate).and_then(|s| s.parse().ok());
    let langid = get_string(row, mapping.langid).and_then(|s| s.parse().ok());
    let epoch = get_string(row, mapping.epoch).and_then(|s| s.parse().ok());

    // Title: use title_latex as fallback for all three if not all specified
    let title_latex = get_string(row, mapping.title_latex).unwrap_or_default();
    let title_unicode =
        get_string(row, mapping.title_unicode).unwrap_or_else(|| title_latex.clone());
    let title_simplified =
        get_string(row, mapping.title_simplified).unwrap_or_else(|| title_latex.clone());

    let item = CreateBibItem {
        bibkey,
        entry_type,
        date_year: parse_i16_col(row, mapping.date_year),
        date_year_2_hyphen: parse_i16_col(row, mapping.date_year_2_hyphen),
        date_year_2_slash: parse_i16_col(row, mapping.date_year_2_slash),
        date_month: parse_i16_col(row, mapping.date_month),
        date_day: parse_i16_col(row, mapping.date_day),
        date_is_no_date: parse_bool_col(row, mapping.date_is_no_date),
        pubstate,
        title_latex,
        title_unicode,
        title_simplified,
        booktitle_latex: get_string(row, mapping.booktitle_latex),
        booktitle_unicode: get_string(row, mapping.booktitle_unicode),
        booktitle_simplified: get_string(row, mapping.booktitle_simplified),
        journal_id: parse_i64_col(row, mapping.journal_id),
        publisher_id: parse_i64_col(row, mapping.publisher_id),
        address: get_string(row, mapping.address),
        volume: get_string(row, mapping.volume),
        number: get_string(row, mapping.number),
        pages: get_string(row, mapping.pages),
        eid: get_string(row, mapping.eid),
        series_id: parse_i64_col(row, mapping.series_id),
        edition: get_string(row, mapping.edition),
        institution_id: parse_i64_col(row, mapping.institution_id),
        school_id: parse_i64_col(row, mapping.school_id),
        type_field: get_string(row, mapping.type_field),
        doi: get_string(row, mapping.doi),
        url: get_string(row, mapping.url),
        eprint: get_string(row, mapping.eprint),
        urn: get_string(row, mapping.urn),
        crossref_id: parse_i64_col(row, mapping.crossref_id),
        issuetitle_latex: get_string(row, mapping.issuetitle_latex),
        issuetitle_unicode: get_string(row, mapping.issuetitle_unicode),
        note_latex: get_string(row, mapping.note_latex),
        note_unicode: get_string(row, mapping.note_unicode),
        extra_note_latex: get_string(row, mapping.extra_note_latex),
        extra_note_unicode: get_string(row, mapping.extra_note_unicode),
        langid,
        is_translation: parse_bool_col(row, mapping.is_translation),
        epoch,
        options: get_string(row, mapping.options),
        shorthand: get_string(row, mapping.shorthand),
        person_id: parse_i64_col(row, mapping.person_id),
        has_fulltext: parse_bool_col(row, mapping.has_fulltext),
        fulltext_path: get_string(row, mapping.fulltext_path),
    };

    Ok((item, warnings))
}
