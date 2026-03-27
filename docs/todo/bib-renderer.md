# Plan: Dialectica HTML Bibliography Renderer

## Context

The Dialectica compilation machine (dltc-make/citeproc) produces HTML references as flat strings with no semantic markup. Post-processing for the philosophie.ch website (reformatting, linked data, cross-linking) requires structured access to author, year, title, journal, volume, pages, etc.

This project adds a **renderer module to philosophie-bib** (Rust) as a pure domain module that takes `BibItem` domain objects and produces structured HTML with `data-field` attributes. It's a single-style renderer hardcoded to Dialectica's house style (Chicago author-date variant).

**Approach:** Dynamic rendering (Option B). HTML is rendered on the fly per API request — no stored/cached HTML. The rendering is pure string concatenation with no I/O beyond the DB query that's already happening for JSON responses. For 300K+ items this is microseconds per entry.

**IO is out of scope.** The renderer is a pure domain module (`BibItem → HTML`). API endpoints that expose it are a separate concern.

## Input: BibItem (from philosophie-bib domain)

The Rust `BibItem` struct mirrors the Python SDK. Key fields used for rendering:

```rust
// crates/domain/src/bibitem.rs (existing)
pub struct BibItem {
    pub entry_type: EntryType,              // Article, Book, InCollection, etc.
    pub bibkey: BibKey,                     // validated bibkey
    pub date: Option<BibItemDate>,          // year, month, day
    pub title: Option<BibStringAttr>,       // latex, unicode, simplified
    pub booktitle: Option<BibStringAttr>,
    pub journal_id: Option<i64>,            // → JOIN to get Journal.name
    pub volume: String,
    pub number: String,
    pub pages: Vec<PageRange>,              // start, end
    pub eid: String,
    pub edition: Option<i32>,
    pub note: Option<BibStringAttr>,
    pub doi: String,
    pub url: String,
    pub thesis_type: Option<BibStringAttr>,
    pub address: Option<BibStringAttr>,
    pub publisher_id: Option<i64>,          // → JOIN to get Publisher.name
    pub series_id: Option<i64>,             // → JOIN to get Series.name
    pub school_id: Option<i64>,             // → JOIN to get School.name
    // authors/editors via bibitem_authors junction table
}
```

`BibStringAttr` has `.latex`, `.unicode`, `.simplified` — the renderer uses `.unicode` for HTML output.

## Output format

```html
<div class="csl-entry" data-type="article" data-bibkey="smith:2024">
  <span data-field="author">
    <span data-field="family" class="smallcaps">Smith</span>,
    <span data-field="given">Jane</span>
  </span>.
  <span data-field="date">2024</span>.
  <span data-field="title">"Some Title."</span>
  <span data-field="journal"><em>Dialectica</em></span>
  <span data-field="volume">78</span>(<span data-field="number">1</span>):
  <span data-field="pages">1–25</span>.
  <span data-field="doi">doi:<a href="https://doi.org/10.48106/...">10.48106/...</a></span>
</div>
```

Every meaningful component wrapped in `data-field` spans. The outer div has `data-type` and `data-bibkey`.

## Dialectica bibliography style rules

Decoded from `dialectica.csl`. The layout is:

```
{contributors}. {date}. {title}. {description}. {secondary-contributors}.
{container-title}, {container-contributors}, {edition}, {locators-chapter},
{collection-title-journal}, {locators}. {collection-title}. {issue}.
{locators-article}. {note}. {access}
```

### Per entry type:

**article** (`entry_type == Article`):
```
AUTHOR. YEAR. "TITLE." JOURNAL VOLUME(NUMBER): PAGES. doi:DOI
```

**book** (`entry_type == Book`, with author):
```
AUTHOR. YEAR. TITLE. [EDITION.] [SERIES.] ADDRESS: PUBLISHER. doi:DOI
```

**book** (edited, no author):
```
EDITOR, ed[s]. YEAR. TITLE. [SERIES.] ADDRESS: PUBLISHER. doi:DOI
```

**incollection** (`entry_type == InCollection`):
```
AUTHOR. YEAR. "TITLE." In BOOKTITLE, [volume VOL,] [edited by EDITOR,] pp. PAGES. ADDRESS: PUBLISHER. doi:DOI
```

**thesis** (`entry_type in (MastersThesis, PhdThesis)`):
```
AUTHOR. YEAR. "TITLE." TYPE, SCHOOL.
```

**unpublished**:
```
AUTHOR. YEAR. "TITLE." NOTE.
```

