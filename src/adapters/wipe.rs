use hexforge::HexforgeError;
use hexforge::db_exports::{PgPool, query};

pub async fn wipe_tables(pool: &PgPool) -> Result<(), HexforgeError> {
    query(
        "TRUNCATE TABLE \
         bibitem_notes, bibitem_refs, bibitem_keywords, bibitem_authors, \
         bibitems, authors, journals, publishers, institutions, schools, series, keywords, \
         data_version",
    )
    .execute(pool)
    .await
    .map_err(HexforgeError::data_source)?;

    Ok(())
}
