//! Full CSV import — pure types, helpers, and CSV parsing for human-readable CSVs.
//!
//! This module contains ZERO async, ZERO database access, ZERO AppState.
//! All I/O orchestration lives in `crate::process::full_import`.

use std::collections::{HashMap, HashSet};

use hexforge::{HexforgeError, ValidationError};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::{AuthorRole, CreateBibItem, RefType};
use crate::logic::csv_parsing::types::{
    DateRangeSeparator, FieldError, ParsedAuthor, ParsedBibRow, ParsedDate, RowParseResult,
};
use crate::logic::csv_parsing::{CsvHeaders, parse_csv_row};

// =============================================================================
// Response types
// =============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct ValidationReport {
    pub total_rows: usize,
    pub valid_rows: usize,
    pub errors: Vec<RowError>,
    pub duplicate_bibkeys: Vec<DuplicateBibkey>,
    pub missing_authors: Vec<String>,
    pub ambiguous_authors: Vec<AmbiguousAuthor>,
    pub missing_journals: Vec<String>,
    pub missing_publishers: Vec<String>,
    pub missing_institutions: Vec<String>,
    pub missing_schools: Vec<String>,
    pub missing_series: Vec<String>,
    pub missing_keywords: MissingKeywords,
    pub missing_crossrefs: Vec<String>,
    pub missing_further_refs: Vec<String>,
    pub missing_depends_on: Vec<String>,
    pub stale_bibitems: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RowError {
    pub row: usize,
    pub bibkey: Option<String>,
    pub errors: Vec<FieldError>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AmbiguousAuthor {
    pub name: String,
    pub matching_ids: Vec<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DuplicateBibkey {
    pub bibkey: String,
    pub rows: Vec<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MissingKeywords {
    pub level_1: Vec<String>,
    pub level_2: Vec<String>,
    pub level_3: Vec<String>,
}

impl ValidationReport {
    pub fn has_issues(&self) -> bool {
        !self.errors.is_empty()
            || !self.duplicate_bibkeys.is_empty()
            || !self.missing_authors.is_empty()
            || !self.ambiguous_authors.is_empty()
            || !self.missing_journals.is_empty()
            || !self.missing_publishers.is_empty()
            || !self.missing_institutions.is_empty()
            || !self.missing_schools.is_empty()
            || !self.missing_series.is_empty()
            || !self.missing_keywords.level_1.is_empty()
            || !self.missing_keywords.level_2.is_empty()
            || !self.missing_keywords.level_3.is_empty()
            || !self.missing_crossrefs.is_empty()
            || !self.missing_further_refs.is_empty()
            || !self.missing_depends_on.is_empty()
    }
}

// =============================================================================
// Entity import report types
// =============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct EntityImportReport {
    pub created_institutions: usize,
    pub created_schools: usize,
    pub created_series: usize,
    pub created_keywords: usize,
    pub errors: Vec<EntityImportError>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EntityImportError {
    pub entity_type: String,
    pub name: String,
    pub error: String,
}

// =============================================================================
// LaTeX → Unicode conversion report types
// =============================================================================

/// Per-item outcome of a LaTeX → Unicode conversion.
#[derive(Debug)]
pub enum ConvertOutcome {
    Ok(String),
    Err { original: String, message: String },
}

/// Aggregate report returned by `POST /api/v1/admin/convert-latex-columns`.
#[derive(Debug, Serialize, ToSchema)]
pub struct LatexConvertReport {
    pub columns: Vec<ColumnConvertResult>,
    pub total_updated: usize,
    pub errors: Vec<LatexConvertError>,
}

/// Stats for one converted column.
#[derive(Debug, Serialize, ToSchema)]
pub struct ColumnConvertResult {
    pub table: &'static str,
    pub column: &'static str,
    pub updated: usize,
}

/// A row whose LaTeX value could not be converted — mirrors `EntityImportError`.
#[derive(Debug, Serialize, ToSchema)]
pub struct LatexConvertError {
    pub table: &'static str,
    pub column: &'static str,
    pub id: i64,
    pub error: String,
}

// =============================================================================
// Full import report types
// =============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct FullImportReport {
    pub imported: usize,
    pub updated: usize,
    pub deleted: usize,
    pub failed: usize,
    pub errors: Vec<RowError>,
}

pub enum FullImportResult {
    Success(FullImportReport),
    /// Validation failed -- return the full report so the caller sees everything at once.
    ValidationFailed(Box<ValidationReport>),
}

// =============================================================================
// Author lookup key
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AuthorNameKey {
    Named {
        family_name: String,
        given_name: Option<String>,
    },
    Mononym(String),
}

impl AuthorNameKey {
    pub fn from_parsed(author: &ParsedAuthor) -> Self {
        match author {
            ParsedAuthor::Named {
                family_name,
                given_name,
            } => AuthorNameKey::Named {
                family_name: family_name.clone(),
                given_name: given_name.clone(),
            },
            ParsedAuthor::Mononym(m) => AuthorNameKey::Mononym(m.clone()),
        }
    }
}

pub fn format_author_key(key: &AuthorNameKey) -> String {
    match key {
        AuthorNameKey::Mononym(m) => m.clone(),
        AuthorNameKey::Named {
            family_name,
            given_name,
        } => match given_name {
            Some(g) => format!("{family_name}, {g}"),
            None => family_name.clone(),
        },
    }
}

// =============================================================================
// Variant info
// =============================================================================

/// Variant info for an author matched via name variant.
#[derive(Clone)]
pub struct VariantInfo {
    pub variant_latex: Option<String>,
    pub variant_unicode: Option<String>,
}

// =============================================================================
// Author lookup result
// =============================================================================

pub struct AuthorLookupResult {
    /// AuthorNameKey -> list of matching author IDs (for validation: detect ambiguous)
    pub id_map: HashMap<AuthorNameKey, Vec<i64>>,
    /// AuthorNameKey -> variant info (only for keys matched via a name variant)
    pub variant_map: HashMap<AuthorNameKey, VariantInfo>,
}

// =============================================================================
// Lookup maps (bundles all lookup data to avoid too-many-args)
// =============================================================================

/// All DB lookup maps, built once per import operation.
pub struct LookupMaps {
    pub authors: AuthorLookupResult,
    pub journals: HashMap<String, i64>,
    pub publishers: HashMap<String, i64>,
    pub institutions: HashMap<String, i64>,
    pub schools: HashMap<String, i64>,
    pub series: HashMap<String, i64>,
    pub keywords: HashMap<(String, i16), i64>,
    pub bibkeys: HashSet<String>,
}

// =============================================================================
// Resolution context (bundles all lookup maps to avoid too-many-args)
// =============================================================================

/// Resolution context for bibitem upsert (single-match authors only).
pub struct ResolutionCtx {
    pub author_resolve: HashMap<AuthorNameKey, i64>,
    /// For keys that matched via a name variant, stores the variant strings.
    pub author_variants: HashMap<AuthorNameKey, VariantInfo>,
    pub journal_map: HashMap<String, i64>,
    pub publisher_map: HashMap<String, i64>,
    pub institution_map: HashMap<String, i64>,
    pub school_map: HashMap<String, i64>,
    pub series_map: HashMap<String, i64>,
    pub keyword_map: HashMap<(String, i16), i64>,
    pub existing_bibkeys: HashSet<String>,
}

impl ResolutionCtx {
    /// Build a ResolutionCtx from LookupMaps (consumes the maps).
    pub fn from_lookup_maps(maps: LookupMaps) -> Self {
        let author_variants = maps.authors.variant_map;
        ResolutionCtx {
            author_resolve: maps
                .authors
                .id_map
                .into_iter()
                .filter_map(|(k, ids)| {
                    if ids.len() == 1 {
                        Some((k, ids[0]))
                    } else {
                        None
                    }
                })
                .collect(),
            author_variants,
            journal_map: maps.journals,
            publisher_map: maps.publishers,
            institution_map: maps.institutions,
            school_map: maps.schools,
            series_map: maps.series,
            keyword_map: maps.keywords,
            existing_bibkeys: maps.bibkeys,
        }
    }
}

// =============================================================================
// Named entity kind
// =============================================================================

#[derive(Clone, Copy)]
pub enum NamedEntityKind {
    Institution,
    School,
    Series,
}

impl NamedEntityKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Institution => "institutions",
            Self::School => "schools",
            Self::Series => "series",
        }
    }
}

