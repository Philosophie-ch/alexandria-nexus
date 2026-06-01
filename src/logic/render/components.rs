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
        let keys: String = authors
            .iter()
            .map(|a| esc(&a.author_key))
            .collect::<Vec<_>>()
            .join(",");
        return format!(
            "<span data-field=\"{role_str}\" data-author-key=\"{keys}\">\u{2014}</span>"
        );
    }
    if authors.is_empty() {
        return String::new();
    }

    let truncated = authors.len() >= 11;
    let display_authors = if truncated { &authors[..7] } else { authors };

    let parts: Vec<String> = display_authors
        .iter()
        .map(|a| {
            let key = esc(&a.author_key);
            if let Some(ref variant) = a.variant_unicode {
                format!(
                    "<span data-field=\"author-name\" data-author-key=\"{key}\">{}</span>",
                    esc(variant)
                )
            } else if let Some(ref mononym) = a.mononym {
                format!(
                    "<span data-field=\"author-name\" data-author-key=\"{key}\">{}</span>",
                    esc(mononym)
                )
            } else {
                let family = a.family.as_deref().unwrap_or("");
                let given = a.given.as_deref().unwrap_or("");
                if given.is_empty() {
                    format!(
                        "<span data-field=\"family\" class=\"smallcaps\" data-author-key=\"{key}\">{}</span>",
                        esc(family)
                    )
                } else {
                    format!(
                        "<span data-field=\"family\" class=\"smallcaps\" data-author-key=\"{key}\">{}</span>, <span data-field=\"given\">{}</span>",
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

/// Returns "pp." for a page range (contains a dash), "p." for a single page.
pub fn pages_prefix(pages: &str) -> &'static str {
    if pages.contains('-') || pages.contains('\u{2013}') {
        "pp."
    } else {
        "p."
    }
}

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
pub fn render_publisher(
    address: Option<&str>,
    publisher_name: Option<&str>,
    publisher_key: Option<&str>,
) -> String {
    let key_attr = match publisher_key {
        Some(k) => format!(" data-publisher-key=\"{}\"", esc(k)),
        None => String::new(),
    };
    match (
        address.filter(|a| !a.is_empty()),
        publisher_name.filter(|p| !p.is_empty()),
    ) {
        (Some(addr), Some(pub_name)) => format!(
            "{}: <span data-field=\"publisher\"{key_attr}>{}</span>",
            esc(addr),
            esc(pub_name)
        ),
        (None, Some(pub_name)) => {
            format!(
                "<span data-field=\"publisher\"{key_attr}>{}</span>",
                esc(pub_name)
            )
        }
        (Some(addr), None) => esc(addr),
        (None, None) => String::new(),
    }
}

// =============================================================================
// Journal rendering
// =============================================================================

/// Render journal name in italics: `<em>JOURNAL</em>`.
pub fn render_journal(journal_name: Option<&str>, journal_key: Option<&str>) -> String {
    match journal_name.filter(|j| !j.is_empty()) {
        Some(name) => {
            let key_attr = match journal_key {
                Some(k) => format!(" data-journal-key=\"{}\"", esc(k)),
                None => String::new(),
            };
            format!(
                "<span data-field=\"journal\"><em{key_attr}>{}</em></span>",
                esc(name)
            )
        }
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

/// Render a series name with optional series number.
///
/// With number: `<span data-field="series">Name</span> n. <span data-field="number">10</span>`
/// Without: `<span data-field="series">Name</span>`
pub fn render_series(
    series_name: Option<&str>,
    series_key: Option<&str>,
    series_number: Option<&str>,
) -> String {
    match series_name.filter(|s| !s.is_empty()) {
        Some(name) => {
            let key_attr = match series_key {
                Some(k) => format!(" data-series-key=\"{}\"", esc(k)),
                None => String::new(),
            };
            let name_span = format!("<span data-field=\"series\"{key_attr}>{}</span>", esc(name));
            match series_number.filter(|n| !n.is_empty()) {
                Some(n) => format!(
                    "{name_span} n.\u{00a0}<span data-field=\"number\">{}</span>",
                    esc(n)
                ),
                None => name_span,
            }
        }
        None => String::new(),
    }
}

// =============================================================================
// School rendering
// =============================================================================

/// Render a school name.
pub fn render_school(school_name: Option<&str>, school_key: Option<&str>) -> String {
    match school_name.filter(|s| !s.is_empty()) {
        Some(name) => {
            let key_attr = match school_key {
                Some(k) => format!(" data-school-key=\"{}\"", esc(k)),
                None => String::new(),
            };
            format!("<span data-field=\"school\"{key_attr}>{}</span>", esc(name))
        }
        None => String::new(),
    }
}

// =============================================================================
// Institution rendering
// =============================================================================

/// Render an institution name.
pub fn render_institution(institution_name: Option<&str>, institution_key: Option<&str>) -> String {
    match institution_name.filter(|s| !s.is_empty()) {
        Some(name) => {
            let key_attr = match institution_key {
                Some(k) => format!(" data-institution-key=\"{}\"", esc(k)),
                None => String::new(),
            };
            format!(
                "<span data-field=\"institution\"{key_attr}>{}</span>",
                esc(name)
            )
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pages_prefix_range() {
        // Single-hyphen ranges (e.g. "57--72", "100-120" in corpus)
        assert_eq!(pages_prefix("1-25"), "pp.");
        assert_eq!(pages_prefix("57-72"), "pp.");
        assert_eq!(pages_prefix("129-144"), "pp.");
        // Double-hyphen ranges (most common form in corpus: "1--14", "3--18")
        assert_eq!(pages_prefix("1--14"), "pp.");
        assert_eq!(pages_prefix("3--18"), "pp.");
        assert_eq!(pages_prefix("57--72"), "pp.");
        // Roman numeral ranges
        assert_eq!(pages_prefix("v-x"), "pp.");
        // Already-normalized en-dash
        assert_eq!(pages_prefix("1\u{2013}25"), "pp.");
    }

    #[test]
    fn test_pages_prefix_single_numeric() {
        // Plain page numbers seen in incollection entries (e.g. 289, 221, 1)
        assert_eq!(pages_prefix("1"), "p.");
        assert_eq!(pages_prefix("42"), "p.");
        assert_eq!(pages_prefix("289"), "p.");
    }

    #[test]
    fn test_pages_prefix_single_roman() {
        // Roman numeral intro pages seen in incollection entries (ix, vii, xi)
        assert_eq!(pages_prefix("ix"), "p.");
        assert_eq!(pages_prefix("vii"), "p.");
        assert_eq!(pages_prefix("xi"), "p.");
        assert_eq!(pages_prefix("v"), "p.");
    }

    // =========================================================================
    // render_journal
    // =========================================================================

    #[test]
    fn test_render_journal_with_key() {
        let html = render_journal(Some("Dialectica"), Some("dialectica"));
        assert!(html.contains("data-journal-key=\"dialectica\""));
        assert!(html.contains("<em data-journal-key=\"dialectica\">Dialectica</em>"));
    }

    #[test]
    fn test_render_journal_without_key() {
        let html = render_journal(Some("Dialectica"), None);
        assert!(!html.contains("data-journal-key"));
        assert!(html.contains("<em>Dialectica</em>"));
    }

    #[test]
    fn test_render_journal_key_escaped() {
        let html = render_journal(Some("J&J"), Some("j\"key"));
        assert!(html.contains("data-journal-key=\"j&quot;key\""));
        assert!(html.contains("J&amp;J"));
    }

    #[test]
    fn test_render_journal_empty_name() {
        assert!(render_journal(Some(""), Some("key")).is_empty());
        assert!(render_journal(None, Some("key")).is_empty());
    }

    // =========================================================================
    // render_publisher
    // =========================================================================

    #[test]
    fn test_render_publisher_with_key_and_address() {
        let html = render_publisher(Some("Oxford"), Some("OUP"), Some("oup"));
        assert!(html.contains("data-publisher-key=\"oup\""));
        assert!(html.contains(
            "Oxford: <span data-field=\"publisher\" data-publisher-key=\"oup\">OUP</span>"
        ));
    }

    #[test]
    fn test_render_publisher_with_key_no_address() {
        let html = render_publisher(None, Some("OUP"), Some("oup"));
        assert!(html.contains("data-publisher-key=\"oup\""));
        assert!(
            html.contains("<span data-field=\"publisher\" data-publisher-key=\"oup\">OUP</span>")
        );
    }

    #[test]
    fn test_render_publisher_without_key() {
        let html = render_publisher(Some("Oxford"), Some("OUP"), None);
        assert!(!html.contains("data-publisher-key"));
        assert!(html.contains("Oxford: <span data-field=\"publisher\">OUP</span>"));
    }

    #[test]
    fn test_render_publisher_key_escaped() {
        let html = render_publisher(None, Some("P&P"), Some("p\"k"));
        assert!(html.contains("data-publisher-key=\"p&quot;k\""));
        assert!(html.contains("P&amp;P"));
    }

    // =========================================================================
    // render_series
    // =========================================================================

    #[test]
    fn test_render_series_with_key() {
        let html = render_series(Some("Cambridge Studies"), Some("cambridge_studies"), None);
        assert!(html.contains("data-series-key=\"cambridge_studies\""));
        assert!(html.contains(
            "data-field=\"series\" data-series-key=\"cambridge_studies\">Cambridge Studies</span>"
        ));
    }

    #[test]
    fn test_render_series_without_key() {
        let html = render_series(Some("Cambridge Studies"), None, None);
        assert!(!html.contains("data-series-key"));
        assert!(html.contains("<span data-field=\"series\">Cambridge Studies</span>"));
    }

    #[test]
    fn test_render_series_key_escaped() {
        let html = render_series(Some("S&S"), Some("s<k"), None);
        assert!(html.contains("data-series-key=\"s&lt;k\""));
        assert!(html.contains("S&amp;S"));
    }

    #[test]
    fn test_render_series_empty_name() {
        assert!(render_series(Some(""), Some("key"), None).is_empty());
        assert!(render_series(None, Some("key"), None).is_empty());
    }

    #[test]
    fn test_render_series_with_number() {
        let html = render_series(
            Some("Contemporary Debates in Philosophy"),
            Some("cdp"),
            Some("10"),
        );
        assert!(
            html.contains("Contemporary Debates in Philosophy</span> n.\u{00a0}<span data-field=\"number\">10</span>"),
            "series name followed by number: {html}"
        );
    }

    #[test]
    fn test_render_series_number_without_name() {
        let html = render_series(None, None, Some("10"));
        assert!(html.is_empty(), "no series name means no output");
    }

    #[test]
    fn test_render_series_empty_number() {
        let html = render_series(Some("Oxford Handbooks"), Some("oh"), Some(""));
        assert!(
            !html.contains("n.\u{00a0}"),
            "empty number is not rendered: {html}"
        );
        assert!(html.contains("Oxford Handbooks"), "name still rendered");
    }

    // =========================================================================
    // render_school
    // =========================================================================

    #[test]
    fn test_render_school_with_key() {
        let html = render_school(Some("MIT"), Some("mit"));
        assert!(html.contains("data-school-key=\"mit\""));
        assert!(html.contains("data-field=\"school\" data-school-key=\"mit\">MIT</span>"));
    }

    #[test]
    fn test_render_school_without_key() {
        let html = render_school(Some("MIT"), None);
        assert!(!html.contains("data-school-key"));
        assert!(html.contains("<span data-field=\"school\">MIT</span>"));
    }

    #[test]
    fn test_render_school_key_escaped() {
        let html = render_school(Some("U&T"), Some("u\"t"));
        assert!(html.contains("data-school-key=\"u&quot;t\""));
        assert!(html.contains("U&amp;T"));
    }

    #[test]
    fn test_render_school_empty_name() {
        assert!(render_school(Some(""), Some("key")).is_empty());
        assert!(render_school(None, Some("key")).is_empty());
    }

    // =========================================================================
    // render_institution
    // =========================================================================

    #[test]
    fn test_render_institution_with_key() {
        let html = render_institution(Some("CERN"), Some("cern"));
        assert!(html.contains("data-institution-key=\"cern\""));
        assert!(
            html.contains("data-field=\"institution\" data-institution-key=\"cern\">CERN</span>")
        );
    }

    #[test]
    fn test_render_institution_without_key() {
        let html = render_institution(Some("CERN"), None);
        assert!(!html.contains("data-institution-key"));
        assert!(html.contains("<span data-field=\"institution\">CERN</span>"));
    }

    #[test]
    fn test_render_institution_key_escaped() {
        let html = render_institution(Some("I&I"), Some("i<k"));
        assert!(html.contains("data-institution-key=\"i&lt;k\""));
        assert!(html.contains("I&amp;I"));
    }

    #[test]
    fn test_render_institution_empty_name() {
        assert!(render_institution(Some(""), Some("key")).is_empty());
        assert!(render_institution(None, Some("key")).is_empty());
    }

    // =========================================================================
    // pages_prefix
    // =========================================================================

    #[test]
    fn test_pages_prefix_article_only_values() {
        // These patterns appear exclusively in article entries (prefix not applied there),
        // but the function should still return "p." since none contain a dash.
        // Electronic article IDs (e.g. e12560, e0173496)
        assert_eq!(pages_prefix("e12560"), "p.");
        assert_eq!(pages_prefix("e0173496"), "p.");
        // Supplement pages (S640, S87)
        assert_eq!(pages_prefix("S640"), "p.");
        assert_eq!(pages_prefix("S87"), "p.");
        // Variant page labels (59b)
        assert_eq!(pages_prefix("59b"), "p.");
        // Discontinuous pages stored as comma list (no dash → p.)
        assert_eq!(pages_prefix("248, 260"), "p.");
        assert_eq!(pages_prefix("39a, 39b"), "p.");
        // Dirty data seen in incollection (Mar 16, ad loc.)
        assert_eq!(pages_prefix("Mar 16"), "p.");
        assert_eq!(pages_prefix("ad loc."), "p.");
    }
}
