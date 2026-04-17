//! Postgres implementation of bibitem search using pg_trgm similarity.
//!
//! Implements `BibitemSearcher` from the process layer. All SQL, sqlx types,
//! and Postgres-specific logic lives here.

use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgArguments, PgPool};

use crate::domain::BibItem;
use crate::domain::{EntryType, Epoch, LangId, PubState};
use crate::logic::search::{SearchRequest, SearchResponse};
use crate::process::search::BibitemSearcher;

/// Minimum similarity threshold for search results (0.0 to 1.0).
const SIMILARITY_THRESHOLD: f32 = 0.1;

/// Row type for the search query that includes the total count window function.
#[derive(Debug, FromRow)]
struct SearchRow {
    id: i64,
    bibkey: String,
    entry_type: EntryType,
    date_year: Option<i16>,
    date_year_2_hyphen: Option<i16>,
    date_year_2_slash: Option<i16>,
    date_month: Option<i16>,
    date_day: Option<i16>,
    date_is_no_date: bool,
    pubstate: Option<PubState>,
    title_latex: String,
    title_unicode: String,
    booktitle_latex: Option<String>,
    booktitle_unicode: Option<String>,
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
    langid: Option<LangId>,
    is_translation: bool,
    epoch: Option<Epoch>,
    options: Option<String>,
    shorthand: Option<String>,
    person_id: Option<i64>,
    has_fulltext: bool,
    fulltext_path: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
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
            conditions.push(format!("entry_type::text = ${param_idx}"));
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
                "id IN (SELECT bibitem_id FROM bibitem_authors WHERE author_id = ${param_idx})"
            ));
            param_idx += 1;
        }

        if request.journal_id.is_some() {
            conditions.push(format!("journal_id = ${param_idx}"));
            param_idx += 1;
        }

        if request.epoch.is_some() {
            conditions.push(format!("epoch::text = ${param_idx}"));
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
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;

        let total = rows.first().map_or(0, |r| r.total_count);
        let results: Vec<BibItem> = rows.into_iter().map(search_row_to_bibitem).collect();

        Ok(SearchResponse {
            results,
            total,
            limit,
            offset,
        })
    }
}

fn search_row_to_bibitem(row: SearchRow) -> BibItem {
    BibItem {
        id: row.id,
        bibkey: row.bibkey,
        entry_type: row.entry_type,
        date_year: row.date_year,
        date_year_2_hyphen: row.date_year_2_hyphen,
        date_year_2_slash: row.date_year_2_slash,
        date_month: row.date_month,
        date_day: row.date_day,
        date_is_no_date: row.date_is_no_date,
        pubstate: row.pubstate,
        title_latex: row.title_latex,
        title_unicode: row.title_unicode,
        booktitle_latex: row.booktitle_latex,
        booktitle_unicode: row.booktitle_unicode,
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
        langid: row.langid,
        is_translation: row.is_translation,
        epoch: row.epoch,
        options: row.options,
        shorthand: row.shorthand,
        person_id: row.person_id,
        has_fulltext: row.has_fulltext,
        fulltext_path: row.fulltext_path,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
