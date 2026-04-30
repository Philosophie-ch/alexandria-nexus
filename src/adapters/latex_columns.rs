//! Postgres implementations of `LatexColumnFetcher` and `LatexColumnWriter`.

use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgPool, Postgres, Transaction, query, query_as};

use crate::process::latex_columns::{ColumnBatch, LatexColumnFetcher, LatexColumnWriter};

/// `(table, latex_col, unicode_col)` — all 15 pairs to process.
const COLUMN_SPECS: &[(&str, &str, &str)] = &[
    ("bibitems", "title_latex", "title_unicode"),
    ("bibitems", "booktitle_latex", "booktitle_unicode"),
    ("bibitems", "note_latex", "note_unicode"),
    ("bibitems", "issuetitle_latex", "issuetitle_unicode"),
    ("bibitems", "extra_note_latex", "extra_note_unicode"),
    ("authors", "family_name_latex", "family_name_unicode"),
    ("authors", "given_name_latex", "given_name_unicode"),
    ("authors", "mononym_latex", "mononym_unicode"),
    ("authors", "famous_name_latex", "famous_name_unicode"),
    ("authors", "shorthand_latex", "shorthand_unicode"),
    ("journals", "name_latex", "name_unicode"),
    ("publishers", "name_latex", "name_unicode"),
    ("institutions", "name_latex", "name_unicode"),
    ("schools", "name_latex", "name_unicode"),
    ("series", "name_latex", "name_unicode"),
];

#[derive(Debug, FromRow)]
struct LatexRow {
    id: i64,
    latex: String,
}

pub struct PgLatexColumnConverter<'a> {
    pool: &'a PgPool,
}

impl<'a> PgLatexColumnConverter<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Fetch all non-null latex values for one column.
    async fn fetch_column(
        &self,
        table: &'static str,
        latex_col: &'static str,
    ) -> Result<Vec<LatexRow>, HexforgeError> {
        let sql =
            format!("SELECT id, {latex_col} AS latex FROM {table} WHERE {latex_col} IS NOT NULL");
        query_as::<_, LatexRow>(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)
    }

    /// Write converted unicode values for one column.
    async fn update_column(
        &self,
        table: &'static str,
        unicode_col: &'static str,
        updates: &[(i64, String)],
    ) -> Result<(), HexforgeError> {
        if updates.is_empty() {
            return Ok(());
        }
        let (ids, values): (Vec<i64>, Vec<String>) = updates.iter().cloned().unzip();
        let sql = format!(
            "UPDATE {table} SET {unicode_col} = u.value \
             FROM unnest($1::int8[], $2::text[]) AS u(id, value) \
             WHERE {table}.id = u.id"
        );
        let mut tx: Transaction<Postgres> = self
            .pool
            .begin()
            .await
            .map_err(HexforgeError::data_source)?;
        query(&sql)
            .bind(ids)
            .bind(values)
            .execute(&mut *tx)
            .await
            .map_err(HexforgeError::data_source)?;
        tx.commit().await.map_err(HexforgeError::data_source)
    }
}

impl LatexColumnFetcher for PgLatexColumnConverter<'_> {
    async fn fetch_all_latex_columns(&self) -> Result<Vec<ColumnBatch>, HexforgeError> {
        let fetcher = self;
        futures::future::try_join_all(COLUMN_SPECS.iter().map(
            |&(table, latex_col, unicode_col)| async move {
                let rows = fetcher.fetch_column(table, latex_col).await?;
                Ok::<ColumnBatch, HexforgeError>(ColumnBatch {
                    entity: table,
                    field: unicode_col,
                    rows: rows.into_iter().map(|r| (r.id, r.latex)).collect(),
                })
            },
        ))
        .await
    }
}

impl LatexColumnWriter for PgLatexColumnConverter<'_> {
    async fn write_unicode_field(
        &self,
        entity: &'static str,
        field: &'static str,
        updates: &[(i64, String)],
    ) -> Result<usize, HexforgeError> {
        self.update_column(entity, field, updates).await?;
        Ok(updates.len())
    }
}