// =============================================================================
// Collected names helper
// =============================================================================

#[derive(Default)]
pub struct CollectedNames {
    pub authors: HashSet<AuthorNameKey>,
    pub journal_names: HashSet<String>,
    pub publisher_names: HashSet<String>,
    pub institution_names: HashSet<String>,
    pub school_names: HashSet<String>,
    pub series_names: HashSet<String>,
    pub keywords_l1: HashSet<String>,
    pub keywords_l2: HashSet<String>,
    pub keywords_l3: HashSet<String>,
    pub crossref_bibkeys: HashSet<String>,
    pub further_ref_bibkeys: HashSet<String>,
    pub depends_on_bibkeys: HashSet<String>,
}

impl CollectedNames {
    pub fn collect_from_row(&mut self, row: &ParsedBibRow) {
        for a in row
            .authors
            .iter()
            .chain(&row.editors)
            .chain(&row.guesteditors)
        {
            self.authors.insert(AuthorNameKey::from_parsed(a));
        }
        if let Some(p) = &row.person {
            self.authors.insert(AuthorNameKey::from_parsed(p));
        }
        if let Some(n) = &row.journal_name {
            self.journal_names.insert(n.clone());
        }
        if let Some(n) = &row.publisher_name {
            self.publisher_names.insert(n.clone());
        }
        if let Some(n) = &row.institution_name {
            self.institution_names.insert(n.clone());
        }
        if let Some(n) = &row.school_name {
            self.school_names.insert(n.clone());
        }
        if let Some(n) = &row.series_name {
            self.series_names.insert(n.clone());
        }
        self.keywords_l1
            .extend(row.keywords.level_1.iter().cloned());
        self.keywords_l2
            .extend(row.keywords.level_2.iter().cloned());
        self.keywords_l3
            .extend(row.keywords.level_3.iter().cloned());
        if let Some(cr) = &row.crossref_bibkey {
            self.crossref_bibkeys.insert(cr.clone());
        }
        self.further_ref_bibkeys
            .extend(row.further_ref_bibkeys.iter().cloned());
        self.depends_on_bibkeys
            .extend(row.depends_on_bibkeys.iter().cloned());
    }
}

