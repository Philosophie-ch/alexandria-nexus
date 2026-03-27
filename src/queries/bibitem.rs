//! Query filter for bibliography items — the most complex filter.

use hexforge::{ParamBinder, QueryFilter};
use serde::Deserialize;

/// Query parameters for filtering bibliography items.
///
/// - `entry_type` — exact match on `entry_type` (cast to enum text)
/// - `year_from` / `year_to` — range filter on `date_year`
/// - `author_id` — EXISTS subquery on `bibitem_authors` junction table
/// - `journal_id` — exact match on `journal_id`
/// - `epoch` — exact match on `epoch` (cast to enum text)
/// - `search_term` — case-insensitive substring match on `title_simplified`
#[derive(Debug, Default, Deserialize)]
pub struct BibItemQuery {
    pub entry_type: Option<String>,
    pub year_from: Option<i16>,
    pub year_to: Option<i16>,
    pub author_id: Option<i64>,
    pub journal_id: Option<i64>,
    pub epoch: Option<String>,
    pub search_term: Option<String>,
}

impl QueryFilter for BibItemQuery {
    fn build_conditions(&self, mut idx: usize) -> (Vec<String>, usize) {
        let mut conditions = vec![];

        if self.entry_type.is_some() {
            conditions.push(format!("entry_type = ${idx}::entry_type"));
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
                "id IN (SELECT bibitem_id FROM bibitem_authors WHERE author_id = ${idx})"
            ));
            idx += 1;
        }

        if self.journal_id.is_some() {
            conditions.push(format!("journal_id = ${idx}"));
            idx += 1;
        }

        if self.epoch.is_some() {
            conditions.push(format!("epoch = ${idx}::epoch"));
            idx += 1;
        }

        if self.search_term.is_some() {
            conditions.push(format!("title_simplified ILIKE '%' || ${idx} || '%'"));
            idx += 1;
        }

        (conditions, idx)
    }

    fn bind(&self, binder: &mut ParamBinder) {
        if let Some(ref val) = self.entry_type {
            binder.add(val.clone());
        }
        if let Some(val) = self.year_from {
            binder.add(val);
        }
        if let Some(val) = self.year_to {
            binder.add(val);
        }
        if let Some(val) = self.author_id {
            binder.add(val);
        }
        if let Some(val) = self.journal_id {
            binder.add(val);
        }
        if let Some(ref val) = self.epoch {
            binder.add(val.clone());
        }
        if let Some(ref val) = self.search_term {
            binder.add(val.clone());
        }
    }
}
