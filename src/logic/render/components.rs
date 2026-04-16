//! Reusable rendering components for bibliography HTML output.
//!
//! Pure functions that format individual fields (authors, dates, titles, etc.)
//! into HTML strings with `data-field` semantic markup.

use crate::domain::{AuthorRole, BibItem};

use super::AuthorName;

// =============================================================================
// HTML escaping
// =============================================================================

/// Escape special HTML characters in text content.
pub fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// =============================================================================
// Author / Editor rendering
// =============================================================================

/// Render a list of authors with `data-field` markup.
///
/// - Family names are wrapped in `<span class="smallcaps">`.
/// - 11+ authors: first 7 then "et al."
/// - If `suppress` is true, renders em-dash instead (for consecutive same-author entries).
pub fn render_authors(authors: &[AuthorName], role: AuthorRole, suppress: bool) -> String {
    let role_str = role.to_string();
    if suppress {
        return format!("<span data-field=\"{role_str}\">\u{2014}</span>");
    }
    if authors.is_empty() {
        return String::new();
    }

    let truncated = authors.len() >= 11;
    let display_authors = if truncated { &authors[..7] } else { authors };

    let parts: Vec<String> = display_authors
        .iter()
        .map(|a| {
            if let Some(ref variant) = a.variant_unicode {
                format!(
                    "<span data-field=\"author-name\">{}</span>",
                    esc(variant)
                )
            } else if let Some(ref mononym) = a.mononym {
                format!(
                    "<span data-field=\"author-name\">{}</span>",
                    esc(mononym)
                )
            } else {
                let family = a.family.as_deref().unwrap_or("");
                let given = a.given.as_deref().unwrap_or("");
                if given.is_empty() {
                    format!(
                        "<span data-field=\"family\" class=\"smallcaps\">{}</span>",
                        esc(family)
                    )
                } else {
                    format!(
                        "<span data-field=\"family\" class=\"smallcaps\">{}</span>, <span data-field=\"given\">{}</span>",
                        esc(family),
                        esc(given)
                    )
                }
            }
        })
        .collect();

    let mut joined = join_names(&parts);
    if truncated {
        joined.push_str(" et al.");
    }

    let mut result = format!("<span data-field=\"{role_str}\">{joined}</span>");

    if role == AuthorRole::Editor {
        if authors.len() == 1 {
            result.push_str(", ed.");
        } else {
            result.push_str(", eds.");
        }
    }

    result
}

/// Join name spans with commas and "and" before the last.
fn join_names(parts: &[String]) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        _ => format!(
            "{} and {}",
            parts[..parts.len() - 1].join(", "),
            parts[parts.len() - 1]
        ),
    }
}

// =============================================================================
// Date rendering
// =============================================================================

/// Render the year as `<span data-field="date">YEAR</span>`.
///
/// Returns "n.d." if no year is present.
pub fn render_date(item: &BibItem) -> String {
    let year_str = match item.date_year {
        Some(y) => y.to_string(),
        None => "n.d.".to_string(),
    };
    format!("<span data-field=\"date\">{year_str}</span>")
}

// =============================================================================
// Title rendering
// =============================================================================

/// Render the title with appropriate formatting.
///
/// - `quoted = true`: wraps in quotation marks `"Title"` (articles, chapters, unpublished)
/// - `quoted = false`: wraps in `<em>Title</em>` (books, theses)
pub fn render_title(title_unicode: &str, quoted: bool) -> String {
    let escaped = esc(title_unicode);
    let formatted = if quoted {
        format!("\u{201c}{escaped}\u{201d}")
    } else {
        format!("<em>{escaped}</em>")
    };
    format!("<span data-field=\"title\">{formatted}</span>")
}

// =============================================================================
// Pages rendering
// =============================================================================

/// Render pages field.
///
/// The `pages` field is a raw string like "1-25" or "1--25".
/// We normalize separators to en-dash.
pub fn render_pages(pages: &str) -> String {
    if pages.is_empty() {
        return String::new();
    }
    // Normalize various dash styles to en-dash
    let normalized = pages.replace("--", "\u{2013}").replace('-', "\u{2013}");
    format!("<span data-field=\"pages\">{}</span>", esc(&normalized))
}

// =============================================================================
// DOI / URL rendering
// =============================================================================

