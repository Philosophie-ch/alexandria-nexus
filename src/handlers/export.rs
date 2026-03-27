//! Export handler for bibitems in CSV and BibTeX formats.
//!
//! `GET /api/v1/admin/export`
//!
//! Query params:
//! - format: "csv" | "bibtex" (default: csv)
//! - ids: comma-separated list of bibitem IDs (optional, exports all if not provided)
//!
//! Requires Admin permission.
//! Uses streaming for large full exports to avoid loading everything into memory.

use futures::StreamExt;
use hexforge::HexforgeError;
use hexforge::axum_exports::{Body, IntoResponse, Query, Response, State, StatusCode, header};
use serde::Deserialize;
use tokio_stream::wrappers::ReceiverStream;

use crate::entities::BibItem;
use crate::state::AppState;

/// Export format options.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    #[default]
    Csv,
    Bibtex,
}

/// Query parameters for export.
#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    /// Export format (csv or bibtex)
    #[serde(default)]
    pub format: ExportFormat,
    /// Optional comma-separated list of bibitem IDs
    pub ids: Option<String>,
}

/// CSV header line.
const CSV_HEADER: &str =
    "id,bibkey,entry_type,pubstate,year,title,booktitle,journal_id,volume,number,doi,url\n";

/// Export bibitems in CSV or BibTeX format.
///
/// `GET /api/v1/admin/export`
///
/// For full exports, uses streaming to efficiently handle large datasets.
/// For ID-based exports, uses batch mode (smaller datasets).
pub async fn export_bibitems(
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, HexforgeError> {
    // Parse IDs if provided
    let ids: Option<Vec<i64>> = query.ids.as_ref().map(|ids_str| {
        ids_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    });

    // For specific ID exports, use batch mode
    if let Some(ref id_list) = ids {
        let bibitems = state
            .bibitem_ds
            .find_by_ids(id_list)
            .await
            .map_err(HexforgeError::data_source)?;

        return match query.format {
            ExportFormat::Csv => Ok(batch_csv_response(&bibitems)),
            ExportFormat::Bibtex => Ok(batch_bibtex_response(&bibitems)),
        };
    }

    // For full exports, use streaming.
    // We build an owned stream from the pool directly so it is 'static.
    let pool = state.pool.pool().clone();

    match query.format {
        ExportFormat::Csv => Ok(streaming_csv_response(pool)),
        ExportFormat::Bibtex => Ok(streaming_bibtex_response(pool)),
    }
}

/// Create a streaming CSV response using a direct pool query.
fn streaming_csv_response(pool: hexforge::db_exports::PgPool) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String, std::convert::Infallible>>(32);

    tokio::spawn(async move {
        // Send header first
        if tx.send(Ok(CSV_HEADER.to_string())).await.is_err() {
            return;
        }

        let mut rows =
            hexforge::db_exports::query_as::<_, BibItem>("SELECT * FROM bibitems ORDER BY id")
                .fetch(&pool);

        while let Some(result) = rows.next().await {
            match result {
                Ok(item) => {
                    let row = format_csv_row(&item);
                    if tx.send(Ok(row)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Ok(format!("# ERROR: {e}\n"))).await;
                    break;
                }
            }
        }
    });

    let body_stream = ReceiverStream::new(rx);
    let body = Body::from_stream(body_stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"bibitems.csv\"",
        )
        .header(header::TRANSFER_ENCODING, "chunked")
        .body(body)
        .expect("Response builder with valid headers cannot fail")
}

/// Create a streaming BibTeX response using a direct pool query.
fn streaming_bibtex_response(pool: hexforge::db_exports::PgPool) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String, std::convert::Infallible>>(32);

    tokio::spawn(async move {
        let mut rows =
            hexforge::db_exports::query_as::<_, BibItem>("SELECT * FROM bibitems ORDER BY id")
                .fetch(&pool);

        while let Some(result) = rows.next().await {
            match result {
                Ok(item) => {
                    let entry = format!("{}\n\n", format_bibtex_entry(&item));
                    if tx.send(Ok(entry)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Ok(format!("% ERROR: {e}\n"))).await;
                    break;
                }
            }
        }
    });

    let body_stream = ReceiverStream::new(rx);
    let body = Body::from_stream(body_stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-bibtex; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"bibitems.bib\"",
        )
        .header(header::TRANSFER_ENCODING, "chunked")
        .body(body)
        .expect("Response builder with valid headers cannot fail")
}

