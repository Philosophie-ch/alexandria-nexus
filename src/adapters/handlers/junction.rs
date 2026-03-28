//! Junction table handlers for bibitem relationships.

use hexforge::axum_exports::{Json, Path, State};
use hexforge::db_exports::{FromRow, query, query_as};
use hexforge::{HexforgeError, RelationChange, ValidationError};
use serde::{Deserialize, Serialize};

use crate::domain::AuthorRole;
use crate::state::AppState;

// ============================================================================
// DTOs
// ============================================================================

/// Request to add an author to a bibitem.
#[derive(Debug, Deserialize)]
pub struct AddAuthorRequest {
    pub author_id: i64,
    #[serde(default)]
    pub role: AuthorRole,
    #[serde(default)]
    pub position: i16,
}

/// Request to replace all authors of a bibitem.
#[derive(Debug, Deserialize)]
pub struct ReplaceAuthorsRequest {
    pub authors: Vec<AuthorLink>,
}

/// An author link with role and position.
#[derive(Debug, Deserialize, Serialize)]
pub struct AuthorLink {
    pub author_id: i64,
    #[serde(default)]
    pub role: AuthorRole,
    #[serde(default)]
    pub position: i16,
}

/// Response for bibitem authors.
#[derive(Debug, Serialize)]
pub struct BibItemAuthorsResponse {
    pub bibitem_id: i64,
    pub authors: Vec<AuthorLink>,
}

/// Request to set keywords for a bibitem.
#[derive(Debug, Deserialize)]
pub struct SetKeywordsRequest {
    pub keyword_level_1_id: Option<i64>,
    pub keyword_level_2_id: Option<i64>,
    pub keyword_level_3_id: Option<i64>,
}

/// Keyword IDs by level.
#[derive(Debug, Serialize)]
pub struct BibItemKeywords {
    pub level_1: Option<i64>,
    pub level_2: Option<i64>,
    pub level_3: Option<i64>,
}

/// Response for bibitem keywords.
#[derive(Debug, Serialize)]
pub struct BibItemKeywordsResponse {
    pub bibitem_id: i64,
    pub keywords: BibItemKeywords,
}

// ============================================================================
// Raw DB types
// ============================================================================

/// Composite key for author relations (used by RelationChange::sync).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AuthorRelationKey {
    author_id: i64,
    role: AuthorRole,
}

#[derive(Debug, FromRow)]
struct RawBibItemAuthor {
    author_id: i64,
    role: String,
    position: i16,
}

#[derive(Debug, FromRow)]
struct RawBibItemKeyword {
    keyword_id: i64,
    keyword_level: i16,
}

// ============================================================================
// Author Junction Handlers
// ============================================================================

/// Get all authors for a bibitem.
///
/// `GET /api/v1/bibitems/:id/authors`
pub async fn get_bibitem_authors(
    State(state): State<AppState>,
    Path(bibitem_id): Path<i64>,
) -> Result<Json<BibItemAuthorsResponse>, HexforgeError> {
    verify_bibitem_exists(&state, bibitem_id).await?;

    let authors: Vec<RawBibItemAuthor> = query_as(
        r#"
        SELECT author_id, role::text as role, position
        FROM bibitem_authors
        WHERE bibitem_id = $1
        ORDER BY position, author_id
        "#,
    )
    .bind(bibitem_id)
    .fetch_all(state.pool.pool())
    .await
    .map_err(HexforgeError::data_source)?;

    let author_links = authors
        .into_iter()
        .map(|a| AuthorLink {
            author_id: a.author_id,
            role: parse_author_role(&a.role),
            position: a.position,
        })
        .collect();

    Ok(Json(BibItemAuthorsResponse {
        bibitem_id,
        authors: author_links,
    }))
}

