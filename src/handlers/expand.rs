//! Expansion logic for bibitem related data.
//!
//! Provides the expansion handler for `?expand=authors,journal,keywords,crossref`.
//!
//! # Supported expand fields
//!
//! - `authors` - Authors linked to the bibitem (role = author)
//! - `editors` - Editors linked to the bibitem (role = editor)
//! - `guesteditors` - Guest editors linked to the bibitem (role = guesteditor)
//! - `journal` - The journal this bibitem is published in
//! - `keywords` - Keywords (up to 3 levels) linked to the bibitem
//! - `crossref` - The crossref bibitem

use hexforge::db_exports::{FromRow, query_as};
use hexforge::{DataSource, ExpandRequest, HexforgeError, is_field_expanded};

use crate::domain::AuthorRole;
use crate::entities::{Author, BibItem, Journal, Keyword};
use crate::state::AppState;

/// List of supported expandable fields for bibitems.
pub const EXPANDABLE_FIELDS: &[&str] = &[
    "authors",
    "editors",
    "guesteditors",
    "journal",
    "keywords",
    "crossref",
];

/// Raw bibitem author from database.
#[derive(Debug, FromRow)]
struct RawBibItemAuthor {
    author_id: i64,
    position: i16,
}

/// Raw bibitem keyword from database.
#[derive(Debug, FromRow)]
struct RawBibItemKeyword {
    keyword_id: i64,
    keyword_level: i16,
}

/// Linked author with role and position in the expansion response.
#[derive(Debug, serde::Serialize)]
pub struct LinkedAuthor {
    #[serde(flatten)]
    pub author: Author,
    pub role: AuthorRole,
    pub position: i16,
}

/// Linked keywords response for expansion.
#[derive(Debug, serde::Serialize)]
pub struct LinkedKeywords {
    pub level_1: Option<Keyword>,
    pub level_2: Option<Keyword>,
    pub level_3: Option<Keyword>,
}

/// Expand a bibitem with requested related data.
///
/// This function is called by hexforge's CrudResourceConfig expand_handler.
/// It converts the bibitem to JSON and adds expanded fields based on the requests.
pub async fn expand_bibitem(
    state: &AppState,
    bibitem: BibItem,
    requests: &[ExpandRequest],
) -> Result<serde_json::Value, HexforgeError> {
    let bibitem_id = bibitem.id;
    let journal_id = bibitem.journal_id;
    let crossref_id = bibitem.crossref_id;

    // Start with the base bibitem response
    let mut response = serde_json::to_value(&bibitem)
        .map_err(|e| HexforgeError::internal(format!("JSON serialization error: {e}")))?;

    // Expand authors
    if is_field_expanded(requests, "authors") {
        let authors = fetch_bibitem_authors(state, bibitem_id, AuthorRole::Author).await?;
        response["authors"] = serde_json::to_value(&authors)
            .map_err(|e| HexforgeError::internal(format!("JSON serialization error: {e}")))?;
    }

    // Expand editors
    if is_field_expanded(requests, "editors") {
        let editors = fetch_bibitem_authors(state, bibitem_id, AuthorRole::Editor).await?;
        response["editors"] = serde_json::to_value(&editors)
            .map_err(|e| HexforgeError::internal(format!("JSON serialization error: {e}")))?;
    }

    // Expand guest editors
    if is_field_expanded(requests, "guesteditors") {
        let guesteditors =
            fetch_bibitem_authors(state, bibitem_id, AuthorRole::Guesteditor).await?;
        response["guesteditors"] = serde_json::to_value(&guesteditors)
            .map_err(|e| HexforgeError::internal(format!("JSON serialization error: {e}")))?;
    }

    // Expand journal
    if is_field_expanded(requests, "journal")
        && let Some(jid) = journal_id
        && let Some(journal) = fetch_journal(state, jid).await?
    {
        response["journal"] = serde_json::to_value(&journal)
            .map_err(|e| HexforgeError::internal(format!("JSON serialization error: {e}")))?;
    }

    // Expand keywords
    if is_field_expanded(requests, "keywords") {
        let keywords = fetch_bibitem_keywords(state, bibitem_id).await?;
        response["keywords"] = serde_json::to_value(&keywords)
            .map_err(|e| HexforgeError::internal(format!("JSON serialization error: {e}")))?;
    }

    // Expand crossref
    if is_field_expanded(requests, "crossref")
        && let Some(cid) = crossref_id
        && let Some(crossref) = fetch_crossref(state, cid).await?
    {
        response["crossref"] = serde_json::to_value(&crossref)
            .map_err(|e| HexforgeError::internal(format!("JSON serialization error: {e}")))?;
    }

    Ok(response)
}

