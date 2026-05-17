//! Full import — pure types and business logic for bulk bibitem import.
//!
//! This module contains ZERO async, ZERO I/O, ZERO AppState.
//! All I/O orchestration lives in `crate::process::full_import`.
//! All format-specific parsing lives in `crate::adapters::field_parsing`.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::domain::{AuthorRole, CreateBibItem, EntryType, Epoch, LangId, PubState, RefType};
use crate::logic::latex_citations::extract_cite_keys;
use crate::logic::pages::compute_start_page;

// =============================================================================
// Parsed data types (produced by adapters/field_parsing, consumed by process)
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParsedAuthor {
    Named {
        family_name: String,
        given_name: Option<String>,
    },
    Mononym(String),
}

impl ParsedAuthor {
    pub fn display_name(&self) -> String {
        match self {
            ParsedAuthor::Mononym(m) => m.clone(),
            ParsedAuthor::Named {
                family_name,
                given_name,
            } => match given_name {
                Some(g) => format!("{family_name}, {g}"),
                None => family_name.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateRangeSeparator {
    Hyphen,
    Slash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedDate {
    NoDate,
    Year(i16),
    YearRange {
        year: i16,
        year2: i16,
        separator: DateRangeSeparator,
    },
    FullDate {
        year: i16,
        month: i16,
        day: i16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BibkeyDate {
    Year(i16),
    Unpub,
    Forthcoming,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBibkey {
    pub full: String,
    pub first_author: String,
    pub other_authors: Option<String>,
    pub date: BibkeyDate,
    pub suffix: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedKeywords {
    pub level_1: Vec<String>,
    pub level_2: Vec<String>,
    pub level_3: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldError {
    pub field: String,
    pub error: String,
}

pub enum RowParseResult {
    Ok(Box<ParsedBibRow>),
    Err {
        bibkey: Option<String>,
        errors: Vec<FieldError>,
    },
}

#[derive(Debug, Clone)]
pub struct ParsedBibRow {
    pub bibkey: String,
    pub entry_type: EntryType,

    pub authors: Vec<ParsedAuthor>,
    pub editors: Vec<ParsedAuthor>,
    pub guesteditors: Vec<ParsedAuthor>,
    pub person: Option<ParsedAuthor>,

    pub date: ParsedDate,
    pub pubstate: Option<PubState>,

    pub title: String,
    pub booktitle: Option<String>,

    pub journal_name: Option<String>,
    pub publisher_name: Option<String>,
    pub institution_name: Option<String>,
    pub school_name: Option<String>,
    pub series_name: Option<String>,

    pub crossref_bibkey: Option<String>,

    pub keywords: ParsedKeywords,

    pub volume: Option<String>,
    pub number: Option<String>,
    pub pages: Option<String>,
    pub eid: Option<String>,
    pub address: Option<String>,
    pub type_field: Option<String>,
    pub edition: Option<String>,
    pub note: Option<String>,
    pub issuetitle: Option<String>,
    pub extra_note: Option<String>,
    pub shorthand: Option<String>,

    pub note_perso: Option<String>,
    pub note_stock: Option<String>,
    pub note_missing: Option<String>,
    pub change_request: Option<String>,
    pub dltc_copyediting_note: Option<String>,
    pub todo_general: Option<String>,
    pub options: Option<String>,

    pub doi: Option<String>,
    pub url: Option<String>,
    pub eprint: Option<String>,
    pub urn: Option<String>,

    pub epoch: Option<Epoch>,
    pub langid: Option<LangId>,
    pub is_translation: bool,
    pub has_fulltext: bool,
    pub license: Option<String>,
}

// =============================================================================
// Response types
// =============================================================================

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct RowError {
    pub row: usize,
    pub bibkey: Option<String>,
    pub errors: Vec<FieldError>,
}

#[derive(Debug, Serialize)]
pub struct AmbiguousAuthor {
    pub name: String,
    pub matching_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct DuplicateBibkey {
    pub bibkey: String,
    pub rows: Vec<usize>,
}

#[derive(Debug, Serialize)]
pub struct MissingKeywords {
    pub level_1: Vec<String>,
    pub level_2: Vec<String>,
    pub level_3: Vec<String>,
}

impl ValidationReport {
    /// Hard errors: conditions that block the entire import (not just individual rows).
    /// Only duplicate bibkeys qualify — they'd cause an upsert conflict across all rows.
    ///
    /// Missing/ambiguous entity refs are per-row: `filter_importable_rows` skips affected rows
    /// while letting the rest through. Row-level parse errors and missing cross-bibitem refs
    /// are also soft — those rows are silently skipped.
    pub fn has_hard_errors(&self) -> bool {
        !self.duplicate_bibkeys.is_empty()
    }

    /// Any issue at all — used by the validate-only endpoint to surface everything.
    pub fn has_issues(&self) -> bool {
        self.has_hard_errors()
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

#[derive(Debug, Serialize)]
pub struct EntityImportReport {
    pub created_institutions: usize,
    pub created_schools: usize,
    pub created_series: usize,
    pub created_keywords: usize,
    pub errors: Vec<EntityImportError>,
}

#[derive(Debug, Serialize)]
pub struct EntityImportError {
    pub entity_type: NamedEntityKind,
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

/// Aggregate report for a batch LaTeX-to-Unicode conversion operation.
#[derive(Debug, Serialize)]
pub struct LatexConvertReport {
    pub columns: Vec<ColumnConvertResult>,
    pub total_updated: usize,
    pub errors: Vec<LatexConvertError>,
    /// Bibkeys referenced by `\cite*{...}` commands in LaTeX fields but not found among existing entries.
    pub missing_citation_keys: Vec<String>,
}

/// Stats for one converted column.
#[derive(Debug, Serialize)]
pub struct ColumnConvertResult {
    pub entity: &'static str,
    pub field: &'static str,
    pub updated: usize,
}

/// A row whose LaTeX value could not be converted — mirrors `EntityImportError`.
#[derive(Debug, Serialize)]
pub struct LatexConvertError {
    pub entity: &'static str,
    pub field: &'static str,
    pub id: i64,
    pub error: String,
}

// =============================================================================
// Full import report types
// =============================================================================

#[derive(Debug, Serialize)]
pub struct FullImportReport {
    pub imported: usize,
    pub updated: usize,
    pub deleted: usize,
    pub failed: usize,
    /// Rows skipped because one or more entity references (author, journal, etc.) could not be resolved.
    pub skipped: usize,
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
    /// AuthorNameKey -> list of matching author_key strings (parallel to id_map)
    pub key_map: HashMap<AuthorNameKey, Vec<String>>,
    /// AuthorNameKey -> variant info (only for keys matched via a name variant)
    pub variant_map: HashMap<AuthorNameKey, VariantInfo>,
    /// name string -> author IDs, famous authors only (for _person resolution)
    pub famous_person_map: HashMap<String, Vec<i64>>,
    /// name string -> author_key strings, famous authors only (parallel to famous_person_map)
    pub famous_person_key_map: HashMap<String, Vec<String>>,
}

// =============================================================================
// Lookup maps (bundles all lookup data to avoid too-many-args)
// =============================================================================

/// All DB lookup maps, built once per import operation.
pub struct LookupMaps {
    pub authors: AuthorLookupResult,
    pub journals: HashMap<String, String>,
    pub publishers: HashMap<String, String>,
    pub institutions: HashMap<String, String>,
    pub schools: HashMap<String, String>,
    pub series: HashMap<String, String>,
    pub keywords: HashMap<(String, i16), String>,
    pub bibkeys: HashSet<String>,
}

// =============================================================================
// Resolution context (bundles all lookup maps to avoid too-many-args)
// =============================================================================

/// Resolution context for bibitem upsert (single-match authors only).
pub struct ResolutionCtx {
    pub author_resolve: HashMap<AuthorNameKey, i64>,
    /// AuthorNameKey → single author_key string (unambiguous matches only).
    pub author_key_resolve: HashMap<AuthorNameKey, String>,
    /// For keys that matched via a name variant, stores the variant strings.
    pub author_variants: HashMap<AuthorNameKey, VariantInfo>,
    /// Unambiguous famous-author name → ID, used exclusively for _person resolution.
    pub famous_person_resolve: HashMap<String, i64>,
    /// Unambiguous famous-author name → author_key string.
    pub famous_person_key_resolve: HashMap<String, String>,
    pub journal_map: HashMap<String, String>,
    pub publisher_map: HashMap<String, String>,
    pub institution_map: HashMap<String, String>,
    pub school_map: HashMap<String, String>,
    pub series_map: HashMap<String, String>,
    pub keyword_map: HashMap<(String, i16), String>,
    pub existing_bibkeys: HashSet<String>,
}

impl ResolutionCtx {
    /// Build a ResolutionCtx from LookupMaps (consumes the maps).
    pub fn from_lookup_maps(maps: LookupMaps) -> Self {
        let author_variants = maps.authors.variant_map;
        let famous_person_resolve = maps
            .authors
            .famous_person_map
            .into_iter()
            .filter_map(|(k, ids)| {
                if ids.len() == 1 {
                    Some((k, ids[0]))
                } else {
                    None
                }
            })
            .collect();
        let famous_person_key_resolve = maps
            .authors
            .famous_person_key_map
            .into_iter()
            .filter_map(|(k, keys)| {
                if keys.len() == 1 {
                    keys.into_iter().next().map(|v| (k, v))
                } else {
                    None
                }
            })
            .collect();
        let author_key_resolve = maps
            .authors
            .key_map
            .into_iter()
            .filter_map(|(k, keys)| {
                if keys.len() == 1 {
                    keys.into_iter().next().map(|v| (k, v))
                } else {
                    None
                }
            })
            .collect();
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
            author_key_resolve,
            author_variants,
            famous_person_resolve,
            famous_person_key_resolve,
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

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NamedEntityKind {
    Institution,
    School,
    Series,
    Keyword,
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
            self.depends_on_bibkeys.insert(cr.clone());
        }
        self.further_ref_bibkeys
            .extend(extract_cite_keys(&row.title));
        if let Some(note) = &row.note {
            self.further_ref_bibkeys.extend(extract_cite_keys(note));
        }
        if let Some(extra) = &row.extra_note {
            self.depends_on_bibkeys.extend(extract_cite_keys(extra));
        }
    }
}

// =============================================================================
// Classification helpers (pure)
// =============================================================================

pub fn find_missing_names(
    requested: &HashSet<String>,
    existing: &HashMap<String, String>,
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
    existing: &HashMap<(String, i16), String>,
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
// Person resolution (three-step fallback)
// =============================================================================

/// Resolve a `_person` entry to a famous author ID.
///
/// Only matches authors marked `famous = true`. The name string from the input
/// (e.g. "Kant", "Aristotle") is looked up directly in `famous_person_resolve`,
/// which is indexed by stripped mononym or family name for famous authors only.
pub fn resolve_person_id(person: &ParsedAuthor, ctx: &ResolutionCtx) -> Option<i64> {
    let name = match person {
        ParsedAuthor::Mononym(m) => m.as_str(),
        ParsedAuthor::Named { family_name, .. } => family_name.as_str(),
    };
    ctx.famous_person_resolve.get(name).copied()
}

// =============================================================================
// Bibitem DTO building (pure)
// =============================================================================

pub fn build_bibitem_dto(row: &ParsedBibRow, ctx: &ResolutionCtx) -> Result<CreateBibItem, String> {
    let person_key = row.person.as_ref().and_then(|p| {
        let name = match p {
            ParsedAuthor::Mononym(m) => m.as_str(),
            ParsedAuthor::Named { family_name, .. } => family_name.as_str(),
        };
        ctx.famous_person_key_resolve.get(name).cloned()
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
        title_unicode: None,
        booktitle_latex: row.booktitle.clone(),
        booktitle_unicode: None,
        journal_key: row
            .journal_name
            .as_ref()
            .and_then(|n| ctx.journal_map.get(n).cloned()),
        publisher_key: row
            .publisher_name
            .as_ref()
            .and_then(|n| ctx.publisher_map.get(n).cloned()),
        address: row.address.clone(),
        volume: row.volume.clone(),
        volume_numeric: None,
        number: row.number.clone(),
        number_numeric: None,
        pages: row.pages.clone(),
        start_page: None,
        eid: row.eid.clone(),
        series_key: row
            .series_name
            .as_ref()
            .and_then(|n| ctx.series_map.get(n).cloned()),
        edition: row.edition.clone(),
        institution_key: row
            .institution_name
            .as_ref()
            .and_then(|n| ctx.institution_map.get(n).cloned()),
        school_key: row
            .school_name
            .as_ref()
            .and_then(|n| ctx.school_map.get(n).cloned()),
        type_field: row.type_field.clone(),
        doi: row.doi.clone(),
        url: row.url.clone(),
        eprint: row.eprint.clone(),
        urn: row.urn.clone(),
        crossref: None, // resolved after all bibitems inserted -- skip for now
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
        person_key,
        has_fulltext: row.has_fulltext,
        fulltext_path: None,
        license: row.license.clone(),
    };

    apply_date_to_dto(&row.date, &mut dto);
    dto.start_page = compute_start_page(dto.pages.as_deref());
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
    let mut file_bibkeys: HashSet<String> = HashSet::new();
    let mut bibkey_rows: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, row) in parsed_rows.iter().enumerate() {
        bibkey_rows
            .entry(row.bibkey.clone())
            .or_default()
            .push(idx + 2); // 1-indexed, skip header
        file_bibkeys.insert(row.bibkey.clone());
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

    // Classify bibkey references (check against DB + input bibkeys)
    let all_known_bibkeys: HashSet<String> = maps.bibkeys.union(&file_bibkeys).cloned().collect();
    let missing_crossrefs = find_missing_bibkeys(&collected.crossref_bibkeys, &all_known_bibkeys);
    let missing_further_refs =
        find_missing_bibkeys(&collected.further_ref_bibkeys, &all_known_bibkeys);
    let missing_depends_on =
        find_missing_bibkeys(&collected.depends_on_bibkeys, &all_known_bibkeys);

    // Stale bibitems: in DB but not in input
    let mut stale_bibitems: Vec<String> = maps.bibkeys.difference(&file_bibkeys).cloned().collect();
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
// Export helpers (pure)
// =============================================================================

/// Pre-resolved lookup data for building export records.
pub struct ExportContext {
    pub author_names: HashMap<String, String>,
    pub journal_names: HashMap<String, String>,
    pub publisher_names: HashMap<String, String>,
    pub institution_names: HashMap<String, String>,
    pub school_names: HashMap<String, String>,
    pub series_names: HashMap<String, String>,
    pub keyword_names: HashMap<String, (String, i16)>,
    pub bibkey_by_id: HashMap<i64, String>,
    pub authors_by_bib: HashMap<String, Vec<crate::domain::junctions::BibitemAuthorsRow>>,
    pub keywords_by_bib: HashMap<String, Vec<crate::domain::junctions::BibitemKeywordsRow>>,
    pub notes_by_bib: HashMap<String, crate::domain::BibitemNotes>,
}

// =============================================================================
// Bulk import junction row types (pure data — no I/O)
// =============================================================================

pub struct AuthorJunctionRow {
    pub bibkey: String,
    pub author_key: String,
    pub role: AuthorRole,
    pub position: i16,
    pub variant_latex: Option<String>,
    pub variant_unicode: Option<String>,
}

pub struct KeywordJunctionRow {
    pub bibkey: String,
    pub keyword_key: String,
    pub keyword_level: i16,
}

pub struct BibitemRefInsertRow {
    pub source_bibkey: String,
    pub target_bibkey: String,
    pub ref_type: RefType,
}

// =============================================================================
// Bulk junction collectors (pure)
// =============================================================================

/// Collect all author junction rows for all parsed rows.
/// Authors that cannot be resolved are silently skipped (already reported in validation).
pub fn collect_author_junction_rows(
    parsed_rows: &[ParsedBibRow],
    ctx: &ResolutionCtx,
) -> Vec<AuthorJunctionRow> {
    let mut rows = Vec::new();
    for parsed in parsed_rows {
        for (role, authors) in [
            (AuthorRole::Author, &parsed.authors),
            (AuthorRole::Editor, &parsed.editors),
            (AuthorRole::Guesteditor, &parsed.guesteditors),
        ] {
            for (position, author) in authors.iter().enumerate() {
                let key = AuthorNameKey::from_parsed(author);
                let Some(author_key) = ctx.author_key_resolve.get(&key).cloned() else {
                    continue;
                };
                let Ok(pos) = i16::try_from(position) else {
                    continue;
                };
                let variant = ctx.author_variants.get(&key);
                rows.push(AuthorJunctionRow {
                    bibkey: parsed.bibkey.clone(),
                    author_key,
                    role,
                    position: pos,
                    variant_latex: variant.and_then(|v| v.variant_latex.clone()),
                    variant_unicode: variant.and_then(|v| v.variant_unicode.clone()),
                });
            }
        }
    }
    rows
}

/// Collect all keyword junction rows for all parsed rows.
/// Keywords that cannot be resolved are silently skipped.
pub fn collect_keyword_junction_rows(
    parsed_rows: &[ParsedBibRow],
    ctx: &ResolutionCtx,
) -> Vec<KeywordJunctionRow> {
    let mut rows = Vec::new();
    for parsed in parsed_rows {
        let all_keywords: Vec<(&String, i16)> = parsed
            .keywords
            .level_1
            .iter()
            .map(|k| (k, 1i16))
            .chain(parsed.keywords.level_2.iter().map(|k| (k, 2i16)))
            .chain(parsed.keywords.level_3.iter().map(|k| (k, 3i16)))
            .collect();
        for (name, level) in all_keywords {
            let Some(keyword_key) = ctx.keyword_map.get(&(name.clone(), level)).cloned() else {
                continue;
            };
            rows.push(KeywordJunctionRow {
                bibkey: parsed.bibkey.clone(),
                keyword_key,
                keyword_level: level,
            });
        }
    }
    rows
}

// =============================================================================
// Per-row entity resolution filter (pure)
// =============================================================================

/// Partition rows into importable (all entity refs resolve) and skipped (at least one missing).
///
/// Uses only O(1) HashMap lookups against the already-loaded `ResolutionCtx`.
/// Rows that fail are emitted as `RowError` entries explaining which references are unresolved.
pub fn filter_importable_rows(
    rows: Vec<ParsedBibRow>,
    ctx: &ResolutionCtx,
) -> (Vec<ParsedBibRow>, Vec<RowError>) {
    let mut importable = Vec::new();
    let mut skipped = Vec::new();
    for (idx, row) in rows.into_iter().enumerate() {
        let missing = collect_missing_for_row(&row, ctx);
        if missing.is_empty() {
            importable.push(row);
        } else {
            skipped.push(RowError {
                row: idx + 2, // 1-indexed, skip header row
                bibkey: Some(row.bibkey.clone()),
                errors: missing
                    .into_iter()
                    .map(|m| FieldError {
                        field: "entity_reference".to_string(),
                        error: m,
                    })
                    .collect(),
            });
        }
    }
    (importable, skipped)
}

fn collect_missing_for_row(row: &ParsedBibRow, ctx: &ResolutionCtx) -> Vec<String> {
    let mut missing = Vec::new();
    for author in row
        .authors
        .iter()
        .chain(&row.editors)
        .chain(&row.guesteditors)
    {
        let key = AuthorNameKey::from_parsed(author);
        if !ctx.author_resolve.contains_key(&key) {
            missing.push(format!("unresolved author: {}", format_author_key(&key)));
        }
    }
    if let Some(p) = &row.person
        && resolve_person_id(p, ctx).is_none()
    {
        let key = AuthorNameKey::from_parsed(p);
        missing.push(format!("unresolved person: {}", format_author_key(&key)));
    }
    if let Some(n) = &row.journal_name
        && !ctx.journal_map.contains_key(n)
    {
        missing.push(format!("missing journal: {n}"));
    }
    if let Some(n) = &row.publisher_name
        && !ctx.publisher_map.contains_key(n)
    {
        missing.push(format!("missing publisher: {n}"));
    }
    if let Some(n) = &row.institution_name
        && !ctx.institution_map.contains_key(n)
    {
        missing.push(format!("missing institution: {n}"));
    }
    if let Some(n) = &row.school_name
        && !ctx.school_map.contains_key(n)
    {
        missing.push(format!("missing school: {n}"));
    }
    if let Some(n) = &row.series_name
        && !ctx.series_map.contains_key(n)
    {
        missing.push(format!("missing series: {n}"));
    }
    missing
}

/// Collect all bibitem ref rows for all parsed rows.
///
/// Further-ref sources: `\cite*` commands in `title` and `note` fields.
/// Depends-on sources: `\cite*` commands in `extra_note` and `crossref_bibkey`.
pub fn collect_ref_rows(parsed_rows: &[ParsedBibRow]) -> Vec<BibitemRefInsertRow> {
    let mut rows = Vec::new();
    for parsed in parsed_rows {
        let title_keys = extract_cite_keys(&parsed.title);
        let note_keys = parsed
            .note
            .as_deref()
            .map(extract_cite_keys)
            .unwrap_or_default();
        let further: Vec<&str> = title_keys
            .iter()
            .chain(note_keys.iter())
            .map(String::as_str)
            .collect();

        let extra_keys = parsed
            .extra_note
            .as_deref()
            .map(extract_cite_keys)
            .unwrap_or_default();
        let mut depends_on: Vec<&str> = extra_keys.iter().map(String::as_str).collect();
        if let Some(cr) = parsed.crossref_bibkey.as_deref() {
            depends_on.push(cr);
        }

        for bibkey in further {
            rows.push(BibitemRefInsertRow {
                source_bibkey: parsed.bibkey.clone(),
                target_bibkey: bibkey.to_string(),
                ref_type: RefType::FurtherRef,
            });
        }
        for bibkey in depends_on {
            rows.push(BibitemRefInsertRow {
                source_bibkey: parsed.bibkey.clone(),
                target_bibkey: bibkey.to_string(),
                ref_type: RefType::DependsOn,
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // resolve_person_id
    // -------------------------------------------------------------------------

    fn ctx_with_famous(famous: &[(&str, i64)]) -> ResolutionCtx {
        let famous_person_resolve = famous
            .iter()
            .map(|(name, id)| (name.to_string(), *id))
            .collect();
        ResolutionCtx {
            author_resolve: HashMap::new(),
            author_key_resolve: HashMap::new(),
            author_variants: HashMap::new(),
            famous_person_resolve,
            famous_person_key_resolve: HashMap::new(),
            journal_map: HashMap::new(),
            publisher_map: HashMap::new(),
            institution_map: HashMap::new(),
            school_map: HashMap::new(),
            series_map: HashMap::new(),
            keyword_map: HashMap::new(),
            existing_bibkeys: Default::default(),
        }
    }

    fn mononym(m: &str) -> ParsedAuthor {
        ParsedAuthor::Mononym(m.to_string())
    }

    #[test]
    fn person_resolved_by_famous_mononym() {
        let ctx = ctx_with_famous(&[("Aristotle", 1)]);
        assert_eq!(resolve_person_id(&mononym("Aristotle"), &ctx), Some(1));
    }

    #[test]
    fn person_resolved_by_famous_family_name() {
        // Famous named author indexed by family name
        let ctx = ctx_with_famous(&[("Kierkegaard", 42)]);
        assert_eq!(resolve_person_id(&mononym("Kierkegaard"), &ctx), Some(42));
    }

    #[test]
    fn person_not_famous_returns_none() {
        // "Müller" not in famous map → no resolution regardless of how many Müllers exist
        let ctx = ctx_with_famous(&[("Kant", 1)]);
        assert_eq!(resolve_person_id(&mononym("Müller"), &ctx), None);
    }

    #[test]
    fn person_not_found_returns_none() {
        let ctx = ctx_with_famous(&[("Aristotle", 1)]);
        assert_eq!(resolve_person_id(&mononym("Plato"), &ctx), None);
    }

    #[test]
    fn person_empty_famous_map_returns_none() {
        let ctx = ctx_with_famous(&[]);
        assert_eq!(resolve_person_id(&mononym("Kant"), &ctx), None);
    }

    // -------------------------------------------------------------------------
    // generate_key (existing tests)
    // -------------------------------------------------------------------------

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
