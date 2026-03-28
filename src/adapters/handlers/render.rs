//! Render handler — builds HTML bibliography from database data.
//!
//! `POST /api/v1/render`
//!
//! Accepts a JSON request body with either `ids` or `bibkeys` to select bibitems.
//! Returns rendered HTML bibliography sorted by author, year, bibkey.
//! Capped at 1000 items per request.

use std::collections::{HashMap, HashSet};

use hexforge::axum_exports::{IntoResponse, Json, Response, State, StatusCode, header};
use hexforge::db_exports::{FromRow, query_as};
use hexforge::{HexforgeError, WhereClause};
use serde::{Deserialize, Serialize};

use crate::domain::{Author, BibItem};
use crate::logic::renderer::{AuthorName, RenderContext, author_sort_key, render_bibliography};
use crate::state::AppState;

// =============================================================================
// Request / response types
// =============================================================================

/// Maximum number of items per render request.
const MAX_RENDER_ITEMS: usize = 1000;

/// Request body for the render endpoint.
#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    /// Select bibitems by ID.
    pub ids: Option<Vec<i64>>,
    /// Select bibitems by bibkey.
    pub bibkeys: Option<Vec<String>>,
}

/// 422 error response for missing or too-many items.
#[derive(Debug, Serialize)]
struct RenderError {
    error: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_bibkeys: Option<Vec<String>>,
}

/// 400 bad request error.
#[derive(Debug, Serialize)]
struct BadRequestError {
    error: &'static str,
    message: &'static str,
}

// =============================================================================
// Junction row type
// =============================================================================

/// A row from bibitem_authors junction table.
#[derive(Debug, FromRow)]
struct BibitemAuthorRow {
    bibitem_id: i64,
    author_id: i64,
    role: String,
    position: i16,
}

// =============================================================================
// Handler
// =============================================================================

