//! Postgres implementations of full CSV import/export store traits.
//!
//! These adapters implement the contracts defined in `crate::process::full_import`
//! using raw SQL against PostgreSQL.

use std::collections::{HashMap, HashSet};

use hexforge::db_exports::{FromRow, PgPool, query, query_as};
use hexforge::{HexforgeError, ValidationError};

use crate::adapters::field_parsing::{CsvHeaders, parse_row};
use crate::domain::junctions::{BibitemAuthorsRow, BibitemKeywordsRow};
use crate::domain::{BibItem, BibitemNotes, Epoch, LangId, PubState, RefType};
use crate::logic::full_import::{
    AuthorJunctionRow, AuthorLookupResult, AuthorNameKey, BibitemRefInsertRow, FieldError,
    KeywordJunctionRow, ParsedBibRow, RowError, RowParseResult, VariantInfo,
};
use crate::logic::transitive_closure::transitive_closure;
use crate::process::full_import::{
    AuthorLookup, AuthorNameFetcher, BibitemDeleter, BibitemFetcher, BibitemNotesFetcher,
    BibkeyLookup, BulkBibitemInsert, BulkJunctionInsert, EntityLookup, JunctionFetcher,
    KeywordLookup, KeywordNameFetcher, NamedEntity, ReverseNameMapFetcher, TransitiveDepsComputer,
};
use crate::process::import::{BibitemNotesData, BibitemNotesStore};

// =============================================================================
// Row types for sqlx (adapter-only)
// =============================================================================

#[derive(FromRow)]
struct AuthorRow {
    id: i64,
    author_key: String,
    family_name_latex: Option<String>,
    given_name_latex: Option<String>,
    mononym_latex: Option<String>,
    famous: bool,
    name_variants_latex: Option<Vec<String>>,
    name_variants_unicode: Option<Vec<String>>,
}

#[derive(FromRow)]
struct BibkeyRow {
    bibkey: String,
}

// =============================================================================
// PgFullImportStore — single struct implementing all full import traits
// =============================================================================

/// Postgres implementation of all full CSV import/export store traits.
///
/// Uses raw SQL to perform batch lookups, junction inserts, and entity deletions.
pub struct PgFullImportStore<'a> {
    pool: &'a PgPool,
}

impl<'a> PgFullImportStore<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

// =============================================================================
// AuthorLookup
// =============================================================================

impl AuthorLookup for PgFullImportStore<'_> {
    async fn batch_lookup_authors(&self) -> Result<AuthorLookupResult, HexforgeError> {
        let rows: Vec<AuthorRow> = query_as(
            "SELECT id, author_key, family_name_latex, given_name_latex, mononym_latex, famous, name_variants_latex, name_variants_unicode FROM authors",
        )
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;

        let mut id_map: HashMap<AuthorNameKey, Vec<i64>> = HashMap::new();
        let mut key_map: HashMap<AuthorNameKey, Vec<String>> = HashMap::new();
        let mut variant_map: HashMap<AuthorNameKey, VariantInfo> = HashMap::new();
        // Famous-only map for _person resolution: name string → author IDs.
        // Indexed by stripped mononym OR family name, famous authors only.
        let mut famous_person_map: HashMap<String, Vec<i64>> = HashMap::new();
        let mut famous_person_key_map: HashMap<String, Vec<String>> = HashMap::new();

        for row in &rows {
            // Primary name key
            let key = if let Some(mononym) = &row.mononym_latex {
                AuthorNameKey::Mononym(strip_outer_braces(mononym))
            } else if let Some(family) = &row.family_name_latex {
                AuthorNameKey::Named {
                    family_name: family.clone(),
                    given_name: row.given_name_latex.clone(),
                }
            } else {
                continue;
            };
            id_map.entry(key.clone()).or_default().push(row.id);
            key_map
                .entry(key.clone())
                .or_default()
                .push(row.author_key.clone());

            // Famous-only index for _person resolution
            if row.famous {
                let name = if let Some(mononym) = &row.mononym_latex {
                    strip_outer_braces(mononym)
                } else if let Some(family) = &row.family_name_latex {
                    family.clone()
                } else {
                    continue;
                };
                famous_person_map
                    .entry(name.clone())
                    .or_default()
                    .push(row.id);
                famous_person_key_map
                    .entry(name)
                    .or_default()
                    .push(row.author_key.clone());
            }

            // LaTeX name variants
            if let Some(variants) = &row.name_variants_latex {
                for variant in variants {
                    let keys = parse_variant_to_keys(variant);
                    for variant_key in keys {
                        id_map.entry(variant_key.clone()).or_default().push(row.id);
                        key_map
                            .entry(variant_key.clone())
                            .or_default()
                            .push(row.author_key.clone());
                        variant_map
                            .entry(variant_key)
                            .or_insert_with(|| VariantInfo {
                                variant_latex: Some(variant.clone()),
                                variant_unicode: None,
                            });
                    }
                }
            }

            // Unicode name variants
            if let Some(variants) = &row.name_variants_unicode {
                for variant in variants {
                    let keys = parse_variant_to_keys(variant);
                    for variant_key in keys {
                        id_map.entry(variant_key.clone()).or_default().push(row.id);
                        key_map
                            .entry(variant_key.clone())
                            .or_default()
                            .push(row.author_key.clone());
                        variant_map
                            .entry(variant_key)
                            .or_insert_with(|| VariantInfo {
                                variant_latex: None,
                                variant_unicode: Some(variant.clone()),
                            });
                    }
                }
            }
        }
        Ok(AuthorLookupResult {
            id_map,
            key_map,
            variant_map,
            famous_person_map,
            famous_person_key_map,
        })
    }
}

