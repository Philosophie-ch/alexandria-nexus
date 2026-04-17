//! Per-entry-type rendering functions.
//!
//! Each function assembles the full HTML string for a specific bibliography entry type,
//! composing the reusable component functions from `components.rs`.

use crate::domain::{AuthorRole, BibItem};

use super::RenderContext;
use super::components::{
    render_access, render_authors, render_booktitle, render_date, render_edition, render_eid,
    render_journal, render_note, render_pages, render_publisher, render_series, render_title,
    render_volume_number,
};

// =============================================================================
// Helper: push a segment with trailing period+space
// =============================================================================

/// Append a non-empty segment followed by ". " to the parts list.
fn push_segment(parts: &mut Vec<String>, segment: String) {
    if !segment.is_empty() {
        parts.push(segment);
    }
}

/// Join parts with ". " and ensure trailing period.
fn join_with_periods(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let mut result = parts.join(". ");
    // Ensure trailing period
    if !result.ends_with('.') {
        result.push('.');
    }
    result
}

// =============================================================================
// Article
// =============================================================================

/// Render an article entry.
///
/// `AUTHOR. YEAR. "TITLE." JOURNAL VOLUME(NUMBER): PAGES. doi:DOI`
pub fn render_article(item: &BibItem, ctx: &RenderContext) -> String {
    let mut parts = Vec::new();

    // AUTHOR
    let author_html = render_authors(&ctx.authors, AuthorRole::Author, ctx.suppress_author);
    push_segment(&mut parts, author_html);

    // YEAR
    push_segment(&mut parts, render_date(item));

    // "TITLE." + JOURNAL VOLUME(NUMBER): PAGES
    let title = render_title(&item.title_unicode, true);
    let journal = render_journal(ctx.journal_name.as_deref());
    let vol_num = render_volume_number(item.volume.as_deref(), item.number.as_deref());

    let pages_or_eid = if let Some(pages) = item.pages.as_deref() {
        if pages.is_empty() {
            render_eid(item.eid.as_deref())
        } else {
            render_pages(pages)
        }
    } else {
        render_eid(item.eid.as_deref())
    };

    // Build the container part: JOURNAL VOLUME(NUMBER): PAGES
    let mut container = String::new();
    if !journal.is_empty() {
        container.push_str(&journal);
    }
    if !vol_num.is_empty() {
        if !container.is_empty() {
            container.push(' ');
        }
        container.push_str(&vol_num);
    }
    if !pages_or_eid.is_empty() {
        if !vol_num.is_empty() {
            container.push_str(": ");
        } else if !container.is_empty() {
            container.push(' ');
        }
        container.push_str(&pages_or_eid);
    }

    // Combine title and container
    if !container.is_empty() {
        push_segment(&mut parts, format!("{title}. {container}"));
    } else {
        push_segment(&mut parts, title);
    }

    // DOI/URL
    let access = render_access(item.doi.as_deref(), item.url.as_deref());
    push_segment(&mut parts, access);

    join_with_periods(&parts)
}

// =============================================================================
// Book
// =============================================================================

/// Render a book entry.
///
/// With author: `AUTHOR. YEAR. TITLE. [EDITION.] [SERIES.] ADDRESS: PUBLISHER. doi:DOI`
/// Edited (no author): `EDITOR, ed[s]. YEAR. TITLE. [SERIES.] ADDRESS: PUBLISHER. doi:DOI`
pub fn render_book(item: &BibItem, ctx: &RenderContext) -> String {
    let mut parts = Vec::new();

    let has_authors = !ctx.authors.is_empty();

    if has_authors {
        // AUTHOR
        let author_html = render_authors(&ctx.authors, AuthorRole::Author, ctx.suppress_author);
        push_segment(&mut parts, author_html);
    } else if !ctx.editors.is_empty() {
        // EDITOR, ed[s].
        let editor_html = render_authors(&ctx.editors, AuthorRole::Editor, false);
        push_segment(&mut parts, editor_html);
    }

    // YEAR
    push_segment(&mut parts, render_date(item));

    // TITLE (italicized for books)
    push_segment(&mut parts, render_title(&item.title_unicode, false));

    // EDITION (only for authored books per spec)
    if has_authors {
        let edition = render_edition(item.edition.as_deref());
        push_segment(&mut parts, edition);
    }

    // SERIES
    let series = render_series(ctx.series_name.as_deref());
    push_segment(&mut parts, series);

    // ADDRESS: PUBLISHER
    let publisher = render_publisher(item.address.as_deref(), ctx.publisher_name.as_deref());
    push_segment(&mut parts, publisher);

    // DOI/URL
    let access = render_access(item.doi.as_deref(), item.url.as_deref());
    push_segment(&mut parts, access);

    join_with_periods(&parts)
}