/// Add an author to a bibitem.
///
/// `POST /api/v1/bibitems/:id/authors`
pub async fn add_author_to_bibitem(
    State(state): State<AppState>,
    Path(bibitem_id): Path<i64>,
    Json(payload): Json<AddAuthorRequest>,
) -> Result<Json<BibItemAuthorsResponse>, HexforgeError> {
    verify_bibitem_exists(&state, bibitem_id).await?;
    verify_author_exists(&state, payload.author_id).await?;

    let role_str = author_role_to_str(&payload.role);

    query(
        r#"
        INSERT INTO bibitem_authors (bibitem_id, author_id, role, position)
        VALUES ($1, $2, $3::author_role, $4)
        ON CONFLICT (bibitem_id, author_id, role) DO UPDATE SET position = $4
        "#,
    )
    .bind(bibitem_id)
    .bind(payload.author_id)
    .bind(role_str)
    .bind(payload.position)
    .execute(state.pool.pool())
    .await
    .map_err(HexforgeError::data_source)?;

    // Return updated list
    get_bibitem_authors(State(state), Path(bibitem_id)).await
}

/// Remove an author from a bibitem.
///
/// `DELETE /api/v1/bibitems/:id/authors/:author_id`
pub async fn remove_author_from_bibitem(
    State(state): State<AppState>,
    Path((bibitem_id, author_id)): Path<(i64, i64)>,
) -> Result<Json<BibItemAuthorsResponse>, HexforgeError> {
    let result = query("DELETE FROM bibitem_authors WHERE bibitem_id = $1 AND author_id = $2")
        .bind(bibitem_id)
        .bind(author_id)
        .execute(state.pool.pool())
        .await
        .map_err(HexforgeError::data_source)?;

    if result.rows_affected() == 0 {
        return Err(HexforgeError::NotFound);
    }

    // Return updated list
    get_bibitem_authors(State(state), Path(bibitem_id)).await
}

/// Replace all authors of a bibitem using efficient diff-based sync.
///
/// `PUT /api/v1/bibitems/:id/authors`
///
/// Uses hexforge's `RelationChange::sync()` to calculate minimal changes,
/// only deleting removed authors and inserting new ones.
pub async fn replace_bibitem_authors(
    State(state): State<AppState>,
    Path(bibitem_id): Path<i64>,
    Json(payload): Json<ReplaceAuthorsRequest>,
) -> Result<Json<BibItemAuthorsResponse>, HexforgeError> {
    verify_bibitem_exists(&state, bibitem_id).await?;

    // Verify all requested authors exist before starting transaction
    for author in &payload.authors {
        verify_author_exists(&state, author.author_id).await?;
    }

    // Fetch current authors
    let current_authors: Vec<RawBibItemAuthor> = query_as(
        r#"
        SELECT author_id, role::text as role, position
        FROM bibitem_authors
        WHERE bibitem_id = $1
        "#,
    )
    .bind(bibitem_id)
    .fetch_all(state.pool.pool())
    .await
    .map_err(HexforgeError::data_source)?;

    // Build current and desired relation keys
    let current_keys: Vec<AuthorRelationKey> = current_authors
        .iter()
        .map(|a| AuthorRelationKey {
            author_id: a.author_id,
            role: parse_author_role(&a.role),
        })
        .collect();

    let desired_keys: Vec<AuthorRelationKey> = payload
        .authors
        .iter()
        .map(|a| AuthorRelationKey {
            author_id: a.author_id,
            role: a.role,
        })
        .collect();

    // Calculate minimal changes using RelationChange::sync
    let changes = RelationChange::sync(&current_keys, &desired_keys);

    // Build lookup for desired positions
    let desired_positions: std::collections::HashMap<AuthorRelationKey, i16> = payload
        .authors
        .iter()
        .map(|a| {
            (
                AuthorRelationKey {
                    author_id: a.author_id,
                    role: a.role,
                },
                a.position,
            )
        })
        .collect();

    // Start transaction
    let mut tx = state
        .pool
        .pool()
        .begin()
        .await
        .map_err(HexforgeError::data_source)?;

    // Delete removed authors
    for key in &changes.detach {
        let role_str = author_role_to_str(&key.role);
        query(
            "DELETE FROM bibitem_authors WHERE bibitem_id = $1 AND author_id = $2 AND role = $3::author_role",
        )
        .bind(bibitem_id)
        .bind(key.author_id)
        .bind(role_str)
        .execute(&mut *tx)
        .await
        .map_err(HexforgeError::data_source)?;
    }

    // Insert new authors
    for key in &changes.attach {
        let role_str = author_role_to_str(&key.role);
        let position = desired_positions.get(key).copied().unwrap_or(0);

        query(
            r#"
            INSERT INTO bibitem_authors (bibitem_id, author_id, role, position)
            VALUES ($1, $2, $3::author_role, $4)
            "#,
        )
        .bind(bibitem_id)
        .bind(key.author_id)
        .bind(role_str)
        .bind(position)
        .execute(&mut *tx)
        .await
        .map_err(HexforgeError::data_source)?;
    }

    // Update positions for existing authors that weren't removed
    for key in &current_keys {
        if !changes.detach.contains(key)
            && let Some(&new_position) = desired_positions.get(key)
        {
            let current_position = current_authors
                .iter()
                .find(|a| a.author_id == key.author_id && parse_author_role(&a.role) == key.role)
                .map(|a| a.position)
                .unwrap_or(0);

            if new_position != current_position {
                let role_str = author_role_to_str(&key.role);
                query(
                    "UPDATE bibitem_authors SET position = $1 WHERE bibitem_id = $2 AND author_id = $3 AND role = $4::author_role",
                )
                .bind(new_position)
                .bind(bibitem_id)
                .bind(key.author_id)
                .bind(role_str)
                .execute(&mut *tx)
                .await
                .map_err(HexforgeError::data_source)?;
            }
        }
    }

    tx.commit().await.map_err(HexforgeError::data_source)?;

    // Return updated list
    get_bibitem_authors(State(state), Path(bibitem_id)).await
}

