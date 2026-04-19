//! Import handlers — thin HTTP adapters for CSV import endpoints.
//!
//! Entity imports: `POST /api/v1/admin/import/{entity}`
//! Bibitem import: `POST /api/v1/admin/import/bibitems` (IDs format CSV)
//!
//! Requires Admin permission.

use hexforge::axum_exports::{IntoResponse, Json, Multipart, Query, Response, State, StatusCode};
use hexforge::{HexforgeError, ValidationError};
use serde::Deserialize;

use crate::adapters::import::{
    PgBibitemJunctionStore, PgBibitemNotesStore, PgBibitemRefsStore, PgNameVariantStore,
    PgReferenceStore, PgSequenceSyncer,
};
use crate::logic::import::{BibitemImportResult, ImportResponse};
use crate::process::import;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct EntityImportParams {
    #[serde(default)]
    pub auto_assign_ids: bool,
}

// =============================================================================
// CSV extraction helper (HTTP-specific — deals with multipart)
// =============================================================================

/// Extract CSV bytes from a multipart upload.
pub(crate) async fn extract_csv_bytes(mut multipart: Multipart) -> Result<Vec<u8>, HexforgeError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field
                .bytes()
                .await
                .map_err(|e| HexforgeError::Validation(ValidationError::custom(e.to_string())))?;
            return Ok(data.to_vec());
        }
    }
    Err(HexforgeError::Validation(ValidationError::custom(
        "No file field found in request. Send a multipart form with a 'file' field.",
    )))
}

// =============================================================================
// Entity import handlers
// =============================================================================

/// Import authors from CSV.
///
/// `POST /api/v1/admin/import/authors`
pub async fn import_authors(
    State(state): State<AppState>,
    Query(params): Query<EntityImportParams>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let syncer = PgSequenceSyncer::new(state.pool.pool());
    let result =
        import::import_authors_from_csv(&state.author_ds, &syncer, data, params.auto_assign_ids)
            .await?;
    Ok(Json(result))
}

/// Import journals from CSV.
///
/// `POST /api/v1/admin/import/journals`
pub async fn import_journals(
    State(state): State<AppState>,
    Query(params): Query<EntityImportParams>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let syncer = PgSequenceSyncer::new(state.pool.pool());
    let result =
        import::import_journals_from_csv(&state.journal_ds, &syncer, data, params.auto_assign_ids)
            .await?;
    Ok(Json(result))
}

/// Import publishers from CSV.
///
/// `POST /api/v1/admin/import/publishers`
pub async fn import_publishers(
    State(state): State<AppState>,
    Query(params): Query<EntityImportParams>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let syncer = PgSequenceSyncer::new(state.pool.pool());
    let result = import::import_publishers_from_csv(
        &state.publisher_ds,
        &syncer,
        data,
        params.auto_assign_ids,
    )
    .await?;
    Ok(Json(result))
}

/// Import institutions from CSV.
///
/// `POST /api/v1/admin/import/institutions`
pub async fn import_institutions(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let syncer = PgSequenceSyncer::new(state.pool.pool());
    let result = import::import_institutions_from_csv(&state.institution_ds, &syncer, data).await?;
    Ok(Json(result))
}

/// Import schools from CSV.
///
/// `POST /api/v1/admin/import/schools`
pub async fn import_schools(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let syncer = PgSequenceSyncer::new(state.pool.pool());
    let result = import::import_schools_from_csv(&state.school_ds, &syncer, data).await?;
    Ok(Json(result))
}

/// Import series from CSV.
///
/// `POST /api/v1/admin/import/series`
pub async fn import_series(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let syncer = PgSequenceSyncer::new(state.pool.pool());
    let result = import::import_series_from_csv(&state.series_ds, &syncer, data).await?;
    Ok(Json(result))
}

/// Import keywords from CSV.
///
/// `POST /api/v1/admin/import/keywords`
pub async fn import_keywords(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let syncer = PgSequenceSyncer::new(state.pool.pool());
    let result = import::import_keywords_from_csv(&state.keyword_ds, &syncer, data).await?;
    Ok(Json(result))
}

/// Import author name variants from CSV.
///
/// `POST /api/v1/admin/import/author-name-variants`
pub async fn import_author_name_variants(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let variant_store = PgNameVariantStore::new(state.pool.pool());
    let result =
        import::import_author_name_variants_from_csv(&state.author_ds, &variant_store, data)
            .await?;
    Ok(Json(result))
}

/// Import bibitems from CSV (IDs format).
///
/// `POST /api/v1/admin/import/bibitems`
///
/// Before inserting, validates ALL referenced IDs exist. If any are missing,
/// returns all missing IDs in a single 422 error and inserts nothing.
pub async fn import_bibitems(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Response, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let junction_store = PgBibitemJunctionStore::new(state.pool.pool());
    let ref_store = PgReferenceStore::new(state.pool.pool());
    let syncer = PgSequenceSyncer::new(state.pool.pool());
    let result = import::import_bibitems_from_csv(
        &state.bibitem_ds,
        &junction_store,
        &ref_store,
        &syncer,
        data,
    )
    .await?;

    match result {
        BibitemImportResult::Success(resp) => Ok(Json(resp).into_response()),
        BibitemImportResult::MissingReferences(missing) => {
            Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(missing)).into_response())
        }
    }
}

/// Import bibitem refs from CSV.
///
/// `POST /api/v1/admin/import/bibitem-refs`
pub async fn import_bibitem_refs(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let refs_store = PgBibitemRefsStore::new(state.pool.pool());
    let id_store = PgReferenceStore::new(state.pool.pool());
    let result = import::import_bibitem_refs_from_csv(&refs_store, &id_store, data).await?;
    Ok(Json(result))
}

/// Import bibitem notes from CSV.
///
/// `POST /api/v1/admin/import/bibitem-notes`
pub async fn import_bibitem_notes(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ImportResponse>, HexforgeError> {
    let data = extract_csv_bytes(multipart).await?;
    let notes_store = PgBibitemNotesStore::new(state.pool.pool());
    let id_store = PgReferenceStore::new(state.pool.pool());
    let result = import::import_bibitem_notes_from_csv(&notes_store, &id_store, data).await?;
    Ok(Json(result))
}
