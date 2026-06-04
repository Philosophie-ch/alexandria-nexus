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
use crate::logic::latex_citations::{CitationData, substitute_citations_html};

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
    pub author_key: String,
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
/// Built by the process layer.
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
    pub journal_key: Option<String>,
    /// Pre-resolved publisher name (unicode).
    pub publisher_name: Option<String>,
    pub publisher_key: Option<String>,
    /// Pre-resolved series name (unicode).
    pub series_name: Option<String>,
    pub series_key: Option<String>,
    /// Series number (from the entry's or parent's `number` field for books/chapters).
    pub series_number: Option<String>,
    /// Pre-resolved institution name (unicode).
    pub institution_name: Option<String>,
    pub institution_key: Option<String>,
    /// Pre-resolved school name (unicode).
    pub school_name: Option<String>,
    pub school_key: Option<String>,
    /// Pre-resolved booktitle (unicode) for incollection/inproceedings.
    pub booktitle_unicode: Option<String>,
    /// Pre-resolved address (for publisher rendering).
    pub address: Option<String>,
    /// Crossref bibkey (pre-resolved).
    pub crossref_bibkey: Option<String>,
    /// If true, the author is replaced with an em-dash (consecutive same-author).
    pub suppress_author: bool,
    /// Year disambiguation suffix (e.g. "a", "b") for repeated author+year.
    pub year_suffix: Option<String>,
    /// Title with `\cite*{}` commands resolved using suffix-aware citation data.
    /// When present, the renderer uses this instead of `title_unicode`.
    pub resolved_title: Option<String>,
    /// Note with `\cite*{}` commands resolved using suffix-aware citation data.
    /// When present, the renderer uses this instead of `note_unicode`.
    pub resolved_note: Option<String>,
}

impl RenderContext {
    /// Return a copy with `suppress_author` set to true.
    pub fn with_suppress_author(&self) -> Self {
        let mut copy = self.clone();
        copy.suppress_author = true;
        copy
    }

    /// Effective title for rendering: resolved (suffix-aware) if available, else unicode, else latex.
    pub fn effective_title<'a>(&'a self, item: &'a BibItem) -> &'a str {
        self.resolved_title
            .as_deref()
            .or(item.title_unicode.as_deref())
            .unwrap_or(&item.title_latex)
    }