// =============================================================================
// InCollection / InProceedings
// =============================================================================

/// Render an incollection or inproceedings entry.
///
/// `AUTHOR. YEAR. "TITLE." In BOOKTITLE, [volume VOL,] [edited by EDITOR,] pp. PAGES.
///  ADDRESS: PUBLISHER. doi:DOI`
pub fn render_chapter(item: &BibItem, ctx: &RenderContext) -> String {
    let mut parts = Vec::new();

    // AUTHOR
    let author_html = render_authors(&ctx.authors, AuthorRole::Author, ctx.suppress_author);
    push_segment(&mut parts, author_html);

    // YEAR
    push_segment(&mut parts, render_date(item));

    // "TITLE." In BOOKTITLE, [edited by EDITOR,] pp. PAGES. ADDRESS: PUBLISHER
    let title = render_title(&item.title_unicode, true);
    let booktitle = render_booktitle(item.booktitle_unicode.as_deref());

    let mut container_parts: Vec<String> = Vec::new();

    // In BOOKTITLE
    if !booktitle.is_empty() {
        container_parts.push(format!("In {booktitle}"));
    }

    // volume VOL
    if let Some(ref vol) = item.volume
        && !vol.is_empty()
    {
        container_parts.push(format!(
            "volume <span data-field=\"volume\">{}</span>",
            super::components::esc(vol)
        ));
    }

    // edited by EDITOR
    if !ctx.editors.is_empty() {
        let editor_names = render_editor_inline(&ctx.editors);
        container_parts.push(format!("edited by {editor_names}"));
    }

    // pp. PAGES
    let pages_or_eid = if item.pages.as_deref().is_some_and(|p| !p.is_empty()) {
        let pages_html = render_pages(item.pages.as_deref().unwrap_or(""));
        if !pages_html.is_empty() {
            format!("pp. {pages_html}")
        } else {
            String::new()
        }
    } else {
        let eid_html = render_eid(item.eid.as_deref());
        if !eid_html.is_empty() {
            eid_html
        } else {
            String::new()
        }
    };
    if !pages_or_eid.is_empty() {
        container_parts.push(pages_or_eid);
    }

    // ADDRESS: PUBLISHER
    let publisher = render_publisher(item.address.as_deref(), ctx.publisher_name.as_deref());
    if !publisher.is_empty() {
        container_parts.push(publisher);
    }

    // Join: "TITLE." In BOOKTITLE, edited by EDITOR, pp. PAGES. ADDRESS: PUBLISHER
    let container = container_parts.join(", ");
    if !container.is_empty() {
        push_segment(&mut parts, format!("{title}. {container}"));
    } else {
        push_segment(&mut parts, title);
    }

    // DOI/URL
    let access = render_access(item.doi.as_deref(), item.url.as_deref());
    push_segment(&mut parts, access);

    join_with_periods(&parts)
}

