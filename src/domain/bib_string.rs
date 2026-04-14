//! BibStringAttr — a pair of LaTeX and Unicode representations.
//!
//! Used for titles, names, and other text that may contain special characters.

use serde::{Deserialize, Serialize};

/// A string attribute with two representations: LaTeX and Unicode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BibStringAttr {
    pub latex: String,
    pub unicode: String,
}

impl BibStringAttr {
    pub fn new(latex: impl Into<String>, unicode: impl Into<String>) -> Self {
        Self {
            latex: latex.into(),
            unicode: unicode.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.latex.is_empty() && self.unicode.is_empty()
    }
}

impl From<&str> for BibStringAttr {
    fn from(s: &str) -> Self {
        Self {
            latex: s.to_string(),
            unicode: s.to_string(),
        }
    }
}

/// Normalize bib_string fields: if unicode is empty, copy from latex.
///
/// Called by generated pre-transform hooks on entity DTOs. Ensures the unicode
/// variant is populated when only latex is provided.
pub fn bib_string_normalize(latex: &Option<String>, unicode: &mut Option<String>) {
    if let Some(latex_val) = latex
        && unicode.as_ref().is_none_or(|s| s.is_empty())
    {
        *unicode = Some(latex_val.clone());
    }
}

/// Normalize required bib_string fields: if unicode is empty, copy from latex.
pub fn bib_string_normalize_required(latex: &str, unicode: &mut String) {
    if unicode.is_empty() {
        *unicode = latex.to_string();
    }
}

impl From<String> for BibStringAttr {
    fn from(s: String) -> Self {
        Self {
            latex: s.clone(),
            unicode: s,
        }
    }
}