// =============================================================================
// EntityLookup
// =============================================================================

impl EntityLookup for PgFullImportStore<'_> {
    async fn batch_lookup_named_entity(
        &self,
        entity: NamedEntity,
    ) -> Result<HashMap<String, String>, HexforgeError> {
        let (table, key_col) = match entity {
            NamedEntity::Journals => ("journals", "journal_key"),
            NamedEntity::Publishers => ("publishers", "publisher_key"),
            NamedEntity::Institutions => ("institutions", "institution_key"),
            NamedEntity::Schools => ("schools", "school_key"),
            NamedEntity::Series => ("series", "series_key"),
        };
        #[derive(FromRow)]
        struct NameKeyRow {
            name_latex: String,
            entity_key: String,
        }
        let sql = format!("SELECT name_latex, {key_col} AS entity_key FROM {table}");
        let rows: Vec<NameKeyRow> = query_as(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(rows
            .into_iter()
            .map(|r| (r.name_latex, r.entity_key))
            .collect())
    }
}

// =============================================================================
// KeywordLookup
// =============================================================================

impl KeywordLookup for PgFullImportStore<'_> {
    async fn batch_lookup_keywords(&self) -> Result<HashMap<(String, i16), String>, HexforgeError> {
        #[derive(FromRow)]
        struct KeywordLookupRow {
            keyword_key: String,
            name: String,
            level: i16,
        }
        let rows: Vec<KeywordLookupRow> = query_as("SELECT keyword_key, name, level FROM keywords")
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;

        Ok(rows
            .into_iter()
            .map(|r| ((r.name, r.level), r.keyword_key))
            .collect())
    }
}

// =============================================================================
// BibkeyLookup
// =============================================================================

impl BibkeyLookup for PgFullImportStore<'_> {
    async fn fetch_all_bibkeys(&self) -> Result<HashSet<String>, HexforgeError> {
        let rows: Vec<BibkeyRow> = query_as("SELECT bibkey FROM bibitems")
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;

        Ok(rows.into_iter().map(|r| r.bibkey).collect())
    }
}

// =============================================================================
// BibitemDeleter
// =============================================================================

impl BibitemDeleter for PgFullImportStore<'_> {
    async fn delete_bibitems_by_bibkeys(&self, bibkeys: &[String]) -> Result<usize, HexforgeError> {
        let result = query("DELETE FROM bibitems WHERE bibkey = ANY($1)")
            .bind(bibkeys)
            .execute(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(usize::try_from(result.rows_affected()).unwrap_or(0))
    }
}

// =============================================================================
// BulkBibitemInsert
// =============================================================================

