# Architecture

Alexandria Nexus follows hexagonal architecture with five layers. Dependencies flow **inward only**: outer layers depend on inner layers, never the reverse.

## Layer Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 1: DOMAIN  (src/domain/)                                     │
│  Pure types. Zero I/O. Zero framework imports.                      │
│  Entities, enums, projections, junction row types, DTOs, transforms │
├─────────────────────────────────────────────────────────────────────┤
│  Layer 2: LOGIC  (src/logic/)                                       │
│  Pure functions. No I/O. No async.                                  │
│  Takes data in, returns data out. Deterministic and testable.       │
│  Validation, CSV parsing, rendering, type conversions               │
├─────────────────────────────────────────────────────────────────────┤
│  Layer 3: PROCESS  (src/process/)                                   │
│  Orchestration. Defines WHAT needs to happen.                       │
│  Receives I/O capabilities as trait objects / abstract interfaces.  │
│  Combines pure logic with injected I/O. Zero concrete I/O.         │
│  Import, export, search, keyword tree                               │
├─────────────────────────────────────────────────────────────────────┤
│  Layer 4: ADAPTERS  (src/adapters/)                                 │
│  Concrete I/O. Defines HOW things happen.                           │
│  Implements process traits with Postgres, HTTP, filesystem.         │
│  Handlers are THIN: extract HTTP → call process → format response.  │
├─────────────────────────────────────────────────────────────────────┤
│  Layer 5: COMPOSITION  (src/composition/)                           │
│  Wires everything together. The only layer that knows all others.   │
│  build_app(), AppState, routing, middleware                         │
└─────────────────────────────────────────────────────────────────────┘
```

## Dependency Rules

These rules are **non-negotiable**. Every import must respect them.

| Layer | Can import from | CANNOT import from |
|-------|----------------|-------------------|
| Domain | nothing (pure) | logic, process, adapters, composition |
| Logic | domain | process, adapters, composition |
| Process | domain, logic | adapters, composition |
| Adapters (trait impls) | domain, logic, process **traits only** | composition |
| Adapters (handlers) | domain, logic, process (traits + types + functions) | composition |
| Composition | everything | — |

**Adapter trait impls and process:** Adapter trait impl files (e.g., `adapters/search.rs`, `adapters/render.rs`) import from process **only to implement traits** that process defines. They CANNOT import process functions or orchestration logic — only trait definitions. This keeps the dependency one-directional: process defines contracts, adapters fulfill them.

**Handlers and process:** Handlers are a special case within the adapters layer. They import process **traits** (to construct adapter impls), process **types** (e.g., `ResolveResult`), and process **functions** (e.g., `render_bibitems_to_html`). This is necessary because process orchestration functions coordinate multiple traits — logic that cannot live on any single trait. Handlers construct the concrete adapter impls and pass them to process functions.

## The Process / Adapter Contract

This is the core architectural pattern. **Process defines contracts (traits). Adapters implement them.**

```rust
// PROCESS layer — defines WHAT it needs (no Postgres, no SQL, no pool)
#[async_trait]
pub trait BibitemSearcher {
    async fn search(&self, query: &str, filters: &SearchFilters, limit: i64, offset: i64)
        -> Result<(Vec<BibItem>, i64), HexforgeError>;
}

pub async fn perform_search(
    searcher: &impl BibitemSearcher,
    request: SearchRequest,
) -> Result<SearchResponse, HexforgeError> {
    // Pure orchestration: validate input, call trait, format output
}

// ADAPTER layer — defines HOW (Postgres-specific)
struct PgBibitemSearcher { pool: PgPool }

#[async_trait]
impl BibitemSearcher for PgBibitemSearcher {
    async fn search(&self, query: &str, ...) -> Result<...> {
        // pg_trgm similarity(), SQL, bind params — all Postgres-specific
    }
}

