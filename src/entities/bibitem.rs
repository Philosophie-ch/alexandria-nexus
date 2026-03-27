//! BibItem entity — maps to the `bibitems` table.
//!
//! This is the main bibliography entry with 46+ columns covering identity,
//! dates, title, publication info, identifiers, references, and metadata.

use chrono::{DateTime, Utc};
use hexforge::Entity;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::{EntryType, Epoch, LangId, PubState};

/// A bibliography item — the core entity of the system.
#[derive(Entity, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[entity(table = "bibitems")]
pub struct BibItem {
    #[entity(id)]
    pub id: i64,

    // Identity
    pub bibkey: String,
    pub entry_type: EntryType,

    // Dates
    pub date_year: Option<i16>,
    pub date_year_2_hyphen: Option<i16>,
    pub date_year_2_slash: Option<i16>,
    pub date_month: Option<i16>,
    pub date_day: Option<i16>,
    pub date_is_no_date: bool,
    pub pubstate: Option<PubState>,

    // Title (BibStringAttr: latex, unicode, simplified)
    pub title_latex: String,
    pub title_unicode: String,
    pub title_simplified: String,

    // Booktitle (for @incollection)
    pub booktitle_latex: Option<String>,
    pub booktitle_unicode: Option<String>,
    pub booktitle_simplified: Option<String>,

    // Publication info
    pub journal_id: Option<i64>,
    pub publisher_id: Option<i64>,
    pub address: Option<String>,
    pub volume: Option<String>,
    pub number: Option<String>,
    pub pages: Option<String>,
    pub eid: Option<String>,
    pub series_id: Option<i64>,
    pub edition: Option<String>,

    // Institutional
    pub institution_id: Option<i64>,
    pub school_id: Option<i64>,
    pub type_field: Option<String>,

    // Identifiers
    pub doi: Option<String>,
    pub url: Option<String>,
    pub eprint: Option<String>,
    pub urn: Option<String>,

    // References
    pub crossref_id: Option<i64>,

    // Issue/notes
    pub issuetitle_latex: Option<String>,
    pub issuetitle_unicode: Option<String>,
    pub note_latex: Option<String>,
    pub note_unicode: Option<String>,
    pub extra_note_latex: Option<String>,
    pub extra_note_unicode: Option<String>,

    // Metadata
    pub langid: Option<LangId>,
    pub is_translation: bool,
    pub epoch: Option<Epoch>,
    pub options: Option<String>,
    pub shorthand: Option<String>,

    // Internal tracking
    pub person_id: Option<i64>,
    pub has_fulltext: bool,
    pub fulltext_path: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