#[derive(FromRow)]
struct InsertedBibitem {
    id: i64,
    bibkey: String,
}

impl BulkBibitemInsert for PgFullImportStore<'_> {
    async fn bulk_insert_bibitems(
        &self,
        entities: &[BibItem],
    ) -> Result<Vec<(i64, String)>, HexforgeError> {
        if entities.is_empty() {
            return Ok(Vec::new());
        }

        let mut address: Vec<Option<String>> = Vec::new();
        let mut bibkey: Vec<String> = Vec::new();
        let mut booktitle_latex: Vec<Option<String>> = Vec::new();
        let mut booktitle_unicode: Vec<Option<String>> = Vec::new();
        let mut crossref: Vec<Option<String>> = Vec::new();
        let mut date_day: Vec<Option<i16>> = Vec::new();
        let mut date_is_no_date: Vec<bool> = Vec::new();
        let mut date_month: Vec<Option<i16>> = Vec::new();
        let mut date_year: Vec<Option<i16>> = Vec::new();
        let mut date_year_2_hyphen: Vec<Option<i16>> = Vec::new();
        let mut date_year_2_slash: Vec<Option<i16>> = Vec::new();
        let mut doi: Vec<Option<String>> = Vec::new();
        let mut edition: Vec<Option<String>> = Vec::new();
        let mut eid: Vec<Option<String>> = Vec::new();
        let mut entry_type: Vec<String> = Vec::new();
        let mut epoch: Vec<Option<String>> = Vec::new();
        let mut eprint: Vec<Option<String>> = Vec::new();
        let mut extra_note_latex: Vec<Option<String>> = Vec::new();
        let mut extra_note_unicode: Vec<Option<String>> = Vec::new();
        let mut fulltext_path: Vec<Option<String>> = Vec::new();
        let mut has_fulltext: Vec<bool> = Vec::new();
        let mut institution_key: Vec<Option<String>> = Vec::new();
        let mut is_translation: Vec<bool> = Vec::new();
        let mut issuetitle_latex: Vec<Option<String>> = Vec::new();
        let mut issuetitle_unicode: Vec<Option<String>> = Vec::new();
        let mut journal_key: Vec<Option<String>> = Vec::new();
        let mut langid: Vec<Option<String>> = Vec::new();
        let mut note_latex: Vec<Option<String>> = Vec::new();
        let mut note_unicode: Vec<Option<String>> = Vec::new();
        let mut number: Vec<Option<String>> = Vec::new();
        let mut options: Vec<Option<String>> = Vec::new();
        let mut pages: Vec<Option<String>> = Vec::new();
        let mut person_key: Vec<Option<String>> = Vec::new();
        let mut publisher_key: Vec<Option<String>> = Vec::new();
        let mut pubstate: Vec<Option<String>> = Vec::new();
        let mut school_key: Vec<Option<String>> = Vec::new();
        let mut series_key: Vec<Option<String>> = Vec::new();
        let mut shorthand: Vec<Option<String>> = Vec::new();
        let mut title_latex: Vec<String> = Vec::new();
        let mut title_unicode: Vec<Option<String>> = Vec::new();
        let mut type_field: Vec<Option<String>> = Vec::new();
        let mut url: Vec<Option<String>> = Vec::new();
        let mut urn: Vec<Option<String>> = Vec::new();
        let mut volume: Vec<Option<String>> = Vec::new();

        for e in entities {
            address.push(e.address.clone());
            bibkey.push(e.bibkey.clone());
            booktitle_latex.push(e.booktitle_latex.clone());
            booktitle_unicode.push(e.booktitle_unicode.clone());
            crossref.push(e.crossref.clone());
            date_day.push(e.date_day);
            date_is_no_date.push(e.date_is_no_date);
            date_month.push(e.date_month);
            date_year.push(e.date_year);
            date_year_2_hyphen.push(e.date_year_2_hyphen);
            date_year_2_slash.push(e.date_year_2_slash);
            doi.push(e.doi.clone());
            edition.push(e.edition.clone());
            eid.push(e.eid.clone());
            entry_type.push(e.entry_type.to_string());
            epoch.push(e.epoch.as_ref().map(Epoch::to_string));
            eprint.push(e.eprint.clone());
            extra_note_latex.push(e.extra_note_latex.clone());
            extra_note_unicode.push(e.extra_note_unicode.clone());
            fulltext_path.push(e.fulltext_path.clone());
            has_fulltext.push(e.has_fulltext);
            institution_key.push(e.institution_key.clone());
            is_translation.push(e.is_translation);
            issuetitle_latex.push(e.issuetitle_latex.clone());
            issuetitle_unicode.push(e.issuetitle_unicode.clone());
            journal_key.push(e.journal_key.clone());
            langid.push(e.langid.as_ref().map(LangId::to_string));
            note_latex.push(e.note_latex.clone());
            note_unicode.push(e.note_unicode.clone());
            number.push(e.number.clone());
            options.push(e.options.clone());
            pages.push(e.pages.clone());
            person_key.push(e.person_key.clone());
            publisher_key.push(e.publisher_key.clone());
            pubstate.push(e.pubstate.as_ref().map(PubState::to_string));
            school_key.push(e.school_key.clone());
            series_key.push(e.series_key.clone());
            shorthand.push(e.shorthand.clone());
            title_latex.push(e.title_latex.clone());
            title_unicode.push(e.title_unicode.clone());
            type_field.push(e.type_field.clone());
            url.push(e.url.clone());
            urn.push(e.urn.clone());
            volume.push(e.volume.clone());
        }

        let rows: Vec<InsertedBibitem> = query_as(
            "INSERT INTO bibitems (
               address, bibkey, booktitle_latex, booktitle_unicode,
               crossref, date_day, date_is_no_date, date_month, date_year,
               date_year_2_hyphen, date_year_2_slash, doi, edition, eid, entry_type,
               epoch, eprint, extra_note_latex, extra_note_unicode, fulltext_path,
               has_fulltext, institution_key, is_translation, issuetitle_latex, issuetitle_unicode,
               journal_key, langid, note_latex, note_unicode, number, options, pages,
               person_key, publisher_key, pubstate, school_key, series_key, shorthand,
               title_latex, title_unicode, type_field, url, urn, volume
             )
             SELECT
               address, bibkey, booktitle_latex, booktitle_unicode,
               crossref, date_day, date_is_no_date, date_month, date_year,
               date_year_2_hyphen, date_year_2_slash, doi, edition, eid, entry_type::entry_type,
               epoch::epoch, eprint, extra_note_latex, extra_note_unicode, fulltext_path,
               has_fulltext, institution_key, is_translation, issuetitle_latex, issuetitle_unicode,
               journal_key, langid::langid, note_latex, note_unicode, number, options, pages,
               person_key, publisher_key, pubstate::pubstate, school_key, series_key, shorthand,
               title_latex, title_unicode, type_field, url, urn, volume
             FROM unnest(
               $1::text[], $2::text[], $3::text[], $4::text[],
               $5::text[], $6::int2[], $7::bool[], $8::int2[], $9::int2[],
               $10::int2[], $11::int2[], $12::text[], $13::text[], $14::text[], $15::text[],
               $16::text[], $17::text[], $18::text[], $19::text[], $20::text[],
               $21::bool[], $22::text[], $23::bool[], $24::text[], $25::text[],
               $26::text[], $27::text[], $28::text[], $29::text[], $30::text[], $31::text[], $32::text[],
               $33::text[], $34::text[], $35::text[], $36::text[], $37::text[], $38::text[],
               $39::text[], $40::text[], $41::text[], $42::text[], $43::text[], $44::text[]
             ) AS t(
               address, bibkey, booktitle_latex, booktitle_unicode,
               crossref, date_day, date_is_no_date, date_month, date_year,
               date_year_2_hyphen, date_year_2_slash, doi, edition, eid, entry_type,
               epoch, eprint, extra_note_latex, extra_note_unicode, fulltext_path,
               has_fulltext, institution_key, is_translation, issuetitle_latex, issuetitle_unicode,
               journal_key, langid, note_latex, note_unicode, number, options, pages,
               person_key, publisher_key, pubstate, school_key, series_key, shorthand,
               title_latex, title_unicode, type_field, url, urn, volume
             )
             RETURNING id, bibkey",
        )
        .bind(address)
        .bind(bibkey)
        .bind(booktitle_latex)
        .bind(booktitle_unicode)
        .bind(crossref)
        .bind(date_day)
        .bind(date_is_no_date)
        .bind(date_month)
        .bind(date_year)
        .bind(date_year_2_hyphen)
        .bind(date_year_2_slash)
        .bind(doi)
        .bind(edition)
        .bind(eid)
        .bind(entry_type)
        .bind(epoch)
        .bind(eprint)
        .bind(extra_note_latex)
        .bind(extra_note_unicode)
        .bind(fulltext_path)
        .bind(has_fulltext)
        .bind(institution_key)
        .bind(is_translation)
        .bind(issuetitle_latex)
        .bind(issuetitle_unicode)
        .bind(journal_key)
        .bind(langid)
        .bind(note_latex)
        .bind(note_unicode)
        .bind(number)
        .bind(options)
        .bind(pages)
        .bind(person_key)
        .bind(publisher_key)
        .bind(pubstate)
        .bind(school_key)
        .bind(series_key)
        .bind(shorthand)
        .bind(title_latex)
        .bind(title_unicode)
        .bind(type_field)
        .bind(url)
        .bind(urn)
        .bind(volume)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;

        Ok(rows.into_iter().map(|r| (r.id, r.bibkey)).collect())
    }
}

