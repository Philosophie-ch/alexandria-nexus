use serde::Serialize;

use crate::domain::{EntryType, Epoch, LangId, PubState};

// =============================================================================
// Parsed author
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

// =============================================================================
// Parsed date
// =============================================================================

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

// =============================================================================
// Parsed bibkey
// =============================================================================

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

// =============================================================================
// Parsed keywords
// =============================================================================

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedKeywords {
    pub level_1: Vec<String>,
    pub level_2: Vec<String>,
    pub level_3: Vec<String>,
}

// =============================================================================
// Row parsing results
// =============================================================================

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

// =============================================================================
// Full parsed row
// =============================================================================

#[derive(Debug, Clone)]
pub struct ParsedBibRow {
    // Identity
    pub bibkey: String,
    pub entry_type: EntryType,

    // People (parsed names, not yet resolved to IDs)
    pub authors: Vec<ParsedAuthor>,
    pub editors: Vec<ParsedAuthor>,
    pub guesteditors: Vec<ParsedAuthor>,
    pub person: Option<ParsedAuthor>,

    // Date
    pub date: ParsedDate,
    pub pubstate: Option<PubState>,

    // Title (from single CSV column, stored for all three variants)
    pub title: String,
    pub booktitle: Option<String>,

    // Entity references (human-readable names, not IDs)
    pub journal_name: Option<String>,
    pub publisher_name: Option<String>,
    pub institution_name: Option<String>,
    pub school_name: Option<String>,
    pub series_name: Option<String>,

    // Bibitem references (bibkeys, not IDs)
    pub crossref_bibkey: Option<String>,
    pub further_ref_bibkeys: Vec<String>,
    pub depends_on_bibkeys: Vec<String>,

    // Keywords (names, not IDs)
    pub keywords: ParsedKeywords,

    // Simple fields
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
    pub options: Option<String>,

    // Identifiers
    pub doi: Option<String>,
    pub url: Option<String>,
    pub eprint: Option<String>,
    pub urn: Option<String>,

    // Metadata
    pub epoch: Option<Epoch>,
    pub langid: Option<LangId>,
    pub is_translation: bool,
    pub has_fulltext: bool,
}
