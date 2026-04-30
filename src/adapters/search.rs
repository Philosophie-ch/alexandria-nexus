//! Postgres implementation of bibitem search.
//!
//! Implements `BibitemSearcher` from the process layer using hexforge's
//! `find_by_similarity` — pg_trgm scoring and COUNT(*) OVER() are handled
//! by hexforge internally.

use hexforge::db_exports::PgPool;
use hexforge::{
    DataSourceError, HexforgeError, Pagination, ParamBinder, PgQuery, SortOrder, TextSearch,
};

use crate::domain::BibItem;
use crate::domain::{EntryType, Epoch};
use crate::logic::search::{SearchRequest, SearchResponse};
use crate::process::search::BibitemSearcher;
use hexforge::DataStore;

/// Filter conditions for bibitem search (non-text filters).
#[derive(Debug, Default)]
struct BibitemSearchFilters {
    entry_type: Option<EntryType>,
    year_from: Option<i16>,
    year_to: Option<i16>,
    author_id: Option<i64>,
    journal_id: Option<i64>,
    epoch: Option<Epoch>,
}

impl PgQuery for BibitemSearchFilters {
    fn build_conditions(&self, mut idx: usize) -> (Vec<String>, usize) {
        let mut conditions = vec![];
        if self.entry_type.is_some() {
            conditions.push(format!("entry_type = ${idx}"));
            idx += 1;
        }
        if self.year_from.is_some() {
            conditions.push(format!("date_year >= ${idx}"));
            idx += 1;
        }
        if self.year_to.is_some() {
            conditions.push(format!("date_year <= ${idx}"));
            idx += 1;
        }
        if self.author_id.is_some() {
            conditions.push(format!(
                "bibkey IN (SELECT bibkey FROM bibitem_authors WHERE author_key = ${idx})"
            ));
            idx += 1;
        }
        if self.journal_id.is_some() {
            conditions.push(format!("journal_id = ${idx}"));
            idx += 1;
        }
        if self.epoch.is_some() {
            conditions.push(format!("epoch = ${idx}"));
            idx += 1;
        }
        (conditions, idx)
    }

    fn bind(&self, binder: &mut ParamBinder) -> Result<(), DataSourceError> {
        if let Some(entry_type) = self.entry_type {
            binder.add(entry_type)?;
        }
        if let Some(year_from) = self.year_from {
            binder.add(year_from)?;
        }
        if let Some(year_to) = self.year_to {
            binder.add(year_to)?;
        }
        if let Some(author_id) = self.author_id {
            binder.add(author_id)?;
        }
        if let Some(journal_id) = self.journal_id {
            binder.add(journal_id)?;
        }
        if let Some(epoch) = self.epoch {
            binder.add(epoch)?;
        }
        Ok(())
    }
}

/// Postgres-backed bibitem searcher.
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
        let ds: DataStore<BibItem> = DataStore::new(self.pool.clone());

        let text_search = TextSearch::on(
            ["title_unicode", "booktitle_unicode"],
            request.query.as_str(),
        );

        let filters = BibitemSearchFilters {
            entry_type: request.entry_type,
            year_from: request.year_from,
            year_to: request.year_to,
            author_id: request.author_id,
            journal_id: request.journal_id,
            epoch: request.epoch,
        };

        let fallback = SortOrder::by_desc("date_year").then_desc("id");

        let pagination = Pagination::with_offset(
            usize::try_from(request.offset).unwrap_or(0),
            usize::try_from(request.limit).unwrap_or(50),
        );

        let (results, total) = ds
            .find_by_similarity(&text_search, &filters, &pagination, &fallback)
            .await
            .map_err(HexforgeError::data_source)?;

        Ok(SearchResponse {
            results,
            total,
            limit: request.limit,
            offset: request.offset,
        })
    }
}