/// Render editors inline for incollection: "Given Family and Given Family" (no small caps).
fn render_editor_inline(editors: &[super::AuthorName]) -> String {
    let parts: Vec<String> = editors
        .iter()
        .map(|a| {
            if let Some(ref mononym) = a.mononym {
                super::components::esc(mononym)
            } else {
                let given = a.given.as_deref().unwrap_or("");
                let family = a.family.as_deref().unwrap_or("");
                if given.is_empty() {
                    super::components::esc(family)
                } else {
                    format!(
                        "{} {}",
                        super::components::esc(given),
                        super::components::esc(family)
                    )
                }
            }
        })
        .collect();

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
// Thesis
// =============================================================================

/// Render a thesis entry (mastersthesis or phdthesis).
///
/// `AUTHOR. YEAR. "TITLE." TYPE, SCHOOL.`
pub fn render_thesis(item: &BibItem, ctx: &RenderContext) -> String {
    let mut parts = Vec::new();

    // AUTHOR
    let author_html = render_authors(&ctx.authors, AuthorRole::Author, ctx.suppress_author);
    push_segment(&mut parts, author_html);

    // YEAR
    push_segment(&mut parts, render_date(item));

    // "TITLE." TYPE, SCHOOL
    let title = render_title(&item.title_unicode, true);

    let thesis_type = item.type_field.as_deref().unwrap_or(match item.entry_type {
        crate::domain::EntryType::Phdthesis => "PhD thesis",
        crate::domain::EntryType::Mastersthesis => "Master\u{2019}s thesis",
        _ => "Thesis",
    });

    let mut suffix_parts: Vec<String> = Vec::new();
    suffix_parts.push(thesis_type.to_string());
    if let Some(ref school) = ctx.school_name
        && !school.is_empty()
    {
        suffix_parts.push(super::components::esc(school));
    }

    let suffix = suffix_parts.join(", ");
    if !suffix.is_empty() {
        push_segment(&mut parts, format!("{title}. {suffix}"));
    } else {
        push_segment(&mut parts, title);
    }

    join_with_periods(&parts)
}

// =============================================================================
// Unpublished
// =============================================================================

/// Render an unpublished entry.
///
/// `AUTHOR. YEAR. "TITLE." NOTE.`
pub fn render_unpublished(item: &BibItem, ctx: &RenderContext) -> String {
    let mut parts = Vec::new();

    // AUTHOR
    let author_html = render_authors(&ctx.authors, AuthorRole::Author, ctx.suppress_author);
    push_segment(&mut parts, author_html);

    // YEAR
    push_segment(&mut parts, render_date(item));

    // "TITLE."
    let title = render_title(&item.title_unicode, true);

    // NOTE
    let note = render_note(item.note_unicode.as_deref());
    if !note.is_empty() {
        push_segment(&mut parts, format!("{title}. {note}"));
    } else {
        push_segment(&mut parts, title);
    }

    join_with_periods(&parts)
}

// =============================================================================
// Generic (misc, techreport, proceedings, unknown)
// =============================================================================

/// Render a generic entry (misc, techreport, proceedings, etc.).
///
/// `AUTHOR. YEAR. TITLE. [NOTE.] [doi:DOI]`
pub fn render_generic(item: &BibItem, ctx: &RenderContext) -> String {
    let mut parts = Vec::new();

    // AUTHOR
    let author_html = render_authors(&ctx.authors, AuthorRole::Author, ctx.suppress_author);
    push_segment(&mut parts, author_html);

    // YEAR
    push_segment(&mut parts, render_date(item));

    // TITLE (not quoted for generic types)
    push_segment(&mut parts, render_title(&item.title_unicode, false));

    // NOTE
    let note = render_note(item.note_unicode.as_deref());
    push_segment(&mut parts, note);

    // INSTITUTION (for techreports)
    if let Some(ref inst_name) = ctx.institution_name
        && !inst_name.is_empty()
    {
        push_segment(&mut parts, super::components::esc(inst_name));
    }

    // DOI/URL
    let access = render_access(item.doi.as_deref(), item.url.as_deref());
    push_segment(&mut parts, access);

    join_with_periods(&parts)
}
