use hexforge::HexforgeError;
use hexforge::db_exports::{PgPool, query_as};

use crate::domain::RefType;
use crate::domain::junctions::BibitemRefsRow;

pub async fn fetch_bibitem_refs_by_type(
    pool: &PgPool,
    ref_type: RefType,
) -> Result<Vec<BibitemRefsRow>, HexforgeError> {
    query_as::<_, BibitemRefsRow>(
        "SELECT source_key, target_key, ref_type \
         FROM bibitem_refs \
         WHERE ref_type = $1 \
         ORDER BY source_key, target_key",
    )
    .bind(ref_type)
    .fetch_all(pool)
    .await
    .map_err(HexforgeError::data_source)
}
