//! Query filter for series.

use hexforge::{ParamBinder, QueryFilter};
use serde::Deserialize;

/// Query parameters for filtering series.
///
/// - `name` — case-insensitive substring match on `name_simplified`
#[derive(Debug, Default, Deserialize)]
pub struct SeriesQuery {
    pub name: Option<String>,
}

impl QueryFilter for SeriesQuery {
    fn build_conditions(&self, mut idx: usize) -> (Vec<String>, usize) {
        let mut conditions = vec![];

        if self.name.is_some() {
            conditions.push(format!("name_simplified ILIKE '%' || ${idx} || '%'"));
            idx += 1;
        }

        (conditions, idx)
    }

    fn bind(&self, binder: &mut ParamBinder) {
        if let Some(ref val) = self.name {
            binder.add(val.clone());
        }
    }
}
