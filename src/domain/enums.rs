//! Domain enums mapping to PostgreSQL enum types.
//!
//! Each enum derives `sqlx::Type` for direct database mapping,
//! `Serialize`/`Deserialize` for JSON, and `ToSchema` for OpenAPI.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// =============================================================================
// EntryType
// =============================================================================

/// BibTeX entry type — matches the `entry_type` PostgreSQL enum.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, sqlx::Type, ToSchema,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "entry_type", rename_all = "lowercase")]
pub enum EntryType {
    Article,
    Book,
    Incollection,
    Inproceedings,
    Mastersthesis,
    Misc,
    Phdthesis,
    Proceedings,
    Techreport,
    Unpublished,
    #[default]
    #[serde(rename = "UNKNOWN")]
    #[sqlx(rename = "UNKNOWN")]
    Unknown,
}

impl fmt::Display for EntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Article => write!(f, "article"),
            Self::Book => write!(f, "book"),
            Self::Incollection => write!(f, "incollection"),
            Self::Inproceedings => write!(f, "inproceedings"),
            Self::Mastersthesis => write!(f, "mastersthesis"),
            Self::Misc => write!(f, "misc"),
            Self::Phdthesis => write!(f, "phdthesis"),
            Self::Proceedings => write!(f, "proceedings"),
            Self::Techreport => write!(f, "techreport"),
            Self::Unpublished => write!(f, "unpublished"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

impl FromStr for EntryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "article" => Ok(Self::Article),
            "book" => Ok(Self::Book),
            "incollection" => Ok(Self::Incollection),
            "inproceedings" => Ok(Self::Inproceedings),
            "mastersthesis" => Ok(Self::Mastersthesis),
            "misc" => Ok(Self::Misc),
            "phdthesis" => Ok(Self::Phdthesis),
            "proceedings" => Ok(Self::Proceedings),
            "techreport" => Ok(Self::Techreport),
            "unpublished" => Ok(Self::Unpublished),
            "UNKNOWN" => Ok(Self::Unknown),
            other => Err(format!("unknown entry type: {other}")),
        }
    }
}

// =============================================================================
// PubState
// =============================================================================

/// Publication state — matches the `pubstate` PostgreSQL enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "pubstate", rename_all = "lowercase")]
pub enum PubState {
    Unpub,
    Forthcoming,
    Inwork,
    Submitted,
    Published,
}

impl fmt::Display for PubState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unpub => write!(f, "unpub"),
            Self::Forthcoming => write!(f, "forthcoming"),
            Self::Inwork => write!(f, "inwork"),
            Self::Submitted => write!(f, "submitted"),
            Self::Published => write!(f, "published"),
        }
    }
}

impl FromStr for PubState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "unpub" => Ok(Self::Unpub),
            "forthcoming" => Ok(Self::Forthcoming),
            "inwork" => Ok(Self::Inwork),
            "submitted" => Ok(Self::Submitted),
            "published" => Ok(Self::Published),
            other => Err(format!("unknown pubstate: {other}")),
        }
    }
}

// =============================================================================
// LangId
// =============================================================================

/// Language identifier — matches the `langid` PostgreSQL enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "langid", rename_all = "lowercase")]
pub enum LangId {
    Catalan,
    Czech,
    Danish,
    Dutch,
    English,
    French,
    Greek,
    Italian,
    Latin,
    Lithuanian,
    Ngerman,
    Polish,
    Portuguese,
    Romanian,
    Russian,
    Slovak,
    Spanish,
    Swedish,
    Unknown,
}

