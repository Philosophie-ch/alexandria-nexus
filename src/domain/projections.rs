//! Projections — typed field subsets for list, search, and expansion views.

use hexforge::Projection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{
    Author, BibItem, EntryType, Institution, Journal, Keyword, PubState, Publisher, School, Series,
};

// =============================================================================
// BibItem projections
// =============================================================================

/// Summary projection for bibliography list endpoints.
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "BibItem")]
pub struct BibItemSummary {
    pub id: i64,
    pub bibkey: String,
    pub entry_type: EntryType,
    pub title_simplified: String,
    pub date_year: Option<i16>,
    pub pubstate: Option<PubState>,
}

// =============================================================================
// Expansion projections — lightweight versions for ?expand= responses
// =============================================================================

/// Author projection for expansion. Names only, no timestamps or shorthand.
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "Author")]
pub struct AuthorExpanded {
    pub id: i64,
    pub author_key: String,
    pub given_name_unicode: Option<String>,
    pub given_name_simplified: Option<String>,
    pub family_name_unicode: Option<String>,
    pub family_name_simplified: Option<String>,
    pub mononym_unicode: Option<String>,
    pub mononym_simplified: Option<String>,
}

/// Journal projection for expansion.
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "Journal")]
pub struct JournalExpanded {
    pub id: i64,
    pub journal_key: String,
    pub name_unicode: String,
    pub name_simplified: String,
}

/// Publisher projection for expansion.
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "Publisher")]
pub struct PublisherExpanded {
    pub id: i64,
    pub publisher_key: String,
    pub name_unicode: String,
    pub name_simplified: String,
}

/// Institution projection for expansion.
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "Institution")]
pub struct InstitutionExpanded {
    pub id: i64,
    pub institution_key: String,
    pub name_unicode: String,
    pub name_simplified: String,
}

/// School projection for expansion.
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "School")]
pub struct SchoolExpanded {
    pub id: i64,
    pub school_key: String,
    pub name_unicode: String,
    pub name_simplified: String,
}

/// Series projection for expansion.
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "Series")]
pub struct SeriesExpanded {
    pub id: i64,
    pub series_key: String,
    pub name_unicode: String,
    pub name_simplified: String,
}

/// Keyword projection for expansion.
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "Keyword")]
pub struct KeywordExpanded {
    pub id: i64,
    pub name: String,
    pub level: i16,
}

/// BibItem projection for crossref expansion (just identity + title).
#[derive(Projection, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[projection(entity = "BibItem")]
pub struct BibItemCrossref {
    pub id: i64,
    pub bibkey: String,
    pub entry_type: EntryType,
    pub title_simplified: String,
    pub date_year: Option<i16>,
}
