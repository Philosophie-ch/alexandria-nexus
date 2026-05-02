use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgPool, query, query_as};

use crate::process::compute_start_pages::{
    BibitemPagesFetcher, BibitemPagesRow, StartPageUpdate, StartPageWriter,
};

#[derive(FromRow)]
struct PagesRow {
    id: i64,
    pages: Option<String>,
}

pub struct PgStartPageComputer<'a> {
    pool: &'a PgPool,
}

impl<'a> PgStartPageComputer<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl BibitemPagesFetcher for PgStartPageComputer<'_> {
    async fn fetch_pages(&self) -> Result<Vec<BibitemPagesRow>, HexforgeError> {
        query_as::<_, PagesRow>("SELECT id, pages FROM bibitems")
            .fetch_all(self.pool)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| BibitemPagesRow {
                        id: r.id,
                        pages: r.pages,
                    })
                    .collect()
            })
            .map_err(HexforgeError::data_source)
    }
}

impl StartPageWriter for PgStartPageComputer<'_> {
    async fn write_start_pages(&self, updates: &[StartPageUpdate]) -> Result<usize, HexforgeError> {
        if updates.is_empty() {
            return Ok(0);
        }
        let (ids, values): (Vec<i64>, Vec<Option<i32>>) =
            updates.iter().map(|u| (u.id, u.start_page)).unzip();
        let rows_affected = query(
            "UPDATE bibitems SET start_page = u.value \
             FROM unnest($1::int8[], $2::int4[]) AS u(id, value) \
             WHERE bibitems.id = u.id",
        )
        .bind(ids)
        .bind(values)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?
        .rows_affected();
        Ok(usize::try_from(rows_affected).expect("rows_affected fits in usize on 64-bit"))
    }
}
