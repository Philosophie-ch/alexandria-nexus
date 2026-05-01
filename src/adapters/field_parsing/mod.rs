pub mod author;
pub mod bibkey;
pub mod date;
pub mod entry_type;
pub mod keywords;
pub mod pages;
pub mod row;

pub use author::parse_variant_to_keys;
pub use row::{CsvHeaders, parse_row};

use hexforge::{HexforgeError, ValidationError};

use crate::logic::full_import::{FieldError, ParsedBibRow, RowError, RowParseResult};

/// Parse a human-readable full CSV into typed rows and field-level errors.
pub fn parse_all_rows(data: &[u8]) -> Result<(Vec<ParsedBibRow>, Vec<RowError>), HexforgeError> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(data);

    let headers = rdr
        .headers()
        .map_err(|e| {
            HexforgeError::Validation(ValidationError::custom(format!("invalid CSV headers: {e}")))
        })?
        .clone();

    let header_fields: Vec<&str> = headers.iter().collect();
    let csv_headers = CsvHeaders::from_headers(&header_fields);
    let mut parsed_rows = Vec::new();
    let mut row_errors = Vec::new();

    for (idx, result) in rdr.records().enumerate() {
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                row_errors.push(RowError {
                    row: idx + 2,
                    bibkey: None,
                    errors: vec![FieldError {
                        field: "_csv".to_string(),
                        error: format!("malformed CSV row: {e}"),
                    }],
                });
                continue;
            }
        };

        let fields: Vec<&str> = record.iter().collect();
        match parse_row(&csv_headers, &fields) {
            RowParseResult::Ok(row) => parsed_rows.push(*row),
            RowParseResult::Err { bibkey, errors } => {
                row_errors.push(RowError {
                    row: idx + 2,
                    bibkey,
                    errors,
                });
            }
        }
    }

    Ok((parsed_rows, row_errors))
}
