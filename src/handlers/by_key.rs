//! Handlers for fetching entities by their unique key.

use hexforge::axum_exports::{Json, Path, State};
use hexforge::{HexforgeError, WhereClause};

use crate::entities::{Author, Institution, Journal, Publisher, School, Series};
use crate::state::AppState;

/// Get an author by their unique key.
///
/// `GET /api/v1/authors/by-key/{key}`
pub async fn get_author_by_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Author>, HexforgeError> {
    let result = state
        .author_ds
        .find_one(WhereClause::new("author_key = $1").bind(key))
        .await
        .map_err(HexforgeError::data_source)?;

    result.map(Json).ok_or(HexforgeError::NotFound)
}

/// Get a journal by its unique key.
///
/// `GET /api/v1/journals/by-key/{key}`
pub async fn get_journal_by_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Journal>, HexforgeError> {
    let result = state
        .journal_ds
        .find_one(WhereClause::new("journal_key = $1").bind(key))
        .await
        .map_err(HexforgeError::data_source)?;

    result.map(Json).ok_or(HexforgeError::NotFound)
}

/// Get a publisher by its unique key.
///
/// `GET /api/v1/publishers/by-key/{key}`
pub async fn get_publisher_by_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Publisher>, HexforgeError> {
    let result = state
        .publisher_ds
        .find_one(WhereClause::new("publisher_key = $1").bind(key))
        .await
        .map_err(HexforgeError::data_source)?;

    result.map(Json).ok_or(HexforgeError::NotFound)
}

/// Get an institution by its unique key.
///
/// `GET /api/v1/institutions/by-key/{key}`
pub async fn get_institution_by_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Institution>, HexforgeError> {
    let result = state
        .institution_ds
        .find_one(WhereClause::new("institution_key = $1").bind(key))
        .await
        .map_err(HexforgeError::data_source)?;

    result.map(Json).ok_or(HexforgeError::NotFound)
}

/// Get a school by its unique key.
///
/// `GET /api/v1/schools/by-key/{key}`
pub async fn get_school_by_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<School>, HexforgeError> {
    let result = state
        .school_ds
        .find_one(WhereClause::new("school_key = $1").bind(key))
        .await
        .map_err(HexforgeError::data_source)?;

    result.map(Json).ok_or(HexforgeError::NotFound)
}

/// Get a series by its unique key.
///
/// `GET /api/v1/series/by-key/{key}`
pub async fn get_series_by_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Series>, HexforgeError> {
    let result = state
        .series_ds
        .find_one(WhereClause::new("series_key = $1").bind(key))
        .await
        .map_err(HexforgeError::data_source)?;

    result.map(Json).ok_or(HexforgeError::NotFound)
}
