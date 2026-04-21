//! Postgres implementation of bibitem search using pg_trgm similarity.
//!
//! Implements `BibitemSearcher` from the process layer. All SQL, sqlx types,
//! and Postgres-specific logic lives here.

use hexforge::HexforgeError;
use hexforge::db_exports::{Arguments, FromRow, PgArguments, PgPool, query_as_with};

use crate::domain::BibItem;
use crate::logic::search::{SearchRequest, SearchResponse};
use crate::process::search::BibitemSearcher;

/// Minimum similarity threshold for search results (0.0 to 1.0).
const SIMILARITY_THRESHOLD: f32 = 0.1;

/// Row type for the search query that includes the total count window function.
#[derive(Debug, FromRow)]
struct SearchRow {
    #[sqlx(flatten)]
    bibitem: BibItem,
    total_count: i64,
}

/// Postgres-backed bibitem searcher using pg_trgm similarity.
pub struct PgBibitemSearcher<'a> {
    pool: &'a PgPool,
}

impl<'a> PgBibitemSearcher<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl BibitemSearcher for PgBibitemSearcher<'_> {
    async fn search(&self, request: &SearchRequest) -> Result<SearchResponse, HexforgeError> {
        let limit = request.limit;
        let offset = request.offset;

        let mut conditions = vec!["1=1".to_string()];
        let mut param_idx: usize = 1;

        if !request.query.is_empty() {
            conditions.push(format!(
                r#"GREATEST(
                COALESCE(similarity(title_unicode, ${param_idx}), 0),
                COALESCE(similarity(booktitle_unicode, ${param_idx}), 0)
            ) >= {SIMILARITY_THRESHOLD}"#
            ));
            param_idx += 1;
        }

        if request.entry_type.is_some() {
            conditions.push(format!("entry_type = ${param_idx}"));
            param_idx += 1;
        }

        if request.year_from.is_some() {
            conditions.push(format!("date_year >= ${param_idx}"));
            param_idx += 1;
        }

        if request.year_to.is_some() {
            conditions.push(format!("date_year <= ${param_idx}"));
            param_idx += 1;
        }

        if request.author_id.is_some() {
            conditions.push(format!(
                "bibkey IN (SELECT bibkey FROM bibitem_authors WHERE author_key = ${param_idx})"
            ));
            param_idx += 1;
        }

        if request.journal_id.is_some() {
            conditions.push(format!("journal_id = ${param_idx}"));
            param_idx += 1;
        }

        if request.epoch.is_some() {
            conditions.push(format!("epoch = ${param_idx}"));
            param_idx += 1;
        }

        let where_clause = conditions.join(" AND ");

        let order_clause = if request.query.is_empty() {
            "date_year DESC NULLS LAST, id DESC".to_string()
        } else {
            r#"GREATEST(
                COALESCE(similarity(title_unicode, $1), 0),
                COALESCE(similarity(booktitle_unicode, $1), 0)
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

        let mut args = PgArguments::default();

        if !request.query.is_empty() {
            args.add(&request.query)
                .map_err(|e| HexforgeError::internal(e.to_string()))?;
        }
        if let Some(entry_type) = request.entry_type {
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
        if let Some(epoch) = request.epoch {
            args.add(epoch)
                .map_err(|e| HexforgeError::internal(e.to_string()))?;
        }
        args.add(limit)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;
        args.add(offset)
            .map_err(|e| HexforgeError::internal(e.to_string()))?;

        let rows: Vec<SearchRow> = query_as_with::<_, SearchRow, _>(&sql, args)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;

        let total = rows.first().map_or(0, |r| r.total_count);
        let results: Vec<BibItem> = rows.into_iter().map(|r| r.bibitem).collect();

        Ok(SearchResponse {
            results,
            total,
            limit,
            offset,
        })
    }
}
