//! Import handler for bulk bibitem creation.
//!
//! `POST /api/v1/admin/import`
//!
//! Accepts a JSON array of bibitems and creates them in batch.
//! Requires Admin permission.

use hexforge::axum_exports::{Json, State};
use hexforge::{DataSource, HexforgeError};
use serde::{Deserialize, Serialize};

use crate::domain::{BibItem, CreateBibItem, create_bib_item_transform};
use crate::state::AppState;
use crate::validation::validate_create_bibitem;

/// Import request containing bibitems to create.
#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    /// List of bibitems to import.
    pub bibitems: Vec<CreateBibItem>,
}

/// Single import error with index and message.
#[derive(Debug, Serialize)]
pub struct ImportError {
    /// Index of the failed item in the input array.
    pub index: usize,
    /// The bibkey for identification.
    pub bibkey: String,
    /// Error message.
    pub error: String,
}

/// Import result with counts and any errors.
#[derive(Debug, Serialize)]
pub struct ImportResponse {
    /// Number of successfully imported items.
    pub imported: usize,
    /// Number of failed items.
    pub failed: usize,
    /// List of errors for failed items.
    pub errors: Vec<ImportError>,
    /// Successfully imported items (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<BibItem>>,
}

/// Import bibitems in batch.
///
/// `POST /api/v1/admin/import`
///
/// This handler:
/// 1. Validates all items first (fail-fast on validation)
/// 2. Transforms DTOs to domain entities
/// 3. Inserts using the DataSource
/// 4. Returns count of imported items and any errors
pub async fn import_bibitems(
    State(state): State<AppState>,
    Json(request): Json<ImportRequest>,
) -> Result<Json<ImportResponse>, HexforgeError> {
    if request.bibitems.is_empty() {
        return Ok(Json(ImportResponse {
            imported: 0,
            failed: 0,
            errors: vec![],
            items: None,
        }));
    }

    let mut errors = Vec::new();
    let mut valid_items = Vec::new();

    // Phase 1: Validate all items
    for (index, dto) in request.bibitems.iter().enumerate() {
        match validate_create_bibitem(dto) {
            Ok(()) => {
                let bibitem = create_bib_item_transform(dto.clone());
                valid_items.push((index, dto.bibkey.clone(), bibitem));
            }
            Err(e) => {
                errors.push(ImportError {
                    index,
                    bibkey: dto.bibkey.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    // If no valid items, return early
    if valid_items.is_empty() {
        return Ok(Json(ImportResponse {
            imported: 0,
            failed: errors.len(),
            errors,
            items: None,
        }));
    }

    // Phase 2: Insert all valid items
    let mut imported_items = Vec::with_capacity(valid_items.len());

    for (index, bibkey, bibitem) in valid_items {
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

                errors.push(ImportError {
                    index,
                    bibkey,
                    error: formatted_msg,
                });
            }
        }
    }

    Ok(Json(ImportResponse {
        imported: imported_items.len(),
        failed: errors.len(),
        errors,
        items: if imported_items.is_empty() {
            None
        } else {
            Some(imported_items)
        },
    }))
}