// =============================================================================
// BulkJunctionInsert
// =============================================================================

impl BulkJunctionInsert for PgFullImportStore<'_> {
    async fn bulk_insert_author_junctions(
        &self,
        rows: &[AuthorJunctionRow],
    ) -> Result<(), HexforgeError> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut bibkeys: Vec<String> = Vec::new();
        let mut author_keys: Vec<String> = Vec::new();
        let mut roles: Vec<String> = Vec::new();
        let mut positions: Vec<i16> = Vec::new();
        let mut variant_latexes: Vec<Option<String>> = Vec::new();
        let mut variant_unicodes: Vec<Option<String>> = Vec::new();
        for r in rows {
            bibkeys.push(r.bibkey.clone());
            author_keys.push(r.author_key.clone());
            roles.push(r.role.to_string());
            positions.push(r.position);
            variant_latexes.push(r.variant_latex.clone());
            variant_unicodes.push(r.variant_unicode.clone());
        }
        query(
            "INSERT INTO bibitem_authors (bibkey, author_key, role, position, name_variant_latex, name_variant_unicode)
             SELECT DISTINCT ON (bibkey, author_key, role)
               bibkey, author_key, role::author_role, position, name_variant_latex, name_variant_unicode
             FROM unnest($1::text[], $2::text[], $3::text[], $4::int2[], $5::text[], $6::text[])
               AS t(bibkey, author_key, role, position, name_variant_latex, name_variant_unicode)
             ORDER BY bibkey, author_key, role, position
             ON CONFLICT (bibkey, author_key, role) DO UPDATE SET
               position = EXCLUDED.position,
               name_variant_latex = EXCLUDED.name_variant_latex,
               name_variant_unicode = EXCLUDED.name_variant_unicode",
        )
        .bind(bibkeys)
        .bind(author_keys)
        .bind(roles)
        .bind(positions)
        .bind(variant_latexes)
        .bind(variant_unicodes)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;
        Ok(())
    }

    async fn bulk_insert_keyword_junctions(
        &self,
        rows: &[KeywordJunctionRow],
    ) -> Result<(), HexforgeError> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut bibkeys: Vec<String> = Vec::new();
        let mut keyword_keys: Vec<String> = Vec::new();
        let mut keyword_levels: Vec<i16> = Vec::new();
        for r in rows {
            bibkeys.push(r.bibkey.clone());
            keyword_keys.push(r.keyword_key.clone());
            keyword_levels.push(r.keyword_level);
        }
        query(
            "INSERT INTO bibitem_keywords (bibkey, keyword_key, keyword_level)
             SELECT bibkey, keyword_key, keyword_level
             FROM unnest($1::text[], $2::text[], $3::int2[]) AS t(bibkey, keyword_key, keyword_level)
             ON CONFLICT (bibkey, keyword_key) DO NOTHING",
        )
        .bind(bibkeys)
        .bind(keyword_keys)
        .bind(keyword_levels)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;
        Ok(())
    }

    async fn bulk_insert_bibitem_refs(
        &self,
        rows: &[BibitemRefInsertRow],
    ) -> Result<(), HexforgeError> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut source_bibkeys: Vec<String> = Vec::new();
        let mut target_bibkeys: Vec<String> = Vec::new();
        let mut ref_types: Vec<String> = Vec::new();
        for r in rows {
            source_bibkeys.push(r.source_bibkey.clone());
            target_bibkeys.push(r.target_bibkey.clone());
            ref_types.push(r.ref_type.to_string());
        }
        query(
            "INSERT INTO bibitem_refs (source_key, target_key, ref_type)
             SELECT r.source_bibkey, r.target_bibkey, r.ref_type::ref_type
             FROM unnest($1::text[], $2::text[], $3::text[]) AS r(source_bibkey, target_bibkey, ref_type)
             WHERE EXISTS (SELECT 1 FROM bibitems WHERE bibkey = r.source_bibkey)
               AND EXISTS (SELECT 1 FROM bibitems WHERE bibkey = r.target_bibkey)
             ON CONFLICT (source_key, target_key, ref_type) DO NOTHING",
        )
        .bind(source_bibkeys)
        .bind(target_bibkeys)
        .bind(ref_types)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;
        Ok(())
    }
}

