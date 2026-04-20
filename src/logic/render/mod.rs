//! Bibliography HTML renderer — pure functions, no I/O.
//!
//! Renders `BibItem` domain objects into structured HTML with `data-field` attributes.
//! Hardcoded to Dialectica's house style (Chicago author-date variant).
//!
//! # Public API
//!
//! - [`render_bibitem`] — render a single entry to HTML
//! - [`render_bibliography`] — render a sorted list with consecutive-author suppression

pub mod components;
pub mod entry_types;

use std::collections::HashMap;

use crate::domain::junctions::BibitemAuthorsRow;
use crate::domain::{Author, AuthorRole, BibItem, EntryType};

// =============================================================================
// AuthorName — lightweight name struct for the renderer
// =============================================================================

/// An author name for rendering purposes.
///
/// Uses unicode name fields. For mononyms, only `mononym` is set.
/// For regular names, `family` and `given` are set.
/// If `variant_unicode` is set, it overrides everything (used when the
/// bibitem references the author by an alternative name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorName {
    pub family: Option<String>,
    pub given: Option<String>,
    pub mononym: Option<String>,
    pub variant_unicode: Option<String>,
}

// =============================================================================
// RenderContext — pre-resolved data the renderer needs
// =============================================================================

/// Pre-resolved data that the renderer needs but cannot fetch itself (no I/O).
///
/// Built by the adapter layer from database lookups.
#[derive(Debug, Clone, Default)]
pub struct RenderContext {
    /// Authors linked to the bibitem, ordered by position.
    pub authors: Vec<AuthorName>,
    /// Editors linked to the bibitem, ordered by position.
    pub editors: Vec<AuthorName>,
    /// Guest editors linked to the bibitem, ordered by position.
    pub guesteditors: Vec<AuthorName>,
    /// Pre-resolved journal name (unicode).
    pub journal_name: Option<String>,
    /// Pre-resolved publisher name (unicode).
    pub publisher_name: Option<String>,
    /// Pre-resolved series name (unicode).
    pub series_name: Option<String>,
    /// Pre-resolved institution name (unicode).
    pub institution_name: Option<String>,
    /// Pre-resolved school name (unicode).
    pub school_name: Option<String>,
    /// Crossref bibkey (pre-resolved).
    pub crossref_bibkey: Option<String>,
    /// If true, the author is replaced with an em-dash (consecutive same-author).
    pub suppress_author: bool,
}

