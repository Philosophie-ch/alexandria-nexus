//! Postgres implementation of `CitationResolver`.

use std::collections::HashMap;

use hexforge::HexforgeError;
use hexforge::db_exports::{FromRow, PgPool, query_as};

use crate::domain::AuthorRole;
use crate::logic::latex_citations::CitationData;
use crate::process::latex_citations::CitationResolver;

pub struct PgCitationResolver<'a> {
    pool: &'a PgPool,
}

impl<'a> PgCitationResolver<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct CitationAuthorRow {
    bibkey: String,
    display_name: Option<String>,
    role: Option<AuthorRole>,
    date_year: Option<i16>,
}

impl CitationResolver for PgCitationResolver<'_> {
    async fn resolve_bibkeys(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, CitationData>, HexforgeError> {
        let rows: Vec<CitationAuthorRow> = query_as(
            "SELECT b.bibkey,
                COALESCE(a.family_name_unicode, a.mononym_unicode) AS display_name,
                ba.role,
                b.date_year
             FROM bibitems b
             LEFT JOIN bibitem_authors ba ON ba.bibkey = b.bibkey
             LEFT JOIN authors a ON a.author_key = ba.author_key
             WHERE b.bibkey = ANY($1)
             ORDER BY b.bibkey, ba.role, ba.position",
        )
        .bind(keys)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;

        type Entry = (Vec<String>, Vec<String>, Option<i16>);
        // Group rows by bibkey, collect authors then editors in position order.
        let mut by_key: HashMap<String, Entry> = HashMap::new();
        for r in rows {
            let entry = by_key
                .entry(r.bibkey)
                .or_insert_with(|| (vec![], vec![], r.date_year));
            if let Some(name) = r.display_name {
                if r.role == Some(AuthorRole::Author) {
                    entry.0.push(name);
                } else if r.role == Some(AuthorRole::Editor) {
                    entry.1.push(name);
                }
            }
        }

        // Bibitems with no junction rows won't appear above — fetch their years separately.
        let present_keys: std::collections::HashSet<&str> =
            by_key.keys().map(String::as_str).collect();
        let missing_keys: Vec<&String> = keys
            .iter()
            .filter(|k| !present_keys.contains(k.as_str()))
            .collect();
        if !missing_keys.is_empty() {
            #[derive(FromRow)]
            struct YearRow {
                bibkey: String,
                date_year: Option<i16>,
            }
            let year_rows: Vec<YearRow> =
                query_as("SELECT bibkey, date_year FROM bibitems WHERE bibkey = ANY($1)")
                    .bind(missing_keys.iter().map(|k| k.as_str()).collect::<Vec<_>>())
                    .fetch_all(self.pool)
                    .await
                    .map_err(HexforgeError::data_source)?;
            for r in year_rows {
                by_key.insert(r.bibkey, (vec![], vec![], r.date_year));
            }
        }

        Ok(by_key
            .into_iter()
            .map(|(bibkey, (authors, editors, year))| {
                let names = if !authors.is_empty() {
                    authors
                } else {
                    editors
                };
                let author = format_name_list(&names);
                (
                    bibkey,
                    CitationData {
                        author,
                        year,
                        year_suffix: None,
                    },
                )
            })
            .collect())
    }
}

fn format_name_list(names: &[String]) -> Option<String> {
    match names {
        [] => None,
        [a] => Some(a.clone()),
        [a, b] => Some(format!("{a} and {b}")),
        [a, b, c] => Some(format!("{a}, {b}, and {c}")),
        [a, ..] => Some(format!("{a} et al.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_list_is_none() {
        assert_eq!(format_name_list(&[]), None);
    }

    #[test]
    fn single_author() {
        assert_eq!(format_name_list(&names(&["Fraser"])), Some("Fraser".into()));
    }

    #[test]
    fn two_authors() {
        assert_eq!(
            format_name_list(&names(&["Fraser", "Haber"])),
            Some("Fraser and Haber".into()),
        );
    }

    #[test]
    fn three_authors() {
        assert_eq!(
            format_name_list(&names(&["Fraser", "Haber", "Müller"])),
            Some("Fraser, Haber, and Müller".into()),
        );
    }

    #[test]
    fn four_or_more_et_al() {
        assert_eq!(
            format_name_list(&names(&["Fraser", "Haber", "Müller", "Smith"])),
            Some("Fraser et al.".into()),
        );
    }
}