impl fmt::Display for LangId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalan => write!(f, "catalan"),
            Self::Czech => write!(f, "czech"),
            Self::Danish => write!(f, "danish"),
            Self::Dutch => write!(f, "dutch"),
            Self::English => write!(f, "english"),
            Self::French => write!(f, "french"),
            Self::Greek => write!(f, "greek"),
            Self::Italian => write!(f, "italian"),
            Self::Latin => write!(f, "latin"),
            Self::Lithuanian => write!(f, "lithuanian"),
            Self::Ngerman => write!(f, "ngerman"),
            Self::Polish => write!(f, "polish"),
            Self::Portuguese => write!(f, "portuguese"),
            Self::Romanian => write!(f, "romanian"),
            Self::Russian => write!(f, "russian"),
            Self::Slovak => write!(f, "slovak"),
            Self::Spanish => write!(f, "spanish"),
            Self::Swedish => write!(f, "swedish"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl FromStr for LangId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "catalan" => Ok(Self::Catalan),
            "czech" => Ok(Self::Czech),
            "danish" => Ok(Self::Danish),
            "dutch" => Ok(Self::Dutch),
            "english" => Ok(Self::English),
            "french" => Ok(Self::French),
            "greek" => Ok(Self::Greek),
            "italian" => Ok(Self::Italian),
            "latin" => Ok(Self::Latin),
            "lithuanian" => Ok(Self::Lithuanian),
            "ngerman" => Ok(Self::Ngerman),
            "polish" => Ok(Self::Polish),
            "portuguese" => Ok(Self::Portuguese),
            "romanian" => Ok(Self::Romanian),
            "russian" => Ok(Self::Russian),
            "slovak" => Ok(Self::Slovak),
            "spanish" => Ok(Self::Spanish),
            "swedish" => Ok(Self::Swedish),
            "unknown" => Ok(Self::Unknown),
            other => Err(format!("unknown langid: {other}")),
        }
    }
}

// =============================================================================
// Epoch
// =============================================================================

/// Historical epoch — matches the `epoch` PostgreSQL enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "kebab-case")]
#[sqlx(type_name = "epoch", rename_all = "kebab-case")]
pub enum Epoch {
    AncientPhilosophy,
    AncientScientists,
    AustrianPhilosophy,
    BritishIdealism,
    Classics,
    Contemporaries,
    ContemporaryScientists,
    ContinentalPhilosophy,
    CriticalTheory,
    Cynics,
    Enlightenment,
    Existentialism,
    ExoticPhilosophy,
    GermanIdealism,
    GermanRationalism,
    GestaltPsychology,
    Hermeneutics,
    IslamicPhilosophy,
    Mathematicians,
    MedievalPhilosophy,
    ModernPhilosophy,
    ModernScientists,
    NeoKantianism,
    Neoplatonism,
    NewRealism,
    OrdinaryLanguagePhilosophy,
    Phenomenology,
    PolishLogic,
    Pragmatism,
    Presocratics,
    Renaissance,
    Stoics,
    Theologians,
    ViennaCircle,
}

impl fmt::Display for Epoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AncientPhilosophy => write!(f, "ancient-philosophy"),
            Self::AncientScientists => write!(f, "ancient-scientists"),
            Self::AustrianPhilosophy => write!(f, "austrian-philosophy"),
            Self::BritishIdealism => write!(f, "british-idealism"),
            Self::Classics => write!(f, "classics"),
            Self::Contemporaries => write!(f, "contemporaries"),
            Self::ContemporaryScientists => write!(f, "contemporary-scientists"),
            Self::ContinentalPhilosophy => write!(f, "continental-philosophy"),
            Self::CriticalTheory => write!(f, "critical-theory"),
            Self::Cynics => write!(f, "cynics"),
            Self::Enlightenment => write!(f, "enlightenment"),
            Self::Existentialism => write!(f, "existentialism"),
            Self::ExoticPhilosophy => write!(f, "exotic-philosophy"),
            Self::GermanIdealism => write!(f, "german-idealism"),
            Self::GermanRationalism => write!(f, "german-rationalism"),
            Self::GestaltPsychology => write!(f, "gestalt-psychology"),
            Self::Hermeneutics => write!(f, "hermeneutics"),
            Self::IslamicPhilosophy => write!(f, "islamic-philosophy"),
            Self::Mathematicians => write!(f, "mathematicians"),
            Self::MedievalPhilosophy => write!(f, "medieval-philosophy"),
            Self::ModernPhilosophy => write!(f, "modern-philosophy"),
            Self::ModernScientists => write!(f, "modern-scientists"),
            Self::NeoKantianism => write!(f, "neo-kantianism"),
            Self::Neoplatonism => write!(f, "neoplatonism"),
            Self::NewRealism => write!(f, "new-realism"),
            Self::OrdinaryLanguagePhilosophy => write!(f, "ordinary-language-philosophy"),
            Self::Phenomenology => write!(f, "phenomenology"),
            Self::PolishLogic => write!(f, "polish-logic"),
            Self::Pragmatism => write!(f, "pragmatism"),
            Self::Presocratics => write!(f, "presocratics"),
            Self::Renaissance => write!(f, "renaissance"),
            Self::Stoics => write!(f, "stoics"),
            Self::Theologians => write!(f, "theologians"),
            Self::ViennaCircle => write!(f, "vienna-circle"),
        }
    }
}

