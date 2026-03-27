//! Full-text search handler for bibitems.
//!
//! Uses pg_trgm for fuzzy trigram-based similarity search.
//! Results and total count are fetched in a single query via `COUNT(*) OVER()`.

use hexforge::HexforgeError;
use hexforge::axum_exports::{Json, State};
use hexforge::db_exports::{FromRow, PgArguments};
use serde::{Deserialize, Serialize};

use crate::entities::BibItem;
use crate::state::AppState;

/// Minimum similarity threshold for search results (0.0 to 1.0).
const SIMILARITY_THRESHOLD: f32 = 0.1;

/// Search request parameters.
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// Search query string (searches title, booktitle using trigram similarity).
    pub query: String,

    /// Filter by entry type.
    pub entry_type: Option<String>,

    /// Filter by year range (from).
    pub year_from: Option<i16>,

    /// Filter by year range (to).
    pub year_to: Option<i16>,

    /// Filter by author ID.
    pub author_id: Option<i64>,

    /// Filter by journal ID.
    pub journal_id: Option<i64>,

    /// Filter by epoch.
    pub epoch: Option<String>,

    /// Maximum number of results (default: 50, max: 100).
    #[serde(default = "default_limit")]
    pub limit: i64,

    /// Offset for pagination.
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// Search response with results and metadata.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<BibItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// Row type for the search query that includes the total count window function.
#[derive(Debug, FromRow)]
struct SearchRow {
    // BibItem columns
    id: i64,
    bibkey: String,
    entry_type: String,
    date_year: Option<i16>,
    date_year_2_hyphen: Option<i16>,
    date_year_2_slash: Option<i16>,
    date_month: Option<i16>,
    date_day: Option<i16>,
    date_is_no_date: bool,
    pubstate: Option<String>,
    title_latex: String,
    title_unicode: String,
    title_simplified: String,
    booktitle_latex: Option<String>,
    booktitle_unicode: Option<String>,
    booktitle_simplified: Option<String>,
    journal_id: Option<i64>,
    publisher_id: Option<i64>,
    address: Option<String>,
    volume: Option<String>,
    number: Option<String>,
    pages: Option<String>,
    eid: Option<String>,
    series_id: Option<i64>,
    edition: Option<String>,
    institution_id: Option<i64>,
    school_id: Option<i64>,
    type_field: Option<String>,
    doi: Option<String>,
    url: Option<String>,
    eprint: Option<String>,
    urn: Option<String>,
    crossref_id: Option<i64>,
    issuetitle_latex: Option<String>,
    issuetitle_unicode: Option<String>,
    note_latex: Option<String>,
    note_unicode: Option<String>,
    extra_note_latex: Option<String>,
    extra_note_unicode: Option<String>,
    langid: Option<String>,
    is_translation: bool,
    epoch: Option<String>,
    options: Option<String>,
    shorthand: Option<String>,
    person_id: Option<i64>,
    has_fulltext: bool,
    fulltext_path: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    // Window function column
    total_count: i64,
}