// =============================================================================
// CSV parsing orchestration (pure)
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

    let csv_headers = CsvHeaders::from_record(&headers);
    let mut parsed_rows = Vec::new();
    let mut row_errors = Vec::new();

    for (idx, result) in rdr.records().enumerate() {
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                row_errors.push(RowError {
                    row: idx + 2, // 1-indexed, skip header
                    bibkey: None,
                    errors: vec![FieldError {
                        field: "_csv".to_string(),
                        error: format!("malformed CSV row: {e}"),
                    }],
                });
                continue;
            }
        };

        match parse_csv_row(&csv_headers, &record) {
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

// =============================================================================
// Classification helpers (pure)
// =============================================================================

pub fn find_missing_names(
    requested: &HashSet<String>,
    existing: &HashMap<String, i64>,
) -> Vec<String> {
    let mut missing: Vec<String> = requested
        .iter()
        .filter(|name| !existing.contains_key(*name))
        .cloned()
        .collect();
    missing.sort();
    missing
}

pub fn find_missing_keywords(
    requested: &HashSet<String>,
    level: i16,
    existing: &HashMap<(String, i16), i64>,
) -> Vec<String> {
    let mut missing: Vec<String> = requested
        .iter()
        .filter(|name| !existing.contains_key(&((*name).clone(), level)))
        .cloned()
        .collect();
    missing.sort();
    missing
}

pub fn find_missing_bibkeys(
    requested: &HashSet<String>,
    existing: &HashSet<String>,
) -> Vec<String> {
    let mut missing: Vec<String> = requested.difference(existing).cloned().collect();
    missing.sort();
    missing
}

// =============================================================================
// Entity creation helpers (pure)
// =============================================================================

pub fn generate_key(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

// =============================================================================
// Bibitem DTO building (pure)
// =============================================================================

pub fn build_bibitem_dto(row: &ParsedBibRow, ctx: &ResolutionCtx) -> Result<CreateBibItem, String> {
    let person_id = row.person.as_ref().and_then(|p| {
        let key = AuthorNameKey::from_parsed(p);
        ctx.author_resolve.get(&key).copied()
    });

    let mut dto = CreateBibItem {
        bibkey: row.bibkey.clone(),
        entry_type: row.entry_type,
        date_year: None,
        date_year_2_hyphen: None,
        date_year_2_slash: None,
        date_month: None,
        date_day: None,
        date_is_no_date: false,
        pubstate: row.pubstate,
        title_latex: row.title.clone(),
        title_unicode: String::new(),
        booktitle_latex: row.booktitle.clone(),
        booktitle_unicode: None,
        journal_id: row
            .journal_name
            .as_ref()
            .and_then(|n| ctx.journal_map.get(n).copied()),
        publisher_id: row
            .publisher_name
            .as_ref()
            .and_then(|n| ctx.publisher_map.get(n).copied()),
        address: row.address.clone(),
        volume: row.volume.clone(),
        number: row.number.clone(),
        pages: row.pages.clone(),
        eid: row.eid.clone(),
        series_id: row
            .series_name
            .as_ref()
            .and_then(|n| ctx.series_map.get(n).copied()),
        edition: row.edition.clone(),
        institution_id: row
            .institution_name
            .as_ref()
            .and_then(|n| ctx.institution_map.get(n).copied()),
        school_id: row
            .school_name
            .as_ref()
            .and_then(|n| ctx.school_map.get(n).copied()),
        type_field: row.type_field.clone(),
        doi: row.doi.clone(),
        url: row.url.clone(),
        eprint: row.eprint.clone(),
        urn: row.urn.clone(),
        crossref_id: None, // resolved after all bibitems inserted -- skip for now
        issuetitle_latex: row.issuetitle.clone(),
        issuetitle_unicode: None,
        note_latex: row.note.clone(),
        note_unicode: None,
        extra_note_latex: row.extra_note.clone(),
        extra_note_unicode: None,
        langid: row.langid,
        is_translation: row.is_translation,
        epoch: row.epoch,
        options: row.options.clone(),
        shorthand: row.shorthand.clone(),
        person_id,
        has_fulltext: row.has_fulltext,
        fulltext_path: None,
    };

    apply_date_to_dto(&row.date, &mut dto);
    Ok(dto)
}

pub fn apply_date_to_dto(date: &ParsedDate, dto: &mut CreateBibItem) {
    match date {
        ParsedDate::NoDate => {
            dto.date_is_no_date = true;
        }
        ParsedDate::Year(y) => {
            dto.date_year = Some(*y);
        }
        ParsedDate::YearRange {
            year,
            year2,
            separator,
        } => {
            dto.date_year = Some(*year);
            match separator {
                DateRangeSeparator::Hyphen => dto.date_year_2_hyphen = Some(*year2),
                DateRangeSeparator::Slash => dto.date_year_2_slash = Some(*year2),
            }
        }
        ParsedDate::FullDate { year, month, day } => {
            dto.date_year = Some(*year);
            dto.date_month = Some(*month);
            dto.date_day = Some(*day);
        }
    }
}

// =============================================================================
// Name variant parsing (pure)
// =============================================================================

pub fn parse_variant_to_keys(variant: &str) -> Vec<AuthorNameKey> {
    if let Ok(parsed) = crate::logic::csv_parsing::author::parse_authors(variant) {
        parsed.iter().map(AuthorNameKey::from_parsed).collect()
    } else {
        vec![AuthorNameKey::Mononym(variant.to_string())]
    }
}

// =============================================================================
// Validation assembly (pure — given lookup data, produce report)
// =============================================================================

/// Assemble a ValidationReport from parsed rows and lookup maps.
/// This is pure: no I/O, just data transformation.
pub fn assemble_validation_report(
    parsed_rows: &[ParsedBibRow],
    row_errors: Vec<RowError>,
    maps: &LookupMaps,
) -> ValidationReport {
    let total_rows = parsed_rows.len() + row_errors.len();

    // Collect all unique names and detect duplicate bibkeys
    let mut collected = CollectedNames::default();
    let mut csv_bibkeys: HashSet<String> = HashSet::new();
    let mut bibkey_rows: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, row) in parsed_rows.iter().enumerate() {
        bibkey_rows
            .entry(row.bibkey.clone())
            .or_default()
            .push(idx + 2); // 1-indexed, skip header
        csv_bibkeys.insert(row.bibkey.clone());
        collected.collect_from_row(row);
    }
    let mut duplicate_bibkeys: Vec<DuplicateBibkey> = bibkey_rows
        .into_iter()
        .filter(|(_, rows)| rows.len() > 1)
        .map(|(bibkey, rows)| DuplicateBibkey { bibkey, rows })
        .collect();
    duplicate_bibkeys.sort_by(|a, b| a.bibkey.cmp(&b.bibkey));

    // Classify authors
    let mut missing_authors = Vec::new();
    let mut ambiguous_authors = Vec::new();
    for key in &collected.authors {
        match maps.authors.id_map.get(key) {
            None => missing_authors.push(format_author_key(key)),
            Some(ids) if ids.len() > 1 => {
                ambiguous_authors.push(AmbiguousAuthor {
                    name: format_author_key(key),
                    matching_ids: ids.clone(),
                });
            }
            _ => {}
        }
    }
    missing_authors.sort();

    // Classify entities
    let missing_journals = find_missing_names(&collected.journal_names, &maps.journals);
    let missing_publishers = find_missing_names(&collected.publisher_names, &maps.publishers);
    let missing_institutions = find_missing_names(&collected.institution_names, &maps.institutions);
    let missing_schools = find_missing_names(&collected.school_names, &maps.schools);
    let missing_series = find_missing_names(&collected.series_names, &maps.series);

    // Classify keywords
    let missing_keywords = MissingKeywords {
        level_1: find_missing_keywords(&collected.keywords_l1, 1, &maps.keywords),
        level_2: find_missing_keywords(&collected.keywords_l2, 2, &maps.keywords),
        level_3: find_missing_keywords(&collected.keywords_l3, 3, &maps.keywords),
    };

    // Classify bibkey references (check against DB + CSV bibkeys)
    let all_known_bibkeys: HashSet<String> = maps.bibkeys.union(&csv_bibkeys).cloned().collect();
    let missing_crossrefs = find_missing_bibkeys(&collected.crossref_bibkeys, &all_known_bibkeys);
    let missing_further_refs =
        find_missing_bibkeys(&collected.further_ref_bibkeys, &all_known_bibkeys);
    let missing_depends_on =
        find_missing_bibkeys(&collected.depends_on_bibkeys, &all_known_bibkeys);

    // Stale bibitems: in DB but not in CSV
    let mut stale_bibitems: Vec<String> = maps.bibkeys.difference(&csv_bibkeys).cloned().collect();
    stale_bibitems.sort();

    ValidationReport {
        total_rows,
        valid_rows: parsed_rows.len(),
        errors: row_errors,
        duplicate_bibkeys,
        missing_authors,
        ambiguous_authors,
        missing_journals,
        missing_publishers,
        missing_institutions,
        missing_schools,
        missing_series,
        missing_keywords,
        missing_crossrefs,
        missing_further_refs,
        missing_depends_on,
        stale_bibitems,
    }
}

// =============================================================================
// Full CSV export helpers (pure)
// =============================================================================

pub const FULL_CSV_HEADERS: &str = "entry_type,bibkey,author,editor,_guesteditor,date,pubstate,title,\
booktitle,journal,publisher,institution,school,series,volume,number,pages,eid,address,type,edition,\
note,_issuetitle,_extra_note,crossref,_further_refs,_depends_on,\
_kw_level1,_kw_level2,_kw_level3,_epoch,_langid,_lang_der,_person,\
_has_link_to_full_text,shorthand,options,doi,url,eprint,urn";

/// Pre-resolved lookup data for building CSV export records.
pub struct ExportContext<'a> {
    pub author_names: &'a HashMap<i64, String>,
    pub journal_names: &'a HashMap<i64, String>,
    pub publisher_names: &'a HashMap<i64, String>,
    pub institution_names: &'a HashMap<i64, String>,
    pub school_names: &'a HashMap<i64, String>,
    pub series_names: &'a HashMap<i64, String>,
    pub keyword_names: &'a HashMap<i64, (String, i16)>,
    pub bibkey_by_id: &'a HashMap<i64, String>,
    pub authors_by_bib: &'a HashMap<i64, Vec<&'a crate::domain::junctions::BibitemAuthorsRow>>,
    pub keywords_by_bib: &'a HashMap<i64, Vec<&'a crate::domain::junctions::BibitemKeywordsRow>>,
    pub refs_by_bib: &'a HashMap<i64, Vec<&'a crate::domain::junctions::BibitemRefsRow>>,
}