impl RenderContext {
    /// Return a copy with `suppress_author` set to true.
    pub fn with_suppress_author(&self) -> Self {
        let mut copy = self.clone();
        copy.suppress_author = true;
        copy
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Render a single `BibItem` to structured HTML.
///
/// The result is a `<div class="csl-entry">` with `data-type` and `data-bibkey` attributes.
pub fn render_bibitem(item: &BibItem, ctx: &RenderContext) -> String {
    let inner = match item.entry_type {
        EntryType::Article => entry_types::render_article(item, ctx),
        EntryType::Book => entry_types::render_book(item, ctx),
        EntryType::Incollection | EntryType::Inproceedings => {
            entry_types::render_chapter(item, ctx)
        }
        EntryType::Mastersthesis | EntryType::Phdthesis => entry_types::render_thesis(item, ctx),
        EntryType::Unpublished => entry_types::render_unpublished(item, ctx),
        _ => entry_types::render_generic(item, ctx),
    };

    let entry_type_str = item.entry_type.to_string();
    let bibkey = components::esc(&item.bibkey);

    format!(
        "<div class=\"csl-entry\" data-type=\"{entry_type_str}\" data-bibkey=\"{bibkey}\">{inner}</div>"
    )
}

/// Render a sorted bibliography with consecutive-author em-dash suppression.
///
/// Items should be pre-sorted by author family name, then year, then bibkey.
/// When consecutive entries have the same author(s), the author is replaced with em-dash.
pub fn render_bibliography(items: &[(BibItem, RenderContext)]) -> String {
    let mut rendered = Vec::with_capacity(items.len());
    let mut prev_author_key: Option<String> = None;

    for (item, ctx) in items {
        let current_key = author_sort_key(&ctx.authors);
        let suppress = prev_author_key.as_ref() == Some(&current_key) && !current_key.is_empty();
        let effective_ctx = if suppress {
            ctx.with_suppress_author()
        } else {
            ctx.clone()
        };
        rendered.push(render_bibitem(item, &effective_ctx));
        prev_author_key = Some(current_key);
    }

    rendered.join("\n")
}

/// Compute a sort key from the author list for comparison.
///
/// Used to detect consecutive same-author entries for em-dash suppression.
pub fn author_sort_key(authors: &[AuthorName]) -> String {
    authors
        .iter()
        .map(|a| {
            if let Some(ref variant) = a.variant_unicode {
                variant.to_lowercase()
            } else if let Some(ref mononym) = a.mononym {
                mononym.to_lowercase()
            } else {
                let family = a.family.as_deref().unwrap_or("").to_lowercase();
                let given = a.given.as_deref().unwrap_or("").to_lowercase();
                format!("{family},{given}")
            }
        })
        .collect::<Vec<_>>()
        .join("|")
}

// =============================================================================
// Helpers
// =============================================================================

/// Extract AuthorName list for a specific role from junction rows.
pub fn extract_role_authors(
    bib_authors: Option<&Vec<&BibitemAuthorsRow>>,
    role: AuthorRole,
    authors_map: &HashMap<i64, Author>,
) -> Vec<AuthorName> {
    let role_str = role.to_string();
    bib_authors
        .map(|rows| {
            let mut filtered: Vec<&BibitemAuthorsRow> = rows
                .iter()
                .filter(|r| r.role == role_str)
                .copied()
                .collect();
            filtered.sort_by_key(|r| r.position);
            filtered
                .iter()
                .filter_map(|r| {
                    authors_map.get(&r.author_id).map(|a| AuthorName {
                        family: a.family_name_unicode.clone(),
                        given: a.given_name_unicode.clone(),
                        mononym: a.mononym_unicode.clone(),
                        variant_unicode: r.name_variant_unicode.clone(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EntryType;

    /// Helper: create a minimal article BibItem for testing.
    fn make_article(bibkey: &str, title: &str, year: Option<i16>) -> BibItem {
        make_bibitem(EntryType::Article, bibkey, title, year)
    }

    /// Helper: create a minimal BibItem for testing.
    fn make_bibitem(
        entry_type: EntryType,
        bibkey: &str,
        title: &str,
        year: Option<i16>,
    ) -> BibItem {
        use chrono::Utc;
        BibItem {
            id: 1,
            bibkey: bibkey.to_string(),
            entry_type,
            date_year: year,
            date_year_2_hyphen: None,
            date_year_2_slash: None,
            date_month: None,
            date_day: None,
            date_is_no_date: year.is_none(),
            pubstate: None,
            title_latex: title.to_string(),
            title_unicode: Some(title.to_string()),
            booktitle_latex: None,
            booktitle_unicode: None,
            journal_id: None,
            publisher_id: None,
            address: None,
            volume: None,
            number: None,
            pages: None,
            eid: None,
            series_id: None,
            edition: None,
            institution_id: None,
            school_id: None,
            type_field: None,
            doi: None,
            url: None,
            eprint: None,
            urn: None,
            crossref_id: None,
            issuetitle_latex: None,
            issuetitle_unicode: None,
            note_latex: None,
            note_unicode: None,
            extra_note_latex: None,
            extra_note_unicode: None,
            langid: None,
            is_translation: false,
            epoch: None,
            options: None,
            shorthand: None,
            person_id: None,
            has_fulltext: false,
            fulltext_path: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Helper: create an AuthorName with family + given.
    fn make_author(given: &str, family: &str) -> AuthorName {
        AuthorName {
            family: Some(family.to_string()),
            given: Some(given.to_string()),
            mononym: None,
            variant_unicode: None,
        }
    }

    /// Helper: create an AuthorName with a mononym.
    fn make_mononym(name: &str) -> AuthorName {
        AuthorName {
            family: None,
            given: None,
            mononym: Some(name.to_string()),
            variant_unicode: None,
        }
    }

    // =========================================================================
    // Test: basic article rendering
    // =========================================================================

    #[test]
    fn test_render_article() {
        let mut item = make_article("smith:2024", "Some Title", Some(2024));
        item.volume = Some("78".to_string());
        item.number = Some("1".to_string());
        item.pages = Some("1-25".to_string());
        item.doi = Some("10.48106/dial.v78.i1.01".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Smith")],
            journal_name: Some("Dialectica".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-field=\"author\""),
            "should have author field"
        );
        assert!(
            html.contains("class=\"smallcaps\">Smith</span>"),
            "family name in smallcaps"
        );
        assert!(
            html.contains("data-field=\"given\">Jane</span>"),
            "given name present"
        );
        assert!(
            html.contains("data-field=\"date\">2024</span>"),
            "year present"
        );
        assert!(
            html.contains("\u{201c}Some Title\u{201d}"),
            "title in quotes"
        );
        assert!(html.contains("<em>Dialectica</em>"), "journal in italics");
        assert!(html.contains("data-field=\"volume\">78</span>"), "volume");
        assert!(html.contains("data-field=\"number\">1</span>"), "number");
        assert!(
            html.contains("data-field=\"pages\">1\u{2013}25</span>"),
            "pages with en-dash"
        );
        assert!(
            html.contains("doi:<a href=\"https://doi.org/10.48106/dial.v78.i1.01\">"),
            "DOI link"
        );
        assert!(html.contains("data-type=\"article\""), "entry type");
        assert!(html.contains("data-bibkey=\"smith:2024\""), "bibkey");
    }

    // =========================================================================
    // Test: book with author
    // =========================================================================

    #[test]
    fn test_render_book() {
        let mut item = make_bibitem(
            EntryType::Book,
            "kant:1781",
            "Critique of Pure Reason",
            Some(1781),
        );
        item.address = Some("Riga".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Immanuel", "Kant")],
            publisher_name: Some("Johann Friedrich Hartknoch".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(html.contains("data-type=\"book\""), "entry type book");
        assert!(
            html.contains("class=\"smallcaps\">Kant</span>"),
            "author family"
        );
        assert!(
            html.contains("<em>Critique of Pure Reason</em>"),
            "title in italics"
        );
        assert!(html.contains("Riga: "), "address present");
        assert!(
            html.contains("data-field=\"publisher\">Johann Friedrich Hartknoch</span>"),
            "publisher"
        );
    }

    // =========================================================================
    // Test: book edited (no author)
    // =========================================================================

    #[test]
    fn test_render_book_edited() {
        let mut item = make_bibitem(
            EntryType::Book,
            "wellman-frey:2003",
            "A Companion to Applied Ethics",
            Some(2003),
        );
        item.address = Some("Oxford".to_string());

        let ctx = RenderContext {
            editors: vec![
                make_author("R. G.", "Wellman"),
                make_author("Christopher Heath", "Frey"),
            ],
            publisher_name: Some("Blackwell".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-field=\"editor\""),
            "editor field present"
        );
        assert!(html.contains(", eds."), "plural editors suffix");
        assert!(!html.contains("data-field=\"author\""), "no author field");
    }

    // =========================================================================
    // Test: incollection
    // =========================================================================

    #[test]
    fn test_render_incollection() {
        let mut item = make_bibitem(
            EntryType::Incollection,
            "doe:2020ch",
            "A Chapter Title",
            Some(2020),
        );
        item.booktitle_unicode = Some("The Big Book".to_string());
        item.pages = Some("100-120".to_string());
        item.address = Some("New York".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("John", "Doe")],
            editors: vec![make_author("Jane", "Editor")],
            publisher_name: Some("Academic Press".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(html.contains("data-type=\"incollection\""), "entry type");
        assert!(
            html.contains("\u{201c}A Chapter Title\u{201d}"),
            "quoted title"
        );
        assert!(
            html.contains("In <span data-field=\"booktitle\"><em>The Big Book</em></span>"),
            "booktitle"
        );
        assert!(html.contains("edited by Jane Editor"), "editor inline");
        assert!(html.contains("pp. "), "pages prefix");
        assert!(html.contains("100\u{2013}120"), "pages with en-dash");
    }

    // =========================================================================
    // Test: thesis
    // =========================================================================

    #[test]
    fn test_render_thesis() {
        let item = make_bibitem(
            EntryType::Phdthesis,
            "student:2023",
            "My Dissertation",
            Some(2023),
        );

        let ctx = RenderContext {
            authors: vec![make_author("Alice", "Student")],
            school_name: Some("University of Somewhere".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(html.contains("data-type=\"phdthesis\""), "entry type");
        assert!(
            html.contains("\u{201c}My Dissertation\u{201d}"),
            "quoted title"
        );
        assert!(html.contains("PhD thesis"), "thesis type");
        assert!(html.contains("University of Somewhere"), "school name");
    }

    // =========================================================================
    // Test: unpublished
    // =========================================================================

    #[test]
    fn test_render_unpublished() {
        let mut item = make_bibitem(
            EntryType::Unpublished,
            "draft:2024",
            "Work in Progress",
            Some(2024),
        );
        item.note_unicode = Some("Manuscript in preparation".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Bob", "Writer")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(html.contains("data-type=\"unpublished\""), "entry type");
        assert!(
            html.contains("\u{201c}Work in Progress\u{201d}"),
            "quoted title"
        );
        assert!(html.contains("Manuscript in preparation"), "note present");
    }

    // =========================================================================
    // Test: mononym author
    // =========================================================================

    #[test]
    fn test_render_mononym() {
        let item = make_article("aristotle:meta", "Metaphysics", Some(-350));

        let ctx = RenderContext {
            authors: vec![make_mononym("Aristotle")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-field=\"author-name\">Aristotle</span>"),
            "mononym rendered"
        );
        assert!(
            !html.contains("class=\"smallcaps\""),
            "no smallcaps for mononym"
        );
    }

    // =========================================================================
    // Test: no date
    // =========================================================================

    #[test]
    fn test_render_no_date() {
        let item = make_article("anon:nd", "Unknown Work", None);

        let ctx = RenderContext {
            authors: vec![make_author("Anon", "Author")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-field=\"date\">n.d.</span>"),
            "n.d. for no date"
        );
    }

    // =========================================================================
    // Test: 11+ authors (truncation)
    // =========================================================================

    #[test]
    fn test_render_11_authors() {
        let item = make_article("many:2024", "Collaborative Work", Some(2024));

        let authors: Vec<AuthorName> = (1..=12)
            .map(|i| make_author(&format!("Author{i}"), &format!("Family{i}")))
            .collect();

        let ctx = RenderContext {
            authors,
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(html.contains("et al."), "et al. for 11+ authors");
        // Should contain first 7 authors
        assert!(html.contains("Family1"), "first author present");
        assert!(html.contains("Family7"), "seventh author present");
        // Should NOT contain 8th+ authors
        assert!(!html.contains("Family8"), "eighth author omitted");
        assert!(!html.contains("Family12"), "twelfth author omitted");
    }

    // =========================================================================
    // Test: DOI link rendering
    // =========================================================================

    #[test]
    fn test_render_doi_link() {
        let mut item = make_article("test:doi", "Test", Some(2024));
        item.doi = Some("10.1234/test".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("doi:<a href=\"https://doi.org/10.1234/test\">10.1234/test</a>"),
            "DOI rendered as link"
        );
    }

    // =========================================================================
    // Test: URL fallback (no DOI)
    // =========================================================================

    #[test]
    fn test_render_url_fallback() {
        let mut item = make_article("test:url", "Test", Some(2024));
        item.url = Some("https://example.com/paper".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("<a href=\"https://example.com/paper\">https://example.com/paper</a>"),
            "URL rendered as link"
        );
        assert!(!html.contains("doi:"), "no DOI prefix");
    }

    // =========================================================================
    // Test: consecutive author suppression (em-dash)
    // =========================================================================

    #[test]
    fn test_consecutive_author_suppression() {
        let item1 = make_article("smith:2020", "First Paper", Some(2020));
        let item2 = make_article("smith:2021", "Second Paper", Some(2021));

        let ctx1 = RenderContext {
            authors: vec![make_author("Jane", "Smith")],
            ..Default::default()
        };
        let ctx2 = RenderContext {
            authors: vec![make_author("Jane", "Smith")],
            ..Default::default()
        };

        let html = render_bibliography(&[(item1, ctx1), (item2, ctx2)]);

        let lines: Vec<&str> = html.lines().collect();
        assert_eq!(lines.len(), 2, "two entries");
        // First entry should have the full author
        assert!(lines[0].contains("Smith"), "first has author");
        // Second entry should have em-dash
        assert!(lines[1].contains("\u{2014}"), "second has em-dash");
    }

    // =========================================================================
    // Test: HTML escaping
    // =========================================================================

    #[test]
    fn test_html_escaping() {
        let item = make_article(
            "escape:test",
            "Title with <b>HTML</b> & \"quotes\"",
            Some(2024),
        );

        let ctx = RenderContext {
            authors: vec![make_author("A&B", "O'Test")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        // Check that HTML entities are escaped
        assert!(
            html.contains("&lt;b&gt;HTML&lt;/b&gt;"),
            "HTML tags escaped in title"
        );
        assert!(html.contains("&amp;"), "ampersand escaped");
        assert!(
            html.contains("&quot;quotes&quot;"),
            "quotes escaped in title"
        );
        assert!(html.contains("A&amp;B"), "ampersand in author escaped");
    }
}
