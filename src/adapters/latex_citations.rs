//! Postgres implementation of `CitationResolver`.

use std::collections::HashMap;

use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgPool, query_as};

use crate::logic::latex_citations::CitationData;
use crate::process::latex_citations::CitationResolver;

pub struct PgCitationResolver<'a> {
    pool: &'a PgPool,
}

impl<'a> PgCitationResolver<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct CitationRow {
    bibkey: String,
    display_name: Option<String>,
    date_year: Option<i16>,
}

impl CitationResolver for PgCitationResolver<'_> {
    async fn resolve_bibkeys(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, CitationData>, HexforgeError> {
        // Prefer author at position 1; fall back to editor at position 1.
        // String literals in JOIN ON clauses are auto-cast to the custom enum by PostgreSQL.
        let rows: Vec<CitationRow> = query_as(
            "SELECT b.bibkey,
                COALESCE(
                    COALESCE(a_auth.family_name_unicode, a_auth.mononym_unicode),
                    COALESCE(a_ed.family_name_unicode, a_ed.mononym_unicode)
                ) AS display_name,
                b.date_year
             FROM bibitems b
             LEFT JOIN bibitem_authors ba_auth
                ON ba_auth.bibitem_id = b.id
               AND ba_auth.role = 'author'::author_role
               AND ba_auth.position = 1
             LEFT JOIN authors a_auth ON a_auth.id = ba_auth.author_id
             LEFT JOIN bibitem_authors ba_ed
                ON ba_ed.bibitem_id = b.id
               AND ba_ed.role = 'editor'::author_role
               AND ba_ed.position = 1
             LEFT JOIN authors a_ed ON a_ed.id = ba_ed.author_id
             WHERE b.bibkey = ANY($1)",
        )
        .bind(keys)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;

        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.bibkey,
                    CitationData {
                        author: r.display_name,
                        year: r.date_year,
                    },
                )
            })
            .collect())
    }
}