// ============================================================================
// Keyword Junction Handlers
// ============================================================================

/// Get keywords for a bibitem.
///
/// `GET /api/v1/bibitems/:id/keywords`
pub async fn get_bibitem_keywords(
    State(state): State<AppState>,
    Path(bibitem_id): Path<i64>,
) -> Result<Json<BibItemKeywordsResponse>, HexforgeError> {
    verify_bibitem_exists(&state, bibitem_id).await?;

    let rows: Vec<RawBibItemKeyword> = query_as(
        r#"
        SELECT keyword_id, keyword_level
        FROM bibitem_keywords
        WHERE bibitem_id = $1
        "#,
    )
    .bind(bibitem_id)
    .fetch_all(state.pool.pool())
    .await
    .map_err(HexforgeError::data_source)?;

    let mut level_1 = None;
    let mut level_2 = None;
    let mut level_3 = None;

    for row in rows {
        match row.keyword_level {
            1 => level_1 = Some(row.keyword_id),
            2 => level_2 = Some(row.keyword_id),
            3 => level_3 = Some(row.keyword_id),
            _ => {} // Ignore invalid levels
        }
    }

    Ok(Json(BibItemKeywordsResponse {
        bibitem_id,
        keywords: BibItemKeywords {
            level_1,
            level_2,
            level_3,
        },
    }))
}