**misc / techreport / other**:
```
AUTHOR. YEAR. TITLE. [NOTE.] [doi:DOI]
```

### Author formatting:
- All names: `Family, Given and Family, Given`
- Family names in small caps
- 11+ authors: first 7 then "et al."
- Editors: append `, ed.` (1 editor) or `, eds.` (2+)
- No author/editor: skip the contributor line

### Title formatting:
- Articles/chapters/unpublished: quoted `"Title"`
- Books/theses: italicized `<em>Title</em>`

### Date:
- Year only for most: `2024`
- No date → `n.d.`

### Pages:
- `start–end` (en-dash)
- Single page: just `start`

### Edition:
- Number → ordinal: `2` → `2nd ed.`

### DOI/URL:
- DOI: `doi:<a href="https://doi.org/{DOI}">{DOI}</a>`
- URL (no DOI): `<a href="{URL}">{URL}</a>`

### Consecutive author suppression:
- When rendering a full bibliography (sorted), repeated author → em-dash `—`
- Handled by `render_bibliography()`, not `render_bibitem()`

## Implementation

### Location in philosophie-bib

New module in the domain crate (pure logic, no I/O):

```
crates/domain/src/
└── html_renderer/
    ├── mod.rs              # Public API: render_bibitem(), render_bibliography()
    ├── entry_types.rs      # Per-type rendering: article, book, incollection, thesis, etc.
    ├── components.rs       # Reusable: render_authors(), render_date(), render_title(),
    │                       #   render_pages(), render_doi(), render_publisher()
    └── constants.rs        # data-field names, CSS classes
```

API endpoint (in the api crate):

```
crates/api/src/handlers/
└── html_export.rs          # Endpoints that call the domain renderer
```

### Public API (domain layer)

```rust
// crates/domain/src/html_renderer/mod.rs

/// Render a single BibItem to structured HTML.
/// `authors` and `editors` are passed separately (from junction table).
/// `journal_name`, `publisher_name`, etc. are pre-resolved by the caller.
pub fn render_bibitem(item: &BibItem, context: &RenderContext) -> String {
    match item.entry_type {
        EntryType::Article => render_article(item, context),
        EntryType::Book => render_book(item, context),
        EntryType::InCollection | EntryType::InProceedings => render_chapter(item, context),
        EntryType::MastersThesis | EntryType::PhdThesis => render_thesis(item, context),
        EntryType::Unpublished => render_unpublished(item, context),
        _ => render_generic(item, context),
    }
}

/// Render a sorted bibliography with consecutive-author em-dash suppression.
pub fn render_bibliography(items: &[(BibItem, RenderContext)]) -> String {
    let mut parts = Vec::with_capacity(items.len());
    let mut prev_author_key: Option<String> = None;
    for (item, ctx) in items {
        let current_key = author_sort_key(&ctx.authors);
        let suppress = prev_author_key.as_ref() == Some(&current_key);
        let ctx = if suppress { ctx.with_suppress_author() } else { ctx.clone() };
        parts.push(render_bibitem(item, &ctx));
        prev_author_key = Some(current_key);
    }
    parts.join("\n")
}

/// Pre-resolved data that the domain renderer needs but can't fetch itself (no I/O).
pub struct RenderContext {
    pub authors: Vec<Author>,           // ordered, from junction table
    pub editors: Vec<Author>,           // ordered, from junction table
    pub journal_name: Option<String>,   // pre-resolved unicode name
    pub publisher_name: Option<String>,
    pub series_name: Option<String>,
    pub school_name: Option<String>,
    pub suppress_author: bool,
}
```

### API endpoints

```rust
// crates/api/src/handlers/html_export.rs

/// GET /api/v1/bibitems/{bibkey}/html → single entry HTML
async fn get_bibitem_html(bibkey: Path<String>, state: State<AppState>) -> impl IntoResponse {
    // 1. Fetch bibitem + authors + journal name etc. (one query with JOINs)
    // 2. Build RenderContext
    // 3. Call render_bibitem()
    // 4. Return HTML with Content-Type: text/html
}

/// GET /api/v1/bibliography/html?keys=k1,k2,...  → sorted list with author suppression
async fn get_bibliography_html(query: Query<BibliographyParams>, state: State<AppState>) -> impl IntoResponse {
    // 1. Fetch all bibitems + contexts in bulk
    // 2. Sort by author family name, date, bibkey
    // 3. Call render_bibliography()
    // 4. Return HTML
}
```

### Component functions (domain layer)

