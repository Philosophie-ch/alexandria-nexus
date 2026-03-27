//! Query filter for authors.

use hexforge::{ParamBinder, QueryFilter};
use serde::Deserialize;

/// Query parameters for filtering authors.
///
/// - `family_name` — case-insensitive substring match on `family_name_simplified`
/// - `search_term` — case-insensitive substring match on `family_name_simplified`
///   OR `given_name_simplified`
#[derive(Debug, Default, Deserialize)]
pub struct AuthorQuery {
    pub family_name: Option<String>,
    pub search_term: Option<String>,
}

impl QueryFilter for AuthorQuery {
    fn build_conditions(&self, mut idx: usize) -> (Vec<String>, usize) {
        let mut conditions = vec![];

        if self.family_name.is_some() {
            conditions.push(format!("family_name_simplified ILIKE '%' || ${idx} || '%'"));
            idx += 1;
        }

        if self.search_term.is_some() {
            conditions.push(format!(
                "(family_name_simplified ILIKE '%' || ${idx} || '%' OR given_name_simplified ILIKE '%' || ${idx} || '%')"
            ));
            idx += 1;
        }

        (conditions, idx)
    }

    fn bind(&self, binder: &mut ParamBinder) {
        if let Some(ref val) = self.family_name {
            binder.add(val.clone());
        }
        if let Some(ref val) = self.search_term {
            binder.add(val.clone());
        }
    }
}