/// Set keywords for a bibitem.
///
/// `POST /api/v1/bibitems/:id/keywords`
pub async fn set_bibitem_keywords(
    State(state): State<AppState>,
    Path(bibitem_id): Path<i64>,
    Json(payload): Json<SetKeywordsRequest>,
) -> Result<Json<BibItemKeywordsResponse>, HexforgeError> {
    verify_bibitem_exists(&state, bibitem_id).await?;

    // Verify keywords exist if provided
    if let Some(id) = payload.keyword_level_1_id {
        verify_keyword_exists(&state, id, 1).await?;
    }
    if let Some(id) = payload.keyword_level_2_id {
        verify_keyword_exists(&state, id, 2).await?;
    }
    if let Some(id) = payload.keyword_level_3_id {
        verify_keyword_exists(&state, id, 3).await?;
    }

    // Start transaction
    let mut tx = state
        .pool
        .pool()
        .begin()
        .await
        .map_err(HexforgeError::data_source)?;

    // Delete existing keywords for this bibitem
    query("DELETE FROM bibitem_keywords WHERE bibitem_id = $1")
        .bind(bibitem_id)
        .execute(&mut *tx)
        .await
        .map_err(HexforgeError::data_source)?;

    // Insert new keywords
    if let Some(keyword_id) = payload.keyword_level_1_id {
        query("INSERT INTO bibitem_keywords (bibitem_id, keyword_id, keyword_level) VALUES ($1, $2, 1)")
            .bind(bibitem_id)
            .bind(keyword_id)
            .execute(&mut *tx)
            .await
            .map_err(HexforgeError::data_source)?;
    }

    if let Some(keyword_id) = payload.keyword_level_2_id {
        query("INSERT INTO bibitem_keywords (bibitem_id, keyword_id, keyword_level) VALUES ($1, $2, 2)")
            .bind(bibitem_id)
            .bind(keyword_id)
            .execute(&mut *tx)
            .await
            .map_err(HexforgeError::data_source)?;
    }

    if let Some(keyword_id) = payload.keyword_level_3_id {
        query("INSERT INTO bibitem_keywords (bibitem_id, keyword_id, keyword_level) VALUES ($1, $2, 3)")
            .bind(bibitem_id)
            .bind(keyword_id)
            .execute(&mut *tx)
            .await
            .map_err(HexforgeError::data_source)?;
    }

    tx.commit().await.map_err(HexforgeError::data_source)?;

    // Return updated keywords
    get_bibitem_keywords(State(state), Path(bibitem_id)).await
}

// ============================================================================
// Helper functions
// ============================================================================

async fn verify_bibitem_exists(state: &AppState, bibitem_id: i64) -> Result<(), HexforgeError> {
    let exists: Option<(i64,)> = query_as("SELECT id FROM bibitems WHERE id = $1")
        .bind(bibitem_id)
        .fetch_optional(state.pool.pool())
        .await
        .map_err(HexforgeError::data_source)?;

    if exists.is_none() {
        return Err(HexforgeError::NotFound);
    }
    Ok(())
}

async fn verify_author_exists(state: &AppState, author_id: i64) -> Result<(), HexforgeError> {
    let exists: Option<(i64,)> = query_as("SELECT id FROM authors WHERE id = $1")
        .bind(author_id)
        .fetch_optional(state.pool.pool())
        .await
        .map_err(HexforgeError::data_source)?;

    if exists.is_none() {
        return Err(HexforgeError::Validation(ValidationError::invalid_value(
            "author_id",
            format!("Author {author_id} not found"),
        )));
    }
    Ok(())
}

async fn verify_keyword_exists(
    state: &AppState,
    keyword_id: i64,
    expected_level: i16,
) -> Result<(), HexforgeError> {
    let keyword: Option<(i64, i16)> = query_as("SELECT id, level FROM keywords WHERE id = $1")
        .bind(keyword_id)
        .fetch_optional(state.pool.pool())
        .await
        .map_err(HexforgeError::data_source)?;

    match keyword {
        None => Err(HexforgeError::Validation(ValidationError::invalid_value(
            "keyword_id",
            format!("Keyword {keyword_id} not found"),
        ))),
        Some((_, level)) if level != expected_level => {
            Err(HexforgeError::Validation(ValidationError::invalid_value(
                "keyword_id",
                format!("Keyword {keyword_id} is level {level}, expected level {expected_level}"),
            )))
        }
        _ => Ok(()),
    }
}

fn parse_author_role(role: &str) -> AuthorRole {
    match role {
        "author" => AuthorRole::Author,
        "editor" => AuthorRole::Editor,
        "guesteditor" => AuthorRole::Guesteditor,
        _ => AuthorRole::Author,
    }
}

fn author_role_to_str(role: &AuthorRole) -> &'static str {
    match role {
        AuthorRole::Author => "author",
        AuthorRole::Editor => "editor",
        AuthorRole::Guesteditor => "guesteditor",
    }
}
