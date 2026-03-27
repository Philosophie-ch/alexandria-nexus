//! BibItem DTOs for API requests.

use serde::Deserialize;
use utoipa::ToSchema;

use crate::domain::{EntryType, Epoch, LangId, PubState};

/// DTO for creating a bibitem.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateBibItem {
    // === Identity (required) ===
    pub bibkey: String,
    pub entry_type: EntryType,

    // === Dates ===
    pub date_year: Option<i16>,
    pub date_year_2_hyphen: Option<i16>,
    pub date_year_2_slash: Option<i16>,
    pub date_month: Option<i16>,
    pub date_day: Option<i16>,
    pub date_is_no_date: Option<bool>,

    // === Pubstate ===
    pub pubstate: Option<PubState>,

    // === Title (required) ===
    pub title_latex: String,
    pub title_unicode: String,
    pub title_simplified: String,

    // === Booktitle (optional, for @incollection) ===
    pub booktitle_latex: Option<String>,
    pub booktitle_unicode: Option<String>,
    pub booktitle_simplified: Option<String>,

    // === Publication Info ===
    pub journal_id: Option<i64>,
    pub publisher_id: Option<i64>,
    pub address: Option<String>,
    pub volume: Option<String>,
    pub number: Option<String>,
    pub pages: Option<String>,
    pub eid: Option<String>,
    pub series_id: Option<i64>,
    pub edition: Option<String>,

    // === Institutional ===
    pub institution_id: Option<i64>,
    pub school_id: Option<i64>,
    pub type_field: Option<String>,

    // === Identifiers ===
    pub doi: Option<String>,
    pub url: Option<String>,
    pub eprint: Option<String>,
    pub urn: Option<String>,

    // === References ===
    pub crossref_id: Option<i64>,

    // === Issue/Notes ===
    pub issuetitle_latex: Option<String>,
    pub issuetitle_unicode: Option<String>,
    pub note_latex: Option<String>,
    pub note_unicode: Option<String>,
    pub extra_note_latex: Option<String>,
    pub extra_note_unicode: Option<String>,

    // === Metadata ===
    pub langid: Option<LangId>,
    pub is_translation: Option<bool>,
    pub epoch: Option<Epoch>,
    pub options: Option<String>,
    pub shorthand: Option<String>,

    // === Internal Tracking ===
    pub person_id: Option<i64>,
    pub has_fulltext: Option<bool>,
    pub fulltext_path: Option<String>,
}

/// DTO for updating a bibitem. All fields are optional for partial updates.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateBibItem {
    pub bibkey: Option<String>,
    pub entry_type: Option<EntryType>,

    // === Dates ===
    pub date_year: Option<i16>,
    pub date_year_2_hyphen: Option<i16>,
    pub date_year_2_slash: Option<i16>,
    pub date_month: Option<i16>,
    pub date_day: Option<i16>,
    pub date_is_no_date: Option<bool>,

    // === Pubstate ===
    pub pubstate: Option<PubState>,

    // === Title ===
    pub title_latex: Option<String>,
    pub title_unicode: Option<String>,
    pub title_simplified: Option<String>,

    // === Booktitle ===
    pub booktitle_latex: Option<String>,
    pub booktitle_unicode: Option<String>,
    pub booktitle_simplified: Option<String>,

    // === Publication Info ===
    pub journal_id: Option<i64>,
    pub publisher_id: Option<i64>,
    pub address: Option<String>,
    pub volume: Option<String>,
    pub number: Option<String>,
    pub pages: Option<String>,
    pub eid: Option<String>,
    pub series_id: Option<i64>,
    pub edition: Option<String>,

    // === Institutional ===
    pub institution_id: Option<i64>,
    pub school_id: Option<i64>,
    pub type_field: Option<String>,

    // === Identifiers ===
    pub doi: Option<String>,
    pub url: Option<String>,
    pub eprint: Option<String>,
    pub urn: Option<String>,

    // === References ===
    pub crossref_id: Option<i64>,

    // === Issue/Notes ===
    pub issuetitle_latex: Option<String>,
    pub issuetitle_unicode: Option<String>,
    pub note_latex: Option<String>,
    pub note_unicode: Option<String>,
    pub extra_note_latex: Option<String>,
    pub extra_note_unicode: Option<String>,

    // === Metadata ===
    pub langid: Option<LangId>,
    pub is_translation: Option<bool>,
    pub epoch: Option<Epoch>,
    pub options: Option<String>,
    pub shorthand: Option<String>,

    // === Internal Tracking ===
    pub person_id: Option<i64>,
    pub has_fulltext: Option<bool>,
    pub fulltext_path: Option<String>,
}