/// Build the CSV record for a single bibitem.
///
/// Takes pre-resolved data (name maps, junction data indexed by bibitem ID)
/// and produces a `Vec<String>` of CSV field values.
pub fn build_export_record(bib: &crate::domain::BibItem, ctx: &ExportContext<'_>) -> Vec<String> {
    let authors_for_role = |role: AuthorRole| -> String {
        let role_str = role.to_string();
        ctx.authors_by_bib
            .get(&bib.id)
            .map(|rows| {
                let mut filtered: Vec<&&crate::domain::junctions::BibitemAuthorsRow> =
                    rows.iter().filter(|r| r.role == role_str).collect();
                filtered.sort_by_key(|r| r.position);
                filtered
                    .iter()
                    .filter_map(|r| ctx.author_names.get(&r.author_id))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" and ")
            })
            .unwrap_or_default()
    };

    let keywords_for_level = |level: i16| -> String {
        ctx.keywords_by_bib
            .get(&bib.id)
            .map(|rows| {
                let names: Vec<&str> = rows
                    .iter()
                    .filter_map(|r| {
                        ctx.keyword_names.get(&r.keyword_id).and_then(|(name, lv)| {
                            if *lv == level {
                                Some(name.as_str())
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                if names.is_empty() {
                    String::new()
                } else {
                    format!("{};", names.join("; "))
                }
            })
            .unwrap_or_default()
    };

    let refs_for_type = |rt: RefType| -> String {
        let rt_str = rt.to_string();
        ctx.refs_by_bib
            .get(&bib.id)
            .map(|rows| {
                rows.iter()
                    .filter(|r| r.ref_type == rt_str)
                    .filter_map(|r| ctx.bibkey_by_id.get(&r.target_id))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    };

    let date = format_date_for_export(bib);
    let crossref = bib
        .crossref_id
        .and_then(|id| ctx.bibkey_by_id.get(&id))
        .cloned()
        .unwrap_or_default();
    let person = bib
        .person_id
        .and_then(|id| ctx.author_names.get(&id))
        .map(|n| format!("{n};"))
        .unwrap_or_default();

    vec![
        bib.entry_type.to_string(),
        bib.bibkey.clone(),
        authors_for_role(AuthorRole::Author),
        authors_for_role(AuthorRole::Editor),
        authors_for_role(AuthorRole::Guesteditor),
        date,
        bib.pubstate.map(|p| p.to_string()).unwrap_or_default(),
        bib.title_latex.clone(),
        bib.booktitle_latex.clone().unwrap_or_default(),
        bib.journal_id
            .and_then(|id| ctx.journal_names.get(&id))
            .cloned()
            .unwrap_or_default(),
        bib.publisher_id
            .and_then(|id| ctx.publisher_names.get(&id))
            .cloned()
            .unwrap_or_default(),
        bib.institution_id
            .and_then(|id| ctx.institution_names.get(&id))
            .cloned()
            .unwrap_or_default(),
        bib.school_id
            .and_then(|id| ctx.school_names.get(&id))
            .cloned()
            .unwrap_or_default(),
        bib.series_id
            .and_then(|id| ctx.series_names.get(&id))
            .cloned()
            .unwrap_or_default(),
        bib.volume.clone().unwrap_or_default(),
        bib.number.clone().unwrap_or_default(),
        bib.pages.clone().unwrap_or_default(),
        bib.eid.clone().unwrap_or_default(),
        bib.address.clone().unwrap_or_default(),
        bib.type_field.clone().unwrap_or_default(),
        bib.edition.clone().unwrap_or_default(),
        bib.note_latex.clone().unwrap_or_default(),
        bib.issuetitle_latex.clone().unwrap_or_default(),
        bib.extra_note_latex.clone().unwrap_or_default(),
        crossref,
        refs_for_type(RefType::FurtherRef),
        refs_for_type(RefType::DependsOn),
        keywords_for_level(1),
        keywords_for_level(2),
        keywords_for_level(3),
        bib.epoch.map(|e| e.to_string()).unwrap_or_default(),
        bib.langid.map(|l| l.to_string()).unwrap_or_default(),
        if bib.is_translation {
            "x".to_string()
        } else {
            String::new()
        },
        person,
        if bib.has_fulltext {
            "x".to_string()
        } else {
            String::new()
        },
        bib.shorthand.clone().unwrap_or_default(),
        bib.options.clone().unwrap_or_default(),
        bib.doi.clone().unwrap_or_default(),
        bib.url.clone().unwrap_or_default(),
        bib.eprint.clone().unwrap_or_default(),
        bib.urn.clone().unwrap_or_default(),
    ]
}

pub fn format_date_for_export(bib: &crate::domain::BibItem) -> String {
    if bib.date_is_no_date {
        return "no date".to_string();
    }
    match (bib.date_year, bib.date_month, bib.date_day) {
        (Some(y), Some(m), Some(d)) => format!("{y}-{m:02}-{d:02}"),
        (Some(y), _, _) => {
            if let Some(y2) = bib.date_year_2_hyphen {
                format!("{y}-{y2}")
            } else if let Some(y2) = bib.date_year_2_slash {
                format!("{y}/{y2}")
            } else {
                y.to_string()
            }
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::generate_key;

    #[test]
    fn basic_lowercase() {
        assert_eq!(generate_key("MIT"), "mit");
    }

    #[test]
    fn spaces_become_single_underscore() {
        assert_eq!(
            generate_key("University of Toronto"),
            "university_of_toronto"
        );
    }

    #[test]
    fn parens_dont_produce_double_underscore() {
        assert_eq!(
            generate_key("COINS (University of Massachusetts)"),
            "coins_university_of_massachusetts"
        );
    }

    #[test]
    fn leading_trailing_special_chars_trimmed() {
        assert_eq!(generate_key("(MIT)"), "mit");
    }

    #[test]
    fn multiple_consecutive_separators_collapsed() {
        assert_eq!(generate_key("A  --  B"), "a_b");
    }
}