// =============================================================================
// BibitemFetcher
// =============================================================================

impl BibitemFetcher for PgFullImportStore<'_> {
    async fn fetch_all_bibitems(&self) -> Result<Vec<BibItem>, HexforgeError> {
        query_as("SELECT * FROM bibitems ORDER BY bibkey")
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)
    }
}

// =============================================================================
// AuthorNameFetcher
// =============================================================================

impl AuthorNameFetcher for PgFullImportStore<'_> {
    async fn fetch_author_names(&self) -> Result<HashMap<String, String>, HexforgeError> {
        #[derive(FromRow)]
        struct AuthorNameRow {
            author_key: String,
            family_name_latex: Option<String>,
            given_name_latex: Option<String>,
            mononym_latex: Option<String>,
        }
        let rows: Vec<AuthorNameRow> = query_as(
            "SELECT author_key, family_name_latex, given_name_latex, mononym_latex FROM authors",
        )
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;

        Ok(rows
            .into_iter()
            .map(|a| {
                let name = if let Some(m) = a.mononym_latex {
                    m
                } else {
                    match (a.family_name_latex, a.given_name_latex) {
                        (Some(f), Some(g)) => format!("{f}, {g}"),
                        (Some(f), None) => f,
                        _ => String::new(),
                    }
                };
                (a.author_key, name)
            })
            .collect())
    }
}

