//! Query filter for keywords.

use hexforge::{ParamBinder, QueryFilter};
use serde::Deserialize;

/// Query parameters for filtering keywords.
///
/// - `level` — exact match on `level` (1-3)
/// - `name` — case-insensitive substring match on `name`
#[derive(Debug, Default, Deserialize)]
pub struct KeywordQuery {
    pub level: Option<i16>,
    pub name: Option<String>,
}

impl QueryFilter for KeywordQuery {
    fn build_conditions(&self, mut idx: usize) -> (Vec<String>, usize) {
        let mut conditions = vec![];

        if self.level.is_some() {
            conditions.push(format!("level = ${idx}"));
            idx += 1;
        }

        if self.name.is_some() {
            conditions.push(format!("name ILIKE '%' || ${idx} || '%'"));
            idx += 1;
        }

        (conditions, idx)
    }

    fn bind(&self, binder: &mut ParamBinder) {
        if let Some(val) = self.level {
            binder.add(val);
        }
        if let Some(ref val) = self.name {
            binder.add(val.clone());
        }
    }
}