/// Render bibliography as HTML.
///
/// `POST /api/v1/render`
///
/// Returns `Content-Type: text/html` with the rendered bibliography.
pub async fn render_bibitems(
    State(state): State<AppState>,
    Json(req): Json<RenderRequest>,
) -> Result<Response, HexforgeError> {
    // 1. Validate request
    let requested_count = req
        .ids
        .as_ref()
        .map(Vec::len)
        .or_else(|| req.bibkeys.as_ref().map(Vec::len));

    match requested_count {
        None => {
            return Ok((
                StatusCode::BAD_REQUEST,
                Json(BadRequestError {
                    error: "bad_request",
                    message: "Request must specify \"ids\" or \"bibkeys\"",
                }),
            )
                .into_response());
        }
        Some(count) if count > MAX_RENDER_ITEMS => {
            return Ok((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(RenderError {
                    error: "too_many_items",
                    message: format!("Requested {count} items, maximum is {MAX_RENDER_ITEMS}"),
                    missing_ids: None,
                    missing_bibkeys: None,
                }),
            )
                .into_response());
        }
        _ => {}
    }

    // 2. Resolve bibitems
    let bibitems: Vec<BibItem> = if let Some(ref id_list) = req.ids {
        if id_list.is_empty() {
            return Ok(html_response(String::new()));
        }
        let found = state
            .bibitem_ds
            .find_by_ids(id_list)
            .await
            .map_err(HexforgeError::data_source)?;
        let found_ids: HashSet<i64> = found.iter().map(|b| b.id).collect();
        let missing: Vec<i64> = id_list
            .iter()
            .filter(|id| !found_ids.contains(id))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Ok((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(RenderError {
                    error: "not_found",
                    message: format!("{} requested ID(s) not found", missing.len()),
                    missing_ids: Some(missing),
                    missing_bibkeys: None,
                }),
            )
                .into_response());
        }
        found
    } else if let Some(ref bibkey_list) = req.bibkeys {
        if bibkey_list.is_empty() {
            return Ok(html_response(String::new()));
        }
        let mut all = Vec::new();
        for bibkey in bibkey_list {
            let found = state
                .bibitem_ds
                .find_one(WhereClause::new("bibkey = $1").bind(bibkey.clone()))
                .await
                .map_err(HexforgeError::data_source)?;
            if let Some(item) = found {
                all.push(item);
            }
        }
        let found_keys: HashSet<&str> = all.iter().map(|b| b.bibkey.as_str()).collect();
        let missing: Vec<String> = bibkey_list
            .iter()
            .filter(|k| !found_keys.contains(k.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Ok((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(RenderError {
                    error: "not_found",
                    message: format!("{} requested bibkey(s) not found", missing.len()),
                    missing_ids: None,
                    missing_bibkeys: Some(missing),
                }),
            )
                .into_response());
        }
        all
    } else {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(BadRequestError {
                error: "bad_request",
                message: "Request must specify \"ids\" or \"bibkeys\"",
            }),
        )
            .into_response());
    };

    if bibitems.is_empty() {
        return Ok(html_response(String::new()));
    }

    // 3. Batch-fetch all related data
    let bibitem_ids: Vec<i64> = bibitems.iter().map(|b| b.id).collect();

    // Collect unique FK IDs
    let mut journal_ids = HashSet::new();
    let mut publisher_ids = HashSet::new();
    let mut institution_ids = HashSet::new();
    let mut school_ids = HashSet::new();
    let mut series_ids = HashSet::new();
    let mut crossref_ids = HashSet::new();

    for bib in &bibitems {
        if let Some(id) = bib.journal_id {
            journal_ids.insert(id);
        }
        if let Some(id) = bib.publisher_id {
            publisher_ids.insert(id);
        }
        if let Some(id) = bib.institution_id {
            institution_ids.insert(id);
        }
        if let Some(id) = bib.school_id {
            school_ids.insert(id);
        }
        if let Some(id) = bib.series_id {
            series_ids.insert(id);
        }
        if let Some(id) = bib.crossref_id {
            crossref_ids.insert(id);
        }
    }

    // Batch-fetch related entities
    let journals_map = batch_fetch_names(&state, "journals", "name_unicode", &journal_ids).await?;
    let publishers_map =
        batch_fetch_names(&state, "publishers", "name_unicode", &publisher_ids).await?;
    let institutions_map =
        batch_fetch_names(&state, "institutions", "name_unicode", &institution_ids).await?;
    let schools_map = batch_fetch_names(&state, "schools", "name_unicode", &school_ids).await?;
    let series_map = batch_fetch_names(&state, "series", "name_unicode", &series_ids).await?;
    let crossrefs_map = batch_fetch_names(&state, "bibitems", "bibkey", &crossref_ids).await?;

    // Batch-fetch junction data (authors/editors)
    let author_rows = fetch_bibitem_authors_batch(&state, &bibitem_ids).await?;

    // Build author lookup: author_id -> Author
    let all_author_ids: HashSet<i64> = author_rows.iter().map(|r| r.author_id).collect();
    let authors_map = batch_fetch_authors(&state, &all_author_ids).await?;

    // Group junction data by bibitem_id and role
    let mut authors_by_bibitem: HashMap<i64, Vec<&BibitemAuthorRow>> = HashMap::new();
    for row in &author_rows {
        authors_by_bibitem
            .entry(row.bibitem_id)
            .or_default()
            .push(row);
    }

    // 4. Build RenderContext for each bibitem and sort
    let mut items_with_ctx: Vec<(BibItem, RenderContext)> = bibitems
        .into_iter()
        .map(|bib| {
            let bib_authors = authors_by_bibitem.get(&bib.id);

            let authors = extract_role_authors(bib_authors, "author", &authors_map);
            let editors = extract_role_authors(bib_authors, "editor", &authors_map);
            let guesteditors = extract_role_authors(bib_authors, "guesteditor", &authors_map);

            let ctx = RenderContext {
                authors,
                editors,
                guesteditors,
                journal_name: bib.journal_id.and_then(|id| journals_map.get(&id).cloned()),
                publisher_name: bib
                    .publisher_id
                    .and_then(|id| publishers_map.get(&id).cloned()),
                series_name: bib.series_id.and_then(|id| series_map.get(&id).cloned()),
                institution_name: bib
                    .institution_id
                    .and_then(|id| institutions_map.get(&id).cloned()),
                school_name: bib.school_id.and_then(|id| schools_map.get(&id).cloned()),
                crossref_bibkey: bib
                    .crossref_id
                    .and_then(|id| crossrefs_map.get(&id).cloned()),
                suppress_author: false,
            };
            (bib, ctx)
        })
        .collect();

    // Sort by author family name -> year -> bibkey
    items_with_ctx.sort_by(|(a, ctx_a), (b, ctx_b)| {
        let key_a = author_sort_key(&ctx_a.authors);
        let key_b = author_sort_key(&ctx_b.authors);
        key_a
            .cmp(&key_b)
            .then_with(|| a.date_year.cmp(&b.date_year))
            .then_with(|| a.bibkey.cmp(&b.bibkey))
    });

    // 5. Render bibliography
    let html = render_bibliography(&items_with_ctx);

    Ok(html_response(html))
}