/// Render DOI as a clickable link, or fall back to URL.
///
/// - DOI: `doi:<a href="https://doi.org/{DOI}">{DOI}</a>`
/// - URL (no DOI): `<a href="{URL}">{URL}</a>`
/// - Neither: empty string
pub fn render_access(doi: Option<&str>, url: Option<&str>) -> String {
    if let Some(doi) = doi
        && !doi.is_empty()
    {
        return format!(
            "<span data-field=\"doi\">doi:<a href=\"https://doi.org/{}\">{}</a></span>",
            esc(doi),
            esc(doi)
        );
    }
    if let Some(url) = url
        && !url.is_empty()
    {
        return format!(
            "<span data-field=\"url\"><a href=\"{}\">{}</a></span>",
            esc(url),
            esc(url)
        );
    }
    String::new()
}

// =============================================================================
// Publisher rendering
// =============================================================================

/// Render publisher with optional address: `ADDRESS: PUBLISHER`.
pub fn render_publisher(address: Option<&str>, publisher_name: Option<&str>) -> String {
    match (
        address.filter(|a| !a.is_empty()),
        publisher_name.filter(|p| !p.is_empty()),
    ) {
        (Some(addr), Some(pub_name)) => format!(
            "{}: <span data-field=\"publisher\">{}</span>",
            esc(addr),
            esc(pub_name)
        ),
        (None, Some(pub_name)) => {
            format!("<span data-field=\"publisher\">{}</span>", esc(pub_name))
        }
        (Some(addr), None) => esc(addr),
        (None, None) => String::new(),
    }
}

// =============================================================================
// Journal rendering
// =============================================================================

/// Render journal name in italics: `<em>JOURNAL</em>`.
pub fn render_journal(journal_name: Option<&str>) -> String {
    match journal_name.filter(|j| !j.is_empty()) {
        Some(name) => format!("<span data-field=\"journal\"><em>{}</em></span>", esc(name)),
        None => String::new(),
    }
}

// =============================================================================
// Volume / Number rendering
// =============================================================================

/// Render volume and number: `VOLUME(NUMBER)`.
pub fn render_volume_number(volume: Option<&str>, number: Option<&str>) -> String {
    let vol = volume.filter(|v| !v.is_empty());
    let num = number.filter(|n| !n.is_empty());

    match (vol, num) {
        (Some(v), Some(n)) => format!(
            "<span data-field=\"volume\">{}</span>(<span data-field=\"number\">{}</span>)",
            esc(v),
            esc(n)
        ),
        (Some(v), None) => format!("<span data-field=\"volume\">{}</span>", esc(v)),
        (None, Some(n)) => format!("(<span data-field=\"number\">{}</span>)", esc(n)),
        (None, None) => String::new(),
    }
}

// =============================================================================
// Edition rendering
// =============================================================================

/// Render edition as ordinal: `2` -> `2nd ed.`
pub fn render_edition(edition: Option<&str>) -> String {
    match edition.filter(|e| !e.is_empty()) {
        Some(ed) => {
            // Try to parse as number for ordinal formatting
            if let Ok(n) = ed.parse::<u32>() {
                let ordinal = ordinal_suffix(n);
                format!("{n}{ordinal} ed.")
            } else {
                format!("{ed}.")
            }
        }
        None => String::new(),
    }
}

/// Get the ordinal suffix for a number.
fn ordinal_suffix(n: u32) -> &'static str {
    match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    }
}

// =============================================================================
// Note rendering
// =============================================================================

/// Render a note field.
pub fn render_note(note_unicode: Option<&str>) -> String {
    match note_unicode.filter(|n| !n.is_empty()) {
        Some(note) => esc(note),
        None => String::new(),
    }
}

// =============================================================================
// Series rendering
// =============================================================================

/// Render a series name.
pub fn render_series(series_name: Option<&str>) -> String {
    match series_name.filter(|s| !s.is_empty()) {
        Some(name) => format!("<span data-field=\"series\">{}</span>", esc(name)),
        None => String::new(),
    }
}

// =============================================================================
// EID rendering
// =============================================================================

/// Render an electronic identifier (used when no pages are present).
pub fn render_eid(eid: Option<&str>) -> String {
    match eid.filter(|e| !e.is_empty()) {
        Some(e) => format!("<span data-field=\"eid\">{}</span>", esc(e)),
        None => String::new(),
    }
}

// =============================================================================
// Booktitle rendering
// =============================================================================

/// Render a booktitle (for incollection/inproceedings) in italics.
pub fn render_booktitle(booktitle_unicode: Option<&str>) -> String {
    match booktitle_unicode.filter(|b| !b.is_empty()) {
        Some(bt) => format!("<span data-field=\"booktitle\"><em>{}</em></span>", esc(bt)),
        None => String::new(),
    }
}