    /// Effective note for rendering: resolved (suffix-aware) if available, else unicode.
    pub fn effective_note<'a>(&'a self, item: &'a BibItem) -> Option<&'a str> {
        self.resolved_note
            .as_deref()
            .or(item.note_unicode.as_deref())
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
    authors_map: &HashMap<String, Author>,
) -> Vec<AuthorName> {
    bib_authors
        .map(|rows| {
            let mut filtered: Vec<&BibitemAuthorsRow> =
                rows.iter().filter(|r| r.role == role).copied().collect();
            filtered.sort_by_key(|r| r.position);
            filtered
                .iter()
                .filter_map(|r| {
                    authors_map.get(&r.author_key).map(|a| AuthorName {
                        author_key: r.author_key.clone(),
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
// Year suffix disambiguation
// =============================================================================

/// Parse the bibkey suffix: the alphabetic part after the year digits in `stem:year<suffix>`.
fn parse_bibkey_suffix(bibkey: &str) -> &str {
    let after_colon = bibkey.rsplit(':').next().unwrap_or("");
    let suffix_start = after_colon
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(after_colon.len());
    &after_colon[suffix_start..]
}

/// Assign year disambiguation suffixes across main and further-ref entries.
///
/// For each (author, year) group with more than one entry, assigns rendered
/// suffixes: the first entry gets no suffix, the second gets "a", third "b", etc.
/// Ordering: main refs first, then further refs; within each group, ordered by
/// bibkey suffix.
pub fn assign_year_suffixes(
    main: &mut [(BibItem, RenderContext)],
    further: &mut [(BibItem, RenderContext)],
) {
    use std::collections::HashMap;

    #[derive(PartialEq, Eq, Clone, Copy)]
    enum Group {
        Main,
        Further,
    }

    type SuffixEntry = (Group, String, usize);
    let mut groups: HashMap<(String, Option<i16>), Vec<SuffixEntry>> = HashMap::new();

    for (i, (bib, ctx)) in main.iter().enumerate() {
        let key = (author_sort_key(&ctx.authors), bib.date_year);
        let suffix = parse_bibkey_suffix(&bib.bibkey).to_string();
        groups
            .entry(key)
            .or_default()
            .push((Group::Main, suffix, i));
    }
    for (i, (bib, ctx)) in further.iter().enumerate() {
        let key = (author_sort_key(&ctx.authors), bib.date_year);
        let suffix = parse_bibkey_suffix(&bib.bibkey).to_string();
        groups
            .entry(key)
            .or_default()
            .push((Group::Further, suffix, i));
    }

    // Collect all assignments, then apply them
    let mut assignments: Vec<(Group, usize, Option<String>)> = Vec::new();

    for (_key, mut entries) in groups {
        if entries.len() <= 1 {
            continue;
        }

        // Sort: main before further, then by bibkey suffix alphabetically
        entries.sort_by(|a, b| {
            let group_ord = match (a.0, b.0) {
                (Group::Main, Group::Further) => std::cmp::Ordering::Less,
                (Group::Further, Group::Main) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };
            group_ord.then_with(|| a.1.cmp(&b.1))
        });

        for (pos, (group, _suffix, idx)) in entries.iter().enumerate() {
            let rendered_suffix = Some(suffix_letter(pos));
            assignments.push((*group, *idx, rendered_suffix));
        }
    }

    for (group, idx, rendered_suffix) in assignments {
        match group {
            Group::Main => main[idx].1.year_suffix = rendered_suffix,
            Group::Further => further[idx].1.year_suffix = rendered_suffix,
        }
    }
}

/// Convert a zero-based index to a letter suffix: 0→"a", 1→"b", ..., 25→"z", 26→"aa", etc.
fn suffix_letter(index: usize) -> String {
    if index < 26 {
        let c = b'a' + u8::try_from(index).unwrap_or(0);
        String::from(char::from(c))
    } else {
        let first = b'a' + u8::try_from((index / 26) - 1).unwrap_or(0);
        let second = b'a' + u8::try_from(index % 26).unwrap_or(0);
        format!("{}{}", char::from(first), char::from(second))
    }
}

// =============================================================================
// Citation resolution for rendering
// =============================================================================

/// Pick the display name for the first author of a bibitem (for citation text).
///
/// Priority: variant_unicode > mononym > family name.
pub fn citation_display_name(authors: &[AuthorName]) -> Option<String> {
    authors.first().and_then(|a| {
        a.variant_unicode
            .clone()
            .or_else(|| a.mononym.clone())
            .or_else(|| a.family.clone())
    })
}

/// Build a citation data map from rendered items for suffix-aware `\cite*{}` resolution.
pub fn build_citation_map(items: &[(BibItem, RenderContext)]) -> HashMap<String, CitationData> {
    items
        .iter()
        .map(|(bib, ctx)| {
            let data = CitationData {
                author: citation_display_name(&ctx.authors),
                year: bib.date_year,
                year_suffix: ctx.year_suffix.clone(),
            };
            (bib.bibkey.clone(), data)
        })
        .collect()
}

/// Resolve `\cite*{}` commands in title_latex and note_latex using suffix-aware citation data.
///
/// Only resolves fields that actually contain `\cite` commands. Stores results in
/// `resolved_title` / `resolved_note` on the RenderContext.
pub fn resolve_citations_in_fields(
    items: &mut [(BibItem, RenderContext)],
    citation_map: &HashMap<String, CitationData>,
) {
    for (bib, ctx) in items.iter_mut() {
        if bib.title_latex.contains("\\cite") {
            ctx.resolved_title = Some(substitute_citations_html(&bib.title_latex, citation_map));
        }
        if let Some(ref note) = bib.note_latex
            && note.contains("\\cite")
        {
            ctx.resolved_note = Some(substitute_citations_html(note, citation_map));
        }
    }
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
            journal_key: None,
            publisher_key: None,
            address: None,
            volume: None,
            volume_numeric: None,
            number: None,
            number_numeric: None,
            pages: None,
            start_page: None,
            eid: None,
            series_key: None,
            edition: None,
            institution_key: None,
            school_key: None,
            type_field: None,
            doi: None,
            url: None,
            eprint: None,
            urn: None,
            crossref: None,
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
            person_key: None,
            has_fulltext: false,
            fulltext_path: None,
            license: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Helper: create an AuthorName with family + given.
    fn make_author(given: &str, family: &str) -> AuthorName {
        let initial = given
            .chars()
            .next()
            .unwrap_or('x')
            .to_lowercase()
            .next()
            .unwrap_or('x');
        AuthorName {
            author_key: format!("{}_{}", family.to_lowercase(), initial),
            family: Some(family.to_string()),
            given: Some(given.to_string()),
            mononym: None,
            variant_unicode: None,
        }
    }

    /// Helper: create an AuthorName with a mononym.
    fn make_mononym(name: &str) -> AuthorName {
        AuthorName {
            author_key: name.to_lowercase(),
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
            html.contains("class=\"smallcaps\" data-author-key=\"smith_j\">Smith</span>"),
            "family name in smallcaps with author key"
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
        let item = make_bibitem(
            EntryType::Book,
            "kant:1781",
            "Critique of Pure Reason",
            Some(1781),
        );

        let ctx = RenderContext {
            authors: vec![make_author("Immanuel", "Kant")],
            publisher_name: Some("Johann Friedrich Hartknoch".to_string()),
            address: Some("Riga".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(html.contains("data-type=\"book\""), "entry type book");
        assert!(
            html.contains("class=\"smallcaps\" data-author-key=\"kant_i\">Kant</span>"),
            "author family with key"
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
        let item = make_bibitem(
            EntryType::Book,
            "wellman-frey:2003",
            "A Companion to Applied Ethics",
            Some(2003),
        );

        let ctx = RenderContext {
            editors: vec![
                make_author("R. G.", "Wellman"),
                make_author("Christopher Heath", "Frey"),
            ],
            publisher_name: Some("Blackwell".to_string()),
            address: Some("Oxford".to_string()),
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
        item.pages = Some("100-120".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("John", "Doe")],
            editors: vec![make_author("Jane", "Editor")],
            booktitle_unicode: Some("The Big Book".to_string()),
            address: Some("New York".to_string()),
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
        assert!(
            html.contains("edited by Jane <span class=\"smallcaps\">Editor</span>"),
            "editor inline with smallcaps: {html}"
        );
        assert!(html.contains("pp. "), "range pages use pp.");
        assert!(html.contains("100\u{2013}120"), "pages with en-dash");
    }

    #[test]
    fn test_render_incollection_single_page() {
        let mut item = make_bibitem(
            EntryType::Incollection,
            "doe:2020ch",
            "A Chapter Title",
            Some(2020),
        );
        item.pages = Some("42".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("John", "Doe")],
            booktitle_unicode: Some("The Big Book".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(html.contains("p. "), "single page uses p.");
        assert!(!html.contains("pp. "), "single page must not use pp.");
        assert!(html.contains("42"), "page number present");
    }

    #[test]
    fn test_render_incollection_roman_page() {
        // Roman numeral intro pages (ix, vii, xi) appear in real incollection data
        let mut item = make_bibitem(
            EntryType::Incollection,
            "doe:2020intro",
            "Introduction",
            Some(2020),
        );
        item.pages = Some("ix".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("John", "Doe")],
            booktitle_unicode: Some("Collected Essays".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(html.contains("p. "), "roman numeral page uses p.");
        assert!(
            !html.contains("pp. "),
            "roman numeral page must not use pp."
        );
        assert!(html.contains("ix"), "roman numeral present");
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
            html.contains(
                "data-field=\"author-name\" data-author-key=\"aristotle\">Aristotle</span>"
            ),
            "mononym rendered with key"
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

    // =========================================================================
    // Test: data-author-key on family name span
    // =========================================================================

    #[test]
    fn test_data_author_key_on_family_span() {
        let item = make_article("test:key", "Test", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Smith")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-author-key=\"smith_j\""),
            "author key on family span: {html}"
        );
        assert!(
            html.contains("class=\"smallcaps\" data-author-key=\"smith_j\">Smith</span>"),
            "key is on the smallcaps element: {html}"
        );
    }

    #[test]
    fn test_data_author_key_on_mononym() {
        let item = make_article("test:mono", "Test", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_mononym("Aristotle")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-author-key=\"aristotle\">Aristotle</span>"),
            "author key on mononym span: {html}"
        );
    }

    // =========================================================================
    // Test: data-author-key on em-dash (comma-separated for multi-author)
    // =========================================================================

    #[test]
    fn test_data_author_key_on_emdash() {
        let item1 = make_article("smith:2020", "First Paper", Some(2020));
        let item2 = make_article("smith:2021", "Second Paper", Some(2021));

        let ctx1 = RenderContext {
            authors: vec![make_author("Jane", "Smith"), make_author("John", "Doe")],
            ..Default::default()
        };
        let ctx2 = RenderContext {
            authors: vec![make_author("Jane", "Smith"), make_author("John", "Doe")],
            ..Default::default()
        };

        let html = render_bibliography(&[(item1, ctx1), (item2, ctx2)]);
        let lines: Vec<&str> = html.lines().collect();

        assert!(
            lines[1].contains("data-author-key=\"smith_j,doe_j\""),
            "comma-separated keys on em-dash: {}",
            lines[1]
        );
    }

    // =========================================================================
    // Test: data-journal-key
    // =========================================================================

    #[test]
    fn test_data_journal_key() {
        let item = make_article("test:jk", "Test", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            journal_name: Some("Dialectica".to_string()),
            journal_key: Some("dialectica".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-journal-key=\"dialectica\""),
            "journal key present: {html}"
        );
        assert!(
            html.contains("<em data-journal-key=\"dialectica\">Dialectica</em>"),
            "key on em element: {html}"
        );
    }

    // =========================================================================
    // Test: data-publisher-key
    // =========================================================================

    #[test]
    fn test_data_publisher_key() {
        let item = make_bibitem(EntryType::Book, "test:pk", "Test Book", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            publisher_name: Some("Oxford University Press".to_string()),
            publisher_key: Some("oup".to_string()),
            address: Some("Oxford".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-publisher-key=\"oup\""),
            "publisher key present: {html}"
        );
        assert!(
            html.contains(
                "data-field=\"publisher\" data-publisher-key=\"oup\">Oxford University Press</span>"
            ),
            "key on publisher span: {html}"
        );
    }

    // =========================================================================
    // Test: data-series-key
    // =========================================================================

    #[test]
    fn test_data_series_key() {
        let item = make_bibitem(EntryType::Book, "test:sk", "Test Book", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            series_name: Some("Cambridge Studies".to_string()),
            series_key: Some("cambridge_studies".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-series-key=\"cambridge_studies\""),
            "series key present: {html}"
        );
    }

    // =========================================================================
    // Test: data-school-key
    // =========================================================================

    #[test]
    fn test_data_school_key() {
        let item = make_bibitem(EntryType::Phdthesis, "test:school", "My Thesis", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            school_name: Some("MIT".to_string()),
            school_key: Some("mit".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-school-key=\"mit\""),
            "school key present: {html}"
        );
        assert!(
            html.contains("data-field=\"school\" data-school-key=\"mit\">MIT</span>"),
            "key on school span: {html}"
        );
    }

    // =========================================================================
    // Test: data-institution-key
    // =========================================================================

    #[test]
    fn test_data_institution_key() {
        let item = make_bibitem(EntryType::Misc, "test:inst", "Tech Report", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            institution_name: Some("CERN".to_string()),
            institution_key: Some("cern".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-institution-key=\"cern\""),
            "institution key present: {html}"
        );
        assert!(
            html.contains("data-field=\"institution\" data-institution-key=\"cern\">CERN</span>"),
            "key on institution span: {html}"
        );
    }

    // =========================================================================
    // Test: data-author-key on variant_unicode author
    // =========================================================================

    #[test]
    fn test_data_author_key_on_variant_unicode() {
        let item = make_article("test:variant", "Test", Some(2024));

        let ctx = RenderContext {
            authors: vec![AuthorName {
                author_key: "plato_v".to_string(),
                family: Some("Platon".to_string()),
                given: None,
                mononym: None,
                variant_unicode: Some("Plato (variant)".to_string()),
            }],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-author-key=\"plato_v\""),
            "author key present on variant_unicode author: {html}"
        );
        assert!(
            html.contains(
                "data-field=\"author-name\" data-author-key=\"plato_v\">Plato (variant)</span>"
            ),
            "key on author-name span with variant text: {html}"
        );
    }

    // =========================================================================
    // Test: no key attribute emitted when key is None
    // =========================================================================

    #[test]
    fn test_key_none_no_attribute() {
        let item = make_article("test:nokey", "Test", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            journal_name: Some("Some Journal".to_string()),
            journal_key: None,
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            !html.contains("data-journal-key"),
            "no journal key attribute when key is None: {html}"
        );
    }

    // =========================================================================
    // Test: no double period when given name ends with "."
    // =========================================================================

    #[test]
    fn test_no_double_period_abbreviated_given_name() {
        let item = make_article("zimmerman:2005", "Some Title", Some(2005));

        let ctx = RenderContext {
            authors: vec![make_author("Dean W.", "Zimmerman")],
            journal_name: Some("Dialectica".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            !html.contains(".."),
            "no double period in rendered output: {html}"
        );
        assert!(
            html.contains("Dean W.</span></span>"),
            "given name period preserved: {html}"
        );
    }

    #[test]
    fn test_no_double_period_single_initial() {
        let item = make_article("doe:2024", "Test", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_author("J.", "Doe")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            !html.contains(".."),
            "no double period with single initial: {html}"
        );
    }

    // =========================================================================
    // Test: incollection with booktitle and address from RenderContext
    // =========================================================================

    #[test]
    fn test_render_incollection_with_full_context() {
        let mut item = make_bibitem(EntryType::Incollection, "ch:2020", "A Chapter", Some(2020));
        item.pages = Some("100-120".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Author")],
            editors: vec![make_author("Ed", "Editor")],
            booktitle_unicode: Some("The Big Book".to_string()),
            address: Some("New York".to_string()),
            publisher_name: Some("Academic Press".to_string()),
            series_name: Some("Great Series".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("In <span data-field=\"booktitle\"><em>The Big Book</em></span>"),
            "booktitle from ctx: {html}"
        );
        assert!(
            html.contains("edited by Ed <span class=\"smallcaps\">Editor</span>"),
            "editors from ctx with smallcaps: {html}"
        );
        assert!(
            html.contains("Great Series"),
            "series rendered in incollection: {html}"
        );
        assert!(html.contains("New York: "), "address from ctx: {html}");
        assert!(
            html.contains("Academic Press"),
            "publisher from ctx: {html}"
        );
    }

    #[test]
    fn test_render_incollection_no_container_data() {
        let mut item = make_bibitem(EntryType::Incollection, "ch:2020", "A Chapter", Some(2020));
        item.pages = Some("42".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Author")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(!html.contains("In "), "no booktitle when absent: {html}");
        assert!(
            html.contains("p. "),
            "pages still render without container: {html}"
        );
    }

    // =========================================================================
    // Test: book with address from RenderContext
    // =========================================================================

    #[test]
    fn test_render_book_address_from_ctx() {
        let item = make_bibitem(EntryType::Book, "book:2024", "A Book", Some(2024));

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            publisher_name: Some("Publisher".to_string()),
            address: Some("London".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("London: "),
            "address from ctx in book: {html}"
        );
        assert!(html.contains("Publisher"), "publisher present: {html}");
    }

    // =========================================================================
    // Test: series with number in book
    // =========================================================================

    #[test]
    fn test_render_book_series_with_number() {
        let item = make_bibitem(
            EntryType::Book,
            "sider:2008",
            "Contemporary Debates in Metaphysics",
            Some(2008),
        );

        let ctx = RenderContext {
            editors: vec![
                make_author("Theodore", "Sider"),
                make_author("John", "Hawthorne"),
                make_author("Dean W.", "Zimmerman"),
            ],
            series_name: Some("Contemporary Debates in Philosophy".to_string()),
            series_key: Some("contemporary_debates_in_philosophy".to_string()),
            series_number: Some("10".to_string()),
            publisher_name: Some("Wiley-Blackwell".to_string()),
            address: Some("Malden, Massachusetts".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("Contemporary Debates in Philosophy</span> n.\u{00a0}<span data-field=\"number\">10</span>"),
            "series with number rendered: {html}"
        );
    }

    // =========================================================================
    // Test: series without number in incollection
    // =========================================================================

    #[test]
    fn test_render_incollection_series_without_number() {
        let mut item = make_bibitem(
            EntryType::Incollection,
            "zimmerman:2011",
            "Presentism and the Space-Time Manifold",
            Some(2011),
        );
        item.pages = Some("163--245".to_string());
        item.doi = Some("10.1093/oxfordhb/9780199298204.003.0008".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Dean W.", "Zimmerman")],
            editors: vec![make_author("Craig", "Callender")],
            booktitle_unicode: Some("The Oxford Handbook of Philosophy of Time".to_string()),
            series_name: Some("Oxford Handbooks".to_string()),
            series_key: Some("oxford_handbooks".to_string()),
            publisher_name: Some("Oxford University Press".to_string()),
            address: Some("Oxford".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("Oxford Handbooks</span>"),
            "series name rendered: {html}"
        );
        assert!(
            !html.contains("n.\u{00a0}"),
            "no series number when absent: {html}"
        );
        assert!(html.contains("Oxford: "), "address rendered: {html}");
        assert!(
            html.contains("Oxford University Press"),
            "publisher rendered: {html}"
        );
    }

    // =========================================================================
    // Test: series with number in incollection
    // =========================================================================

    #[test]
    fn test_render_incollection_series_with_number() {
        let mut item = make_bibitem(
            EntryType::Incollection,
            "zimmerman:2008b",
            "The Privileged Present",
            Some(2008),
        );
        item.pages = Some("211--225".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Dean W.", "Zimmerman")],
            editors: vec![
                make_author("Theodore", "Sider"),
                make_author("John", "Hawthorne"),
                make_author("Dean W.", "Zimmerman"),
            ],
            booktitle_unicode: Some("Contemporary Debates in Metaphysics".to_string()),
            series_name: Some("Contemporary Debates in Philosophy".to_string()),
            series_key: Some("contemporary_debates_in_philosophy".to_string()),
            series_number: Some("10".to_string()),
            publisher_name: Some("Wiley-Blackwell".to_string()),
            address: Some("Malden, Massachusetts".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("Contemporary Debates in Philosophy</span> n.\u{00a0}<span data-field=\"number\">10</span>"),
            "series with number in incollection: {html}"
        );
        assert!(
            html.contains("Malden, Massachusetts: "),
            "address after series: {html}"
        );
    }

    // =========================================================================
    // Test: incollection editor small caps with multiple editors
    // =========================================================================

    #[test]
    fn test_render_incollection_editor_smallcaps() {
        let mut item = make_bibitem(EntryType::Incollection, "ch:2020", "A Chapter", Some(2020));
        item.pages = Some("1-10".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Author")],
            editors: vec![
                make_author("Theodore", "Sider"),
                make_author("John", "Hawthorne"),
                make_author("Dean W.", "Zimmerman"),
            ],
            booktitle_unicode: Some("Some Book".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("Theodore <span class=\"smallcaps\">Sider</span>"),
            "first editor family in smallcaps: {html}"
        );
        assert!(
            html.contains("John <span class=\"smallcaps\">Hawthorne</span>"),
            "second editor family in smallcaps: {html}"
        );
        assert!(
            html.contains("Dean W. <span class=\"smallcaps\">Zimmerman</span>"),
            "third editor family in smallcaps: {html}"
        );
    }

    // =========================================================================
    // Test: incollection publisher is period-separated from container
    // =========================================================================

    #[test]
    fn test_render_incollection_publisher_period_separated() {
        let mut item = make_bibitem(EntryType::Incollection, "ch:2020", "A Chapter", Some(2020));
        item.pages = Some("100-120".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Author")],
            booktitle_unicode: Some("The Book".to_string()),
            publisher_name: Some("Publisher".to_string()),
            address: Some("City".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            !html.contains("100\u{2013}120, City"),
            "publisher should NOT be comma-joined with pages: {html}"
        );
        assert!(
            html.contains("100\u{2013}120</span>. City: "),
            "publisher is period-separated from pages: {html}"
        );
    }

    // =========================================================================
    // Test: note rendering in book
    // =========================================================================

    #[test]
    fn test_render_book_with_note() {
        let mut item = make_bibitem(
            EntryType::Book,
            "reichenbach_h:1928",
            "Philosophie der Raum-Zeit-Lehre",
            Some(1928),
        );
        item.note_unicode = Some("English translation: Reichenbach (1958)".to_string());
        item.doi = Some("10.1515/9783111485676".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Hans", "Reichenbach")],
            publisher_name: Some("de Gruyter".to_string()),
            address: Some("Berlin".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("English translation: Reichenbach (1958)"),
            "note rendered in book: {html}"
        );
        assert!(
            html.contains("de Gruyter"),
            "publisher still present: {html}"
        );
        assert!(html.contains("doi:"), "DOI still present: {html}");
    }

    #[test]
    fn test_render_book_no_note() {
        let item = make_bibitem(EntryType::Book, "kant:1781", "Critique", Some(1781));

        let ctx = RenderContext {
            authors: vec![make_author("Immanuel", "Kant")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(!html.contains("note"), "no note field when absent: {html}");
    }

    // =========================================================================
    // Test: note rendering in article
    // =========================================================================

    #[test]
    fn test_render_article_with_note() {
        let mut item = make_article("smith:2020", "Some Article", Some(2020));
        item.note_unicode = Some("Reprinted in Jones (2021)".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Smith")],
            journal_name: Some("Dialectica".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("Reprinted in Jones (2021)"),
            "note rendered in article: {html}"
        );
    }

    // =========================================================================
    // Test: note rendering in incollection
    // =========================================================================

    #[test]
    fn test_render_incollection_with_note() {
        let mut item = make_bibitem(EntryType::Incollection, "ch:2020", "A Chapter", Some(2020));
        item.pages = Some("100-120".to_string());
        item.note_unicode = Some("Originally published in 2015".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("John", "Doe")],
            booktitle_unicode: Some("The Book".to_string()),
            publisher_name: Some("Publisher".to_string()),
            address: Some("City".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("Originally published in 2015"),
            "note rendered in incollection: {html}"
        );
    }

    // =========================================================================
    // Test: note rendering in thesis
    // =========================================================================

    #[test]
    fn test_render_thesis_with_note() {
        let mut item = make_bibitem(
            EntryType::Phdthesis,
            "student:2023",
            "My Dissertation",
            Some(2023),
        );
        item.note_unicode = Some("Forthcoming as a book".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Alice", "Student")],
            school_name: Some("University of Somewhere".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("Forthcoming as a book"),
            "note rendered in thesis: {html}"
        );
    }

    // =========================================================================
    // Test: year suffix in date rendering
    // =========================================================================

    #[test]
    fn test_render_date_with_suffix() {
        let item = make_article("smith:2020a", "First Paper", Some(2020));

        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Smith")],
            year_suffix: Some("a".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-field=\"date\">2020a</span>"),
            "year suffix rendered: {html}"
        );
    }

    #[test]
    fn test_render_date_without_suffix() {
        let item = make_article("smith:2020", "Only Paper", Some(2020));

        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Smith")],
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("data-field=\"date\">2020</span>"),
            "no suffix when None: {html}"
        );
    }

    // =========================================================================
    // Test: parse_bibkey_suffix
    // =========================================================================

    #[test]
    fn test_parse_bibkey_suffix() {
        assert_eq!(super::parse_bibkey_suffix("smith:2020a"), "a");
        assert_eq!(super::parse_bibkey_suffix("smith:2020b"), "b");
        assert_eq!(super::parse_bibkey_suffix("smith:2020"), "");
        assert_eq!(super::parse_bibkey_suffix("smith:2020abc"), "abc");
        assert_eq!(super::parse_bibkey_suffix("jones_a:1999"), "");
        assert_eq!(super::parse_bibkey_suffix("jones_a:1999a"), "a");
    }

    // =========================================================================
    // Test: suffix_letter
    // =========================================================================

    #[test]
    fn test_suffix_letter() {
        assert_eq!(super::suffix_letter(0), "a");
        assert_eq!(super::suffix_letter(1), "b");
        assert_eq!(super::suffix_letter(25), "z");
        assert_eq!(super::suffix_letter(26), "aa");
        assert_eq!(super::suffix_letter(27), "ab");
    }

    // =========================================================================
    // Test: assign_year_suffixes
    // =========================================================================

    #[test]
    fn test_assign_year_suffixes_no_collision() {
        let mut main = vec![
            (
                make_article("smith:2020", "Paper A", Some(2020)),
                RenderContext {
                    authors: vec![make_author("Jane", "Smith")],
                    ..Default::default()
                },
            ),
            (
                make_article("jones:2021", "Paper B", Some(2021)),
                RenderContext {
                    authors: vec![make_author("Bob", "Jones")],
                    ..Default::default()
                },
            ),
        ];

        assign_year_suffixes(&mut main, &mut []);

        assert_eq!(main[0].1.year_suffix, None, "no collision, no suffix");
        assert_eq!(main[1].1.year_suffix, None, "no collision, no suffix");
    }

    #[test]
    fn test_assign_year_suffixes_main_only() {
        let mut main = vec![
            (
                make_article("smith:2020a", "First", Some(2020)),
                RenderContext {
                    authors: vec![make_author("Jane", "Smith")],
                    ..Default::default()
                },
            ),
            (
                make_article("smith:2020b", "Second", Some(2020)),
                RenderContext {
                    authors: vec![make_author("Jane", "Smith")],
                    ..Default::default()
                },
            ),
        ];

        assign_year_suffixes(&mut main, &mut []);

        assert_eq!(
            main[0].1.year_suffix,
            Some("a".to_string()),
            "first gets 'a'"
        );
        assert_eq!(
            main[1].1.year_suffix,
            Some("b".to_string()),
            "second gets 'b'"
        );
    }

    #[test]
    fn test_assign_year_suffixes_main_before_further() {
        let mut main = vec![
            (
                make_article("jones:2020b", "Main 1", Some(2020)),
                RenderContext {
                    authors: vec![make_author("Bob", "Jones")],
                    ..Default::default()
                },
            ),
            (
                make_article("jones:2020c", "Main 2", Some(2020)),
                RenderContext {
                    authors: vec![make_author("Bob", "Jones")],
                    ..Default::default()
                },
            ),
        ];
        let mut further = vec![(
            make_article("jones:2020a", "Further 1", Some(2020)),
            RenderContext {
                authors: vec![make_author("Bob", "Jones")],
                ..Default::default()
            },
        )];

        assign_year_suffixes(&mut main, &mut further);

        assert_eq!(
            main[0].1.year_suffix,
            Some("a".to_string()),
            "jones:2020b (main, first by suffix) gets 'a'"
        );
        assert_eq!(
            main[1].1.year_suffix,
            Some("b".to_string()),
            "jones:2020c (main, second by suffix) gets 'b'"
        );
        assert_eq!(
            further[0].1.year_suffix,
            Some("c".to_string()),
            "jones:2020a (further) gets 'c'"
        );
    }

    #[test]
    fn test_assign_year_suffixes_three_in_main() {
        let mut main = vec![
            (
                make_article("smith:2020a", "First", Some(2020)),
                RenderContext {
                    authors: vec![make_author("Jane", "Smith")],
                    ..Default::default()
                },
            ),
            (
                make_article("smith:2020b", "Second", Some(2020)),
                RenderContext {
                    authors: vec![make_author("Jane", "Smith")],
                    ..Default::default()
                },
            ),
            (
                make_article("smith:2020c", "Third", Some(2020)),
                RenderContext {
                    authors: vec![make_author("Jane", "Smith")],
                    ..Default::default()
                },
            ),
        ];

        assign_year_suffixes(&mut main, &mut []);

        assert_eq!(main[0].1.year_suffix, Some("a".to_string()));
        assert_eq!(main[1].1.year_suffix, Some("b".to_string()));
        assert_eq!(main[2].1.year_suffix, Some("c".to_string()));
    }

    // =========================================================================
    // Test: resolved_title overrides title_unicode
    // =========================================================================

    #[test]
    fn test_resolved_title_used_over_unicode() {
        let mut item = make_article("test:2020", "Unicode Title", Some(2020));
        item.title_latex = "Latex Title".to_string();

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            resolved_title: Some("Resolved Title with Smith (2020a)".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("Resolved Title with Smith (2020a)"),
            "resolved title used: {html}"
        );
        assert!(
            !html.contains("Unicode Title"),
            "unicode title not used: {html}"
        );
    }

    // =========================================================================
    // Test: resolved_note overrides note_unicode
    // =========================================================================

    #[test]
    fn test_resolved_note_used_over_unicode() {
        let mut item = make_bibitem(EntryType::Book, "test:2020", "A Book", Some(2020));
        item.note_unicode = Some("Translation: Jones (2021)".to_string());

        let ctx = RenderContext {
            authors: vec![make_author("Test", "Author")],
            resolved_note: Some("Translation: Jones (2021a)".to_string()),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("Translation: Jones (2021a)"),
            "resolved note used: {html}"
        );
        assert!(
            !html.contains("Translation: Jones (2021)\""),
            "unicode note not used when resolved present"
        );
    }

    #[test]
    fn test_resolved_title_html_spans_not_escaped() {
        let mut item = make_bibitem(
            EntryType::Book,
            "fine_k:2005g",
            r"Tense and Reality, see also \citet{fine_k:2005a}",
            Some(2005),
        );
        item.title_unicode = None;

        let ctx = RenderContext {
            authors: vec![make_author("Kit", "Fine")],
            resolved_title: Some(
                "Tense and Reality, see also <span data-bibkey=\"fine_k:2005a\">Fine (2005a)</span>"
                    .to_string(),
            ),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("<span data-bibkey=\"fine_k:2005a\">Fine (2005a)</span>"),
            "HTML spans in resolved title must not be escaped: {html}"
        );
        assert!(
            !html.contains("&lt;span"),
            "no escaped angle brackets: {html}"
        );
    }

    #[test]
    fn test_resolved_note_html_spans_not_escaped() {
        let mut item = make_bibitem(
            EntryType::Book,
            "reichenbach_h:1928",
            "Philosophie der Raum-Zeit-Lehre",
            Some(1928),
        );
        item.note_latex = Some(r"English translation: \citet{reichenbach_h:1958}".to_string());
        item.note_unicode = None;

        let ctx = RenderContext {
            authors: vec![make_author("Hans", "Reichenbach")],
            resolved_note: Some(
                "English translation: <span data-bibkey=\"reichenbach_h:1958\">Reichenbach (1958)</span>"
                    .to_string(),
            ),
            ..Default::default()
        };

        let html = render_bibitem(&item, &ctx);

        assert!(
            html.contains("<span data-bibkey=\"reichenbach_h:1958\">Reichenbach (1958)</span>"),
            "HTML spans in resolved note must not be escaped: {html}"
        );
        assert!(
            !html.contains("&lt;span"),
            "no escaped angle brackets: {html}"
        );
    }
}