/// Create a batch CSV response (for small ID-based exports).
fn batch_csv_response(bibitems: &[BibItem]) -> Response {
    let csv = generate_csv(bibitems);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"bibitems.csv\"",
            ),
        ],
        csv,
    )
        .into_response()
}

/// Create a batch BibTeX response (for small ID-based exports).
fn batch_bibtex_response(bibitems: &[BibItem]) -> Response {
    let bibtex = generate_bibtex(bibitems);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/x-bibtex; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"bibitems.bib\"",
            ),
        ],
        bibtex,
    )
        .into_response()
}

/// Generate CSV output from bibitems.
fn generate_csv(bibitems: &[BibItem]) -> String {
    let mut output = String::with_capacity(bibitems.len() * 200 + CSV_HEADER.len());
    output.push_str(CSV_HEADER);

    for item in bibitems {
        output.push_str(&format_csv_row(item));
    }

    output
}

/// Format a single bibitem as a CSV row.
fn format_csv_row(item: &BibItem) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{},{},{}\n",
        item.id,
        escape_csv(&item.bibkey),
        escape_csv(&item.entry_type.to_string()),
        item.pubstate
            .as_ref()
            .map(|p| escape_csv(&p.to_string()))
            .unwrap_or_default(),
        item.date_year.map(|y| y.to_string()).unwrap_or_default(),
        escape_csv(&item.title_simplified),
        item.booktitle_simplified
            .as_deref()
            .map(escape_csv)
            .unwrap_or_default(),
        item.journal_id.map(|j| j.to_string()).unwrap_or_default(),
        item.volume.as_deref().map(escape_csv).unwrap_or_default(),
        item.number.as_deref().map(escape_csv).unwrap_or_default(),
        item.doi.as_deref().map(escape_csv).unwrap_or_default(),
        item.url.as_deref().map(escape_csv).unwrap_or_default(),
    )
}

/// Escape a string for CSV output.
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Generate BibTeX output from bibitems.
fn generate_bibtex(bibitems: &[BibItem]) -> String {
    let mut output = String::new();
    for item in bibitems {
        output.push_str(&format_bibtex_entry(item));
        output.push_str("\n\n");
    }
    output
}

/// Format a single bibitem as a BibTeX entry.
fn format_bibtex_entry(item: &BibItem) -> String {
    let entry_type = item.entry_type.to_string();

    let mut output = format!("@{}{{{},\n", entry_type, item.bibkey);

    // Title
    if !item.title_latex.is_empty() {
        output.push_str(&format!("  title = {{{}}},\n", item.title_latex));
    }

    // Year
    if let Some(year) = item.date_year {
        output.push_str(&format!("  year = {{{year}}},\n"));
    }

    // Booktitle
    if let Some(ref bt) = item.booktitle_latex
        && !bt.is_empty()
    {
        output.push_str(&format!("  booktitle = {{{bt}}},\n"));
    }

    // Address
    if let Some(ref address) = item.address
        && !address.is_empty()
    {
        output.push_str(&format!("  address = {{{address}}},\n"));
    }

    // Volume
    if let Some(ref volume) = item.volume
        && !volume.is_empty()
    {
        output.push_str(&format!("  volume = {{{volume}}},\n"));
    }

    // Number
    if let Some(ref number) = item.number
        && !number.is_empty()
    {
        output.push_str(&format!("  number = {{{number}}},\n"));
    }

    // Pages
    if let Some(ref pages) = item.pages
        && !pages.is_empty()
    {
        output.push_str(&format!("  pages = {{{pages}}},\n"));
    }

    // DOI
    if let Some(ref doi) = item.doi
        && !doi.is_empty()
    {
        output.push_str(&format!("  doi = {{{doi}}},\n"));
    }

    // URL
    if let Some(ref url) = item.url
        && !url.is_empty()
    {
        output.push_str(&format!("  url = {{{url}}},\n"));
    }

    // Note
    if let Some(ref note) = item.note_latex
        && !note.is_empty()
    {
        output.push_str(&format!("  note = {{{note}}},\n"));
    }

    // Language
    if let Some(ref langid) = item.langid {
        output.push_str(&format!("  langid = {{{langid}}},\n"));
    }

    // Remove trailing comma and newline, close brace
    if output.ends_with(",\n") {
        output.truncate(output.len() - 2);
        output.push('\n');
    }
    output.push('}');

    output
}