// =============================================================================
// ReverseNameMapFetcher
// =============================================================================

impl ReverseNameMapFetcher for PgFullImportStore<'_> {
    async fn fetch_entity_name_map(
        &self,
        entity: NamedEntity,
    ) -> Result<HashMap<String, String>, HexforgeError> {
        let (table, key_col) = match entity {
            NamedEntity::Journals => ("journals", "journal_key"),
            NamedEntity::Publishers => ("publishers", "publisher_key"),
            NamedEntity::Institutions => ("institutions", "institution_key"),
            NamedEntity::Schools => ("schools", "school_key"),
            NamedEntity::Series => ("series", "series_key"),
        };
        #[derive(FromRow)]
        struct KeyNameRow {
            entity_key: String,
            name_latex: String,
        }
        let sql = format!("SELECT {key_col} AS entity_key, name_latex FROM {table}");
        let rows: Vec<KeyNameRow> = query_as(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(rows
            .into_iter()
            .map(|r| (r.entity_key, r.name_latex))
            .collect())
    }
}

// =============================================================================
// KeywordNameFetcher
// =============================================================================

impl KeywordNameFetcher for PgFullImportStore<'_> {
    async fn fetch_keyword_names(&self) -> Result<HashMap<String, (String, i16)>, HexforgeError> {
        #[derive(FromRow)]
        struct KeywordKeyRow {
            keyword_key: String,
            name: String,
            level: i16,
        }
        let rows: Vec<KeywordKeyRow> = query_as("SELECT keyword_key, name, level FROM keywords")
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(rows
            .into_iter()
            .map(|k| (k.keyword_key, (k.name, k.level)))
            .collect())
    }
}