```rust
// crates/domain/src/html_renderer/components.rs

pub fn render_authors(authors: &[Author], role: &str, suppress: bool) -> String {
    if suppress {
        return format!("<span data-field=\"{role}\">—</span>");
    }
    if authors.is_empty() {
        return String::new();
    }
    let display_authors = if authors.len() >= 11 { &authors[..7] } else { authors };
    let parts: Vec<String> = display_authors.iter().map(|a| {
        if let Some(ref mononym) = a.mononym {
            format!("<span data-field=\"author-name\">{}</span>", esc(&mononym.unicode))
        } else {
            format!(
                "<span data-field=\"family\" class=\"smallcaps\">{}</span>, <span data-field=\"given\">{}</span>",
                esc(&a.family_name.as_ref().map_or("", |n| &n.unicode)),
                esc(&a.given_name.as_ref().map_or("", |n| &n.unicode)),
            )
        }
    }).collect();

    let mut joined = join_names(&parts);
    if authors.len() >= 11 {
        joined.push_str(" et al.");
    }
    let mut result = format!("<span data-field=\"{role}\">{joined}</span>");
    if role == "editor" {
        result.push_str(if authors.len() == 1 { ", ed." } else { ", eds." });
    }
    result
}

pub fn render_date(date: &Option<BibItemDate>) -> String { /* ... */ }
pub fn render_title(title: &Option<BibStringAttr>, quoted: bool) -> String { /* ... */ }
pub fn render_pages(pages: &[PageRange]) -> String { /* ... */ }
pub fn render_doi(doi: &str, url: &str) -> String { /* ... */ }
pub fn render_publisher(address: &Option<BibStringAttr>, publisher_name: &Option<String>) -> String { /* ... */ }
pub fn render_journal(journal_name: &Option<String>) -> String { /* ... */ }

fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn join_names(parts: &[String]) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        _ => format!("{} and {}", parts[..parts.len()-1].join(", "), parts[parts.len()-1]),
    }
}
```

### Edge cases to handle

From actual dialectica.bib data:

| Case | Example | How to handle |
|------|---------|---------------|
| Name particles | `Dutilh Novaes` | `family_name` includes particle |
| Name suffixes | `Belnap, Jr.` | Check if stored in `family_name` |
| Mononyms | `Aristotle` | `author.mononym` field — render without comma |
| Multiple editors | `Wellman and Frey, eds.` | Plural `, eds.` |
| Edition numbers | `2` | Ordinal: `2nd ed.` |
| Series | `Blackwell Companions` | After publisher |
| Notes with citations | `Reprinted in Author (Year)` | Render as plain text from `.unicode` |
| No DOI, has URL | Various | Fall back to URL link |
| No author, has editor | Edited books | Editor with `, ed.` replaces author |
| eid instead of pages | `e1489` | Render eid if no pages |
| No date | Forthcoming | Render as `n.d.` or `forthcoming` based on pubstate |

## Testing

```rust
// crates/domain/src/html_renderer/tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_article() {
        let item = make_article("smith:2024", "Some Title", 2024);
        let ctx = RenderContext {
            authors: vec![make_author("Jane", "Smith")],
            journal_name: Some("Dialectica".into()),
            ..Default::default()
        };
        let html = render_bibitem(&item, &ctx);
        assert!(html.contains("data-field=\"author\""));
        assert!(html.contains("class=\"smallcaps\""));
        assert!(html.contains("data-field=\"date\">2024</span>"));
        assert!(html.contains("<em>Dialectica</em>"));
    }

    #[test]
    fn test_consecutive_author_suppression() { /* ... */ }

    #[test]
    fn test_render_book_edited() { /* ... */ }

    #[test]
    fn test_render_incollection() { /* ... */ }

    #[test]
    fn test_render_thesis() { /* ... */ }

    #[test]
    fn test_mononym() { /* ... */ }

    #[test]
    fn test_no_date() { /* ... */ }

    #[test]
    fn test_11_plus_authors() { /* ... */ }
}
```

## Estimated effort

| Component | Lines (approx) | Notes |
|-----------|----------------|-------|
| `components.rs` | ~150 | render_authors, render_date, render_title, render_pages, render_doi, render_publisher, render_journal, helpers |
| `entry_types.rs` | ~180 | 6 entry types × ~30 lines each |
| `mod.rs` | ~50 | Public API, dispatch, bibliography sorting, RenderContext |
| `constants.rs` | ~15 | CSS classes, field names |
| `html_export.rs` (api) | ~60 | Two endpoints + query building |
| Tests | ~250 | One test per entry type + edge cases |
| **Total** | **~700** | |