impl FromStr for Epoch {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ancient-philosophy" => Ok(Self::AncientPhilosophy),
            "ancient-scientists" => Ok(Self::AncientScientists),
            "austrian-philosophy" => Ok(Self::AustrianPhilosophy),
            "british-idealism" => Ok(Self::BritishIdealism),
            "classics" => Ok(Self::Classics),
            "contemporaries" => Ok(Self::Contemporaries),
            "contemporary-scientists" => Ok(Self::ContemporaryScientists),
            "continental-philosophy" => Ok(Self::ContinentalPhilosophy),
            "critical-theory" => Ok(Self::CriticalTheory),
            "cynics" => Ok(Self::Cynics),
            "enlightenment" => Ok(Self::Enlightenment),
            "existentialism" => Ok(Self::Existentialism),
            "exotic-philosophy" => Ok(Self::ExoticPhilosophy),
            "german-idealism" => Ok(Self::GermanIdealism),
            "german-rationalism" => Ok(Self::GermanRationalism),
            "gestalt-psychology" => Ok(Self::GestaltPsychology),
            "hermeneutics" => Ok(Self::Hermeneutics),
            "islamic-philosophy" => Ok(Self::IslamicPhilosophy),
            "mathematicians" => Ok(Self::Mathematicians),
            "medieval-philosophy" => Ok(Self::MedievalPhilosophy),
            "modern-philosophy" => Ok(Self::ModernPhilosophy),
            "modern-scientists" => Ok(Self::ModernScientists),
            "neo-kantianism" => Ok(Self::NeoKantianism),
            "neoplatonism" => Ok(Self::Neoplatonism),
            "new-realism" => Ok(Self::NewRealism),
            "ordinary-language-philosophy" => Ok(Self::OrdinaryLanguagePhilosophy),
            "phenomenology" => Ok(Self::Phenomenology),
            "polish-logic" => Ok(Self::PolishLogic),
            "pragmatism" => Ok(Self::Pragmatism),
            "presocratics" => Ok(Self::Presocratics),
            "renaissance" => Ok(Self::Renaissance),
            "stoics" => Ok(Self::Stoics),
            "theologians" => Ok(Self::Theologians),
            "vienna-circle" => Ok(Self::ViennaCircle),
            other => Err(format!("unknown epoch: {other}")),
        }
    }
}

// =============================================================================
// AuthorRole
// =============================================================================

/// Author role in a bibliography item — matches the `author_role` PostgreSQL enum.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, sqlx::Type, ToSchema,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "author_role", rename_all = "lowercase")]
pub enum AuthorRole {
    #[default]
    Author,
    Editor,
    Guesteditor,
}

impl fmt::Display for AuthorRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Author => write!(f, "author"),
            Self::Editor => write!(f, "editor"),
            Self::Guesteditor => write!(f, "guesteditor"),
        }
    }
}

impl FromStr for AuthorRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "author" => Ok(Self::Author),
            "editor" => Ok(Self::Editor),
            "guesteditor" => Ok(Self::Guesteditor),
            other => Err(format!("unknown author role: {other}")),
        }
    }
}

// =============================================================================
// RefType
// =============================================================================

/// Reference type between bibliography items — matches the `ref_type` PostgreSQL enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "ref_type", rename_all = "snake_case")]
pub enum RefType {
    FurtherRef,
    DependsOn,
}

impl fmt::Display for RefType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FurtherRef => write!(f, "further_ref"),
            Self::DependsOn => write!(f, "depends_on"),
        }
    }
}

impl FromStr for RefType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "further_ref" => Ok(Self::FurtherRef),
            "depends_on" => Ok(Self::DependsOn),
            other => Err(format!("unknown ref type: {other}")),
        }
    }
}

// =============================================================================
// PermissionLevel
// =============================================================================

/// API key permission level — matches the `permission_level` PostgreSQL enum.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, sqlx::Type, ToSchema,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "permission_level", rename_all = "lowercase")]
pub enum PermissionLevel {
    Public,
    #[default]
    Read,
    Write,
    Admin,
}

impl fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => write!(f, "public"),
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Admin => write!(f, "admin"),
        }
    }
}

impl FromStr for PermissionLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "public" => Ok(Self::Public),
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            "admin" => Ok(Self::Admin),
            other => Err(format!("unknown permission level: {other}")),
        }
    }
}