// =============================================================================
// JunctionFetcher
// =============================================================================

impl TransitiveDepsComputer for PgFullImportStore<'_> {
    async fn compute_transitive_deps(&self) -> Result<(usize, usize), HexforgeError> {
        query("TRUNCATE bibitem_further_refs, bibitem_depends_on")
            .execute(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;

        let further = compute_and_insert_closure(self.pool, RefType::FurtherRef).await?;
        let depends = compute_and_insert_closure(self.pool, RefType::DependsOn).await?;
        Ok((further, depends))
    }
}

async fn compute_and_insert_closure(
    pool: &hexforge::db_exports::PgPool,
    ref_type: RefType,
) -> Result<usize, HexforgeError> {
    let table = match ref_type {
        RefType::FurtherRef => "bibitem_further_refs",
        RefType::DependsOn => "bibitem_depends_on",
    };

    #[derive(FromRow)]
    struct RefEdge {
        source_key: String,
        target_key: String,
    }

    let raw: Vec<RefEdge> =
        query_as("SELECT source_key, target_key FROM bibitem_refs WHERE ref_type = $1::ref_type")
            .bind(ref_type.to_string())
            .fetch_all(pool)
            .await
            .map_err(HexforgeError::data_source)?;

    if raw.is_empty() {
        return Ok(0);
    }

    let edges: Vec<(String, String)> = raw
        .into_iter()
        .map(|e| (e.source_key, e.target_key))
        .collect();
    let closure = transitive_closure(&edges);
    if closure.is_empty() {
        return Ok(0);
    }

    let source_keys: Vec<String> = closure.iter().map(|(s, _)| s.clone()).collect();
    let dep_keys: Vec<String> = closure.iter().map(|(_, d)| d.clone()).collect();
    let sql = format!(
        "INSERT INTO {table} (source_key, dep_key) \
         SELECT * FROM UNNEST($1::text[], $2::text[]) ON CONFLICT DO NOTHING"
    );
    let rows = query(&sql)
        .bind(&source_keys[..])
        .bind(&dep_keys[..])
        .execute(pool)
        .await
        .map_err(HexforgeError::data_source)?
        .rows_affected();
    Ok(usize::try_from(rows).unwrap_or(usize::MAX))
}

impl JunctionFetcher for PgFullImportStore<'_> {
    async fn fetch_bibitem_authors_batch(
        &self,
        ids: &[i64],
    ) -> Result<Vec<BibitemAuthorsRow>, HexforgeError> {
        query_as::<_, BibitemAuthorsRow>(
            "SELECT ba.bibkey, ba.author_key, ba.role, ba.position, ba.name_variant_latex, ba.name_variant_unicode \
             FROM bibitem_authors ba JOIN bibitems b ON b.bibkey = ba.bibkey WHERE b.id = ANY($1) ORDER BY ba.bibkey, ba.role, ba.position",
        )
        .bind(ids)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)
    }

    async fn fetch_bibitem_keywords_batch(
        &self,
        ids: &[i64],
    ) -> Result<Vec<BibitemKeywordsRow>, HexforgeError> {
        query_as::<_, BibitemKeywordsRow>(
            "SELECT bk.bibkey, bk.keyword_key, bk.keyword_level \
             FROM bibitem_keywords bk JOIN bibitems b ON b.bibkey = bk.bibkey WHERE b.id = ANY($1) ORDER BY bk.bibkey",
        )
        .bind(ids)
        .fetch_all(self.pool)
        .await
        .map_err(HexforgeError::data_source)
    }
}