/// Search bibitems with full-text search and filters.
///
/// `POST /api/v1/search`
///
/// Uses a single query with `COUNT(*) OVER()` to get both results and total
/// count in one round-trip.
pub async fn search_bibitems(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, HexforgeError> {
    let limit = request.limit.clamp(1, 100);
    let offset = request.offset.max(0);

    // Build dynamic query
    let mut conditions = vec!["1=1".to_string()];
    let mut param_idx: usize = 1;

    // Text search using pg_trgm similarity
    if !request.query.is_empty() {
        conditions.push(format!(
            r#"GREATEST(
                COALESCE(similarity(title_simplified, ${param_idx}), 0),
                COALESCE(similarity(booktitle_simplified, ${param_idx}), 0)
            ) >= {SIMILARITY_THRESHOLD}"#
        ));
        param_idx += 1;
    }

    // Entry type filter
    if request.entry_type.is_some() {
        conditions.push(format!("entry_type::text = ${param_idx}"));
        param_idx += 1;
    }

    // Year range filters
    if request.year_from.is_some() {
        conditions.push(format!("date_year >= ${param_idx}"));
        param_idx += 1;
    }

    if request.year_to.is_some() {
        conditions.push(format!("date_year <= ${param_idx}"));
        param_idx += 1;
    }

    // Author filter (via junction table)
    if request.author_id.is_some() {
        conditions.push(format!(
            "id IN (SELECT bibitem_id FROM bibitem_authors WHERE author_id = ${param_idx})"
        ));
        param_idx += 1;
    }

    // Journal filter
    if request.journal_id.is_some() {
        conditions.push(format!("journal_id = ${param_idx}"));
        param_idx += 1;
    }

    // Epoch filter
    if request.epoch.is_some() {
        conditions.push(format!("epoch::text = ${param_idx}"));
        param_idx += 1;
    }

    let where_clause = conditions.join(" AND ");

    // Single query with COUNT(*) OVER() for total count
    let order_clause = if request.query.is_empty() {
        "date_year DESC NULLS LAST, id DESC".to_string()
    } else {
        r#"GREATEST(
                COALESCE(similarity(title_simplified, $1), 0),
                COALESCE(similarity(booktitle_simplified, $1), 0)
            ) DESC,
            date_year DESC NULLS LAST,
            id DESC"#
            .to_string()
    };

    let sql = format!(
        r#"SELECT *, COUNT(*) OVER() AS total_count
        FROM bibitems
        WHERE {where_clause}
        ORDER BY {order_clause}
        LIMIT ${param_idx} OFFSET ${next}"#,
        next = param_idx + 1
    );

    // Build query with dynamic bindings
    use sqlx::Arguments;
    let mut args = PgArguments::default();

    if !request.query.is_empty() {
        args.add(&request.query)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }
    if let Some(ref entry_type) = request.entry_type {
        args.add(entry_type)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }
    if let Some(year_from) = request.year_from {
        args.add(year_from)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }
    if let Some(year_to) = request.year_to {
        args.add(year_to)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }
    if let Some(author_id) = request.author_id {
        args.add(author_id)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }
    if let Some(journal_id) = request.journal_id {
        args.add(journal_id)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }
    if let Some(ref epoch) = request.epoch {
        args.add(epoch)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
    }
    args.add(limit)
        .map_err(|e| HexforgeError::internal(e.to_string()))?;
    args.add(offset)
        .map_err(|e| HexforgeError::internal(e.to_string()))?;

    let rows: Vec<SearchRow> = sqlx::query_as_with::<_, SearchRow, _>(&sql, args)
        .fetch_all(state.pool.pool())
        .await
        .map_err(HexforgeError::data_source)?;

    // Extract total from first row (all rows have the same total_count)
    let total = rows.first().map_or(0, |r| r.total_count);

    // Convert to BibItem entities
    let results: Vec<BibItem> = rows.into_iter().map(search_row_to_bibitem).collect();

    Ok(Json(SearchResponse {
        results,
        total,
        limit,
        offset,
    }))
}

/// Convert a SearchRow to a BibItem.
fn search_row_to_bibitem(row: SearchRow) -> BibItem {
    use std::str::FromStr;

    BibItem {
        id: row.id,
        bibkey: row.bibkey,
        entry_type: row
            .entry_type
            .parse()
            .unwrap_or(crate::domain::EntryType::Unknown),
        date_year: row.date_year,
        date_year_2_hyphen: row.date_year_2_hyphen,
        date_year_2_slash: row.date_year_2_slash,
        date_month: row.date_month,
        date_day: row.date_day,
        date_is_no_date: row.date_is_no_date,
        pubstate: row
            .pubstate
            .and_then(|s| crate::domain::PubState::from_str(&s).ok()),
        title_latex: row.title_latex,
        title_unicode: row.title_unicode,
        title_simplified: row.title_simplified,
        booktitle_latex: row.booktitle_latex,
        booktitle_unicode: row.booktitle_unicode,
        booktitle_simplified: row.booktitle_simplified,
        journal_id: row.journal_id,
        publisher_id: row.publisher_id,
        address: row.address,
        volume: row.volume,
        number: row.number,
        pages: row.pages,
        eid: row.eid,
        series_id: row.series_id,
        edition: row.edition,
        institution_id: row.institution_id,
        school_id: row.school_id,
        type_field: row.type_field,
        doi: row.doi,
        url: row.url,
        eprint: row.eprint,
        urn: row.urn,
        crossref_id: row.crossref_id,
        issuetitle_latex: row.issuetitle_latex,
        issuetitle_unicode: row.issuetitle_unicode,
        note_latex: row.note_latex,
        note_unicode: row.note_unicode,
        extra_note_latex: row.extra_note_latex,
        extra_note_unicode: row.extra_note_unicode,
        langid: row
            .langid
            .and_then(|s| crate::domain::LangId::from_str(&s).ok()),
        is_translation: row.is_translation,
        epoch: row
            .epoch
            .and_then(|s| crate::domain::Epoch::from_str(&s).ok()),
        options: row.options,
        shorthand: row.shorthand,
        person_id: row.person_id,
        has_fulltext: row.has_fulltext,
        fulltext_path: row.fulltext_path,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