// HANDLER — thin HTTP glue
pub async fn search_bibitems(State(state): State<AppState>, Json(req): Json<SearchRequest>) {
    let searcher = PgBibitemSearcher::new(state.pool.pool());
    let response = perform_search(&searcher, req).await?;
    Ok(Json(response))
}
```

**Why this matters:** If you switch from Postgres to MongoDB, or add ElasticSearch alongside Postgres, you only implement new adapters. Process, logic, and domain are 100% reusable. You wire the new adapters in composition and nothing else changes.

**Key rules:**

1. **Process NEVER has concrete I/O.** No `PgPool`, no `query()`, no `sqlx`, no filesystem. If it needs I/O, it receives a trait object.
2. **Process defines traits for its I/O needs.** These traits use only domain types in their signatures.
3. **Adapters implement process traits.** The Postgres adapter uses `PgPool`, raw SQL, `sqlx` types — all hidden behind the trait.
4. **Handlers construct adapters and call process.** They extract HTTP, build the concrete adapter, call the process function, format the response.
5. **hexforge's `DataSource<T>` trait** is already an abstract contract. Process can use `&impl DataSource<T>` for standard CRUD operations without depending on adapters.

## Layer Details

### Domain (`src/domain/`)

Pure types with no framework dependencies.

- **Entities**: Author, Journal, Publisher, Institution, School, Series, Keyword, BibItem, ApiKey, BibitemNotes
- **Enums**: EntryType, PubState, LangId, Epoch, AuthorRole, RefType, PermissionLevel
- **Projections**: BibItemSummary, AuthorExpanded, etc.
- **Junction row types** (`junctions.rs`): BibitemAuthorsRow, BibitemKeywordsRow, BibitemRefsRow

All entity files are generator-owned (Regenerate policy).

### Logic (`src/logic/`)

Pure functions. No `async`. No database. No `AppState`. No pool.

- `validation/` — input validation per entity
- `csv_parsing/` — pure parsers for CSV fields
- `render/` — HTML bibliography renderer (takes pre-resolved `RenderContext`, produces HTML)
- `search.rs` — pure types: `SearchRequest`, `SearchResponse`
- `import.rs` — pure types: `ImportResponse`, `ImportRowError`, CSV helpers

**Test**: if a function in `logic/` has `async` or imports anything from `adapters/`, `process/`, or `composition/`, it's in the wrong layer.

### Process (`src/process/`)

Orchestration. Defines **what** needs to happen via traits. Receives I/O as injected trait implementations.

- Defines traits for I/O operations not covered by hexforge's `DataSource`
- Uses `&impl DataSource<T>` for standard CRUD (find_by_id, insert, update)
- Uses custom traits for specialized operations (search, junction inserts, batch lookups)
- Zero concrete I/O: no `PgPool`, no `query()`, no `sqlx`, no SQL strings

**Test**: if a function in `process/` imports from `adapters/` or `composition/`, or uses `PgPool`/`sqlx`/`query()` directly, it's in the wrong layer.

### Adapters (`src/adapters/`)

Concrete I/O implementations.

- `auth.rs` — API key validator
- `db/` — database enum mappings, query filters, junction batch-fetch functions (all generated)
- `handlers/` — **thin** HTTP handlers + process trait implementations
- Implements traits defined by the process layer with Postgres-specific code

### Composition (`src/composition/`)

Wires everything together. The only layer that knows about all others.

- `mod.rs` — `build_app()`: registers CRUD resources, junctions, expansions, custom handlers
- `state.rs` — `AppState` with `DataStore<Entity, Query>` per entity and `DatabasePool`

## Code Generation

Schema source of truth: `hexforge.yml`

| Policy | Files | Behavior |
|--------|-------|----------|
| **Regenerate** | Entity files, enums, projections, db_mappings, queries, junctions, state | Always overwritten. Do not edit. |
| **CreateOnce** | Validation stubs, operation stubs, lib.rs, main.rs, composition/mod.rs | Written once, then consumer-owned. |
| **Merge** | mod.rs files | Generated modules merged with consumer-added modules and re-exports. |

## Data Model

**8 primary entities:** Author, Journal, Publisher, Institution, School, Series, Keyword, BibItem

**3 junction tables:**
- `bibitem_authors` — BibItem↔Author with role, position, name_variant_latex, name_variant_unicode
- `bibitem_keywords` — BibItem↔Keyword with keyword_level
- `bibitem_refs` — BibItem↔BibItem with ref_type

**2 internal entities:** ApiKey, BibitemNotes