impl BibitemNotesStore for PgFullImportStore<'_> {
    async fn upsert_bibitem_notes(
        &self,
        bibkey: &str,
        notes: &BibitemNotesData<'_>,
    ) -> Result<(), HexforgeError> {
        query(
            "INSERT INTO bibitem_notes \
             (bibkey, note_perso, note_stock, note_missing, change_request, dltc_copyediting_note, todo_general) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (bibkey) DO UPDATE SET \
             note_perso = EXCLUDED.note_perso, \
             note_stock = EXCLUDED.note_stock, \
             note_missing = EXCLUDED.note_missing, \
             change_request = EXCLUDED.change_request, \
             dltc_copyediting_note = EXCLUDED.dltc_copyediting_note, \
             todo_general = EXCLUDED.todo_general, \
             updated_at = NOW()",
        )
        .bind(bibkey)
        .bind(notes.note_perso)
        .bind(notes.note_stock)
        .bind(notes.note_missing)
        .bind(notes.change_request)
        .bind(notes.dltc_copyediting_note)
        .bind(notes.todo_general)
        .execute(self.pool)
        .await
        .map_err(HexforgeError::data_source)?;
        Ok(())
    }
}

impl BibitemNotesFetcher for PgFullImportStore<'_> {
    async fn fetch_all_bibitem_notes(
        &self,
    ) -> Result<HashMap<String, BibitemNotes>, HexforgeError> {
        let rows = query_as::<_, BibitemNotes>("SELECT * FROM bibitem_notes")
            .fetch_all(self.pool)
            .await
            .map_err(HexforgeError::data_source)?;
        Ok(rows.into_iter().map(|n| (n.bibkey.clone(), n)).collect())
    }
}

// =============================================================================
// Name variant → AuthorNameKey helper (uses the adapter-layer author parser)
// =============================================================================

fn strip_outer_braces(s: &str) -> String {
    if s.starts_with('{') && s.ends_with('}') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

pub fn parse_variant_to_keys(variant: &str) -> Vec<AuthorNameKey> {
    if let Ok(parsed) = crate::adapters::field_parsing::author::parse_authors(variant) {
        parsed.iter().map(AuthorNameKey::from_parsed).collect()
    } else {
        vec![AuthorNameKey::Mononym(variant.to_string())]
    }
}

// =============================================================================
// CSV parse function (wire format — belongs at the adapter boundary)
// =============================================================================

pub fn parse_all_rows(data: &[u8]) -> Result<(Vec<ParsedBibRow>, Vec<RowError>), HexforgeError> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(data);

    let headers = rdr
        .headers()
        .map_err(|e| {
            HexforgeError::Validation(ValidationError::custom(format!("invalid CSV headers: {e}")))
        })?
        .clone();

    let header_fields: Vec<&str> = headers.iter().collect();
    let csv_headers = CsvHeaders::from_headers(&header_fields);
    let mut parsed_rows = Vec::new();
    let mut row_errors = Vec::new();

    for (idx, result) in rdr.records().enumerate() {
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                row_errors.push(RowError {
                    row: idx + 2,
                    bibkey: None,
                    errors: vec![FieldError {
                        field: "_csv".to_string(),
                        error: format!("malformed CSV row: {e}"),
                    }],
                });
                continue;
            }
        };

        let fields: Vec<&str> = record.iter().collect();
        match parse_row(&csv_headers, &fields) {
            RowParseResult::Ok(row) => parsed_rows.push(*row),
            RowParseResult::Err { bibkey, errors } => {
                row_errors.push(RowError {
                    row: idx + 2,
                    bibkey,
                    errors,
                });
            }
        }
    }

    Ok((parsed_rows, row_errors))
}

#[cfg(test)]
mod tests {
    use super::strip_outer_braces;

    #[test]
    fn strips_outer_braces() {
        assert_eq!(strip_outer_braces("{Aristotle}"), "Aristotle");
        assert_eq!(strip_outer_braces("{Plato}"), "Plato");
    }

    #[test]
    fn no_braces_unchanged() {
        assert_eq!(strip_outer_braces("Brandeslein"), "Brandeslein");
    }

    #[test]
    fn only_outer_braces_stripped() {
        assert_eq!(
            strip_outer_braces("{Biblioth{\\`e}que}"),
            "Biblioth{\\`e}que"
        );
    }

    #[test]
    fn empty_string_unchanged() {
        assert_eq!(strip_outer_braces(""), "");
    }
}
