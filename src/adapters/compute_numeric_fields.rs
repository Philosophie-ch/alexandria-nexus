use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgPool, query, query_as};

use crate::process::compute_numeric_fields::{
    BibitemTextFieldsFetcher, BibitemTextFieldsRow, NumericFieldsUpdate, NumericFieldsWriter,
};

#[derive(FromRow)]
struct TextFieldsRow {
    id: i64,
    pages: Option<String>,
    volume: Option<String>,
    number: Option<String>,
}

pub struct PgNumericFieldComputer<'a> {
    pool: &'a PgPool,
}

impl<'a> PgNumericFieldComputer<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

impl BibitemTextFieldsFetcher for PgNumericFieldComputer<'_> {
    async fn fetch_text_fields(&self) -> Result<Vec<BibitemTextFieldsRow>, HexforgeError> {
        query_as::<_, TextFieldsRow>("SELECT id, pages, volume, number FROM bibitems")
            .fetch_all(self.pool)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| BibitemTextFieldsRow {
                        id: r.id,
                        pages: r.pages,
                        volume: r.volume,
                        number: r.number,
                    })
                    .collect()
            })
            .map_err(HexforgeError::data_source)
    }
}

impl NumericFieldsWriter for PgNumericFieldComputer<'_> {
    async fn write_numeric_fields(
        &self,
        updates: &[NumericFieldsUpdate],
    ) -> Result<usize, HexforgeError> {
        if updates.is_empty() {
            return Ok(0);
        }
        let mut ids = Vec::with_capacity(updates.len());
        let mut start_pages = Vec::with_capacity(updates.len());
        let mut volume_numerics = Vec::with_capacity(updates.len());
        let mut number_numerics = Vec::with_capacity(updates.len());
        for u in updates {
            ids.push(u.id);
            start_pages.push(u.start_page);
            volume_numerics.push(u.volume_numeric);
            number_numerics.push(u.number_numeric);
        }
        let rows_affected = query(
            "UPDATE bibitems SET \
                 start_page = u.sp, \
                 volume_numeric = u.vn, \
                 number_numeric = u.nn \
             FROM unnest($1::int8[], $2::int4[], $3::int4[], $4::int4[]) \
                 AS u(id, sp, vn, nn) \
             WHERE bibitems.id = u.id",
        )
        .bind(ids)
        .bind(start_pages)
        .bind(volume_numerics)
        .bind(number_numerics)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?
        .rows_affected();
        Ok(usize::try_from(rows_affected).expect("rows_affected fits in usize on 64-bit"))
    }
}