/// Fetch authors/editors for a bibitem with a specific role.
async fn fetch_bibitem_authors(
    state: &AppState,
    bibitem_id: i64,
    role: AuthorRole,
) -> Result<Vec<LinkedAuthor>, HexforgeError> {
    let role_str = match role {
        AuthorRole::Author => "author",
        AuthorRole::Editor => "editor",
        AuthorRole::Guesteditor => "guesteditor",
    };

    let links: Vec<RawBibItemAuthor> = query_as(
        r#"
        SELECT author_id, position
        FROM bibitem_authors
        WHERE bibitem_id = $1 AND role::text = $2
        ORDER BY position, author_id
        "#,
    )
    .bind(bibitem_id)
    .bind(role_str)
    .fetch_all(state.pool.pool())
    .await
    .map_err(HexforgeError::data_source)?;

    let mut authors = Vec::with_capacity(links.len());
    for link in links {
        let author: Option<Author> = state
            .author_ds
            .find_by_id(&link.author_id)
            .await
            .map_err(HexforgeError::data_source)?;

        if let Some(author) = author {
            authors.push(LinkedAuthor {
                author,
                role,
                position: link.position,
            });
        }
    }

    Ok(authors)
}

/// Fetch journal by ID.
async fn fetch_journal(
    state: &AppState,
    journal_id: i64,
) -> Result<Option<Journal>, HexforgeError> {
    state
        .journal_ds
        .find_by_id(&journal_id)
        .await
        .map_err(HexforgeError::data_source)
}

/// Fetch keywords for a bibitem.
async fn fetch_bibitem_keywords(
    state: &AppState,
    bibitem_id: i64,
) -> Result<LinkedKeywords, HexforgeError> {
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

    let mut level_1_id = None;
    let mut level_2_id = None;
    let mut level_3_id = None;

    for row in rows {
        match row.keyword_level {
            1 => level_1_id = Some(row.keyword_id),
            2 => level_2_id = Some(row.keyword_id),
            3 => level_3_id = Some(row.keyword_id),
            _ => {}
        }
    }

    let level_1 = if let Some(id) = level_1_id {
        fetch_keyword(state, id).await?
    } else {
        None
    };

    let level_2 = if let Some(id) = level_2_id {
        fetch_keyword(state, id).await?
    } else {
        None
    };

    let level_3 = if let Some(id) = level_3_id {
        fetch_keyword(state, id).await?
    } else {
        None
    };

    Ok(LinkedKeywords {
        level_1,
        level_2,
        level_3,
    })
}

/// Fetch keyword by ID.
async fn fetch_keyword(
    state: &AppState,
    keyword_id: i64,
) -> Result<Option<Keyword>, HexforgeError> {
    state
        .keyword_ds
        .find_by_id(&keyword_id)
        .await
        .map_err(HexforgeError::data_source)
}

/// Fetch crossref bibitem by ID.
async fn fetch_crossref(
    state: &AppState,
    crossref_id: i64,
) -> Result<Option<BibItem>, HexforgeError> {
    state
        .bibitem_ds
        .find_by_id(&crossref_id)
        .await
        .map_err(HexforgeError::data_source)
}