// =============================================================================
// Helpers
// =============================================================================

/// Build an HTML response.
fn html_response(body: String) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

/// Extract AuthorName list for a specific role from junction rows.
fn extract_role_authors(
    bib_authors: Option<&Vec<&BibitemAuthorRow>>,
    role: &str,
    authors_map: &HashMap<i64, Author>,
) -> Vec<AuthorName> {
    bib_authors
        .map(|rows| {
            let mut filtered: Vec<&BibitemAuthorRow> =
                rows.iter().filter(|r| r.role == role).copied().collect();
            filtered.sort_by_key(|r| r.position);
            filtered
                .iter()
                .filter_map(|r| authors_map.get(&r.author_id).map(author_to_name))
                .collect()
        })
        .unwrap_or_default()
}

/// Convert a domain Author to an AuthorName (using unicode fields).
fn author_to_name(author: &Author) -> AuthorName {
    AuthorName {
        family: author.family_name_unicode.clone(),
        given: author.given_name_unicode.clone(),
        mononym: author.mononym_unicode.clone(),
    }
}

/// Row type for batch name lookups.
#[derive(Debug, FromRow)]
struct IdNameRow {
    id: i64,
    name: String,
}

/// Batch-fetch a single name column from a table for a set of IDs.
///
/// Returns a HashMap of id -> name_value.
async fn batch_fetch_names(
    state: &AppState,
    table: &str,
    name_column: &str,
    ids: &HashSet<i64>,
) -> Result<HashMap<i64, String>, HexforgeError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let id_vec: Vec<i64> = ids.iter().copied().collect();
    let sql = format!("SELECT id, {name_column} AS name FROM {table} WHERE id = ANY($1)");
    let rows: Vec<IdNameRow> = query_as(&sql)
        .bind(&id_vec)
        .fetch_all(state.pool.pool())
        .await
        .map_err(HexforgeError::data_source)?;
    Ok(rows.into_iter().map(|r| (r.id, r.name)).collect())
}

/// Batch-fetch authors by IDs into a HashMap.
async fn batch_fetch_authors(
    state: &AppState,
    ids: &HashSet<i64>,
) -> Result<HashMap<i64, Author>, HexforgeError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let id_vec: Vec<i64> = ids.iter().copied().collect();
    let authors = state
        .author_ds
        .find_by_ids(&id_vec)
        .await
        .map_err(HexforgeError::data_source)?;
    Ok(authors.into_iter().map(|a| (a.id, a)).collect())
}

/// Batch-fetch all bibitem_authors rows for the given bibitem IDs.
async fn fetch_bibitem_authors_batch(
    state: &AppState,
    bibitem_ids: &[i64],
) -> Result<Vec<BibitemAuthorRow>, HexforgeError> {
    if bibitem_ids.is_empty() {
        return Ok(vec![]);
    }
    query_as::<_, BibitemAuthorRow>(
        r#"
        SELECT bibitem_id, author_id, role::text as role, position
        FROM bibitem_authors
        WHERE bibitem_id = ANY($1)
        ORDER BY bibitem_id, role, position
        "#,
    )
    .bind(bibitem_ids)
    .fetch_all(state.pool.pool())
    .await
    .map_err(HexforgeError::data_source)
}
