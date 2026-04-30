# Architecture

Alexandria Nexus follows hexagonal architecture with five layers. Dependencies flow **inward only**: outer layers depend on inner layers, never the reverse. This document is the **authoritative source** for layer architecture, dependency rules, and structural decisions. For type safety and coding style, [CODING_STANDARDS.md](CODING_STANDARDS.md) is authoritative.

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
│  Validation, rendering, type conversions                            │
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

**Handlers and process:** Handlers import process **types** (e.g., `ResolveResult`) and process **functions** (e.g., `render_bibitems_to_html`) to call process orchestration. Handlers are **thin HTTP adapters only**: extract request → forward to process → format response. They do not contain business logic and do not decide which adapter implementation to use — that is composition's job.

**Adapter isolation:** Adapter sub-modules (`db/`, `handlers/`, `field_parsing/`, root-level trait impls) must **not** import from each other. Each sub-module interacts only with the inner layers (domain, logic, process). If two adapter sub-modules need to cooperate — e.g., a handler needs a trait impl struct — composition wires them together by injecting the adapter (via `AppState` or function parameters). No adapter reaches across to import from a sibling adapter sub-module.

## The Process / Adapter Contract

This is the core architectural pattern. **Process defines abstract I/O contracts (traits). Adapters implement them. Composition wires them.**

### The pattern

For any feature that needs I/O, process defines **separate** `read` and `write` traits — abstract function signatures with no storage concepts (no tables, columns, schemas, SQL). The orchestration function receives them as parameters:

```
raw_data = reader.read()       # abstract: returns Vec<T> — no column/table concept
result   = logic_fn(raw_data)  # pure transformation (Layer 2)
report   = writer.write(result) # abstract: returns a report — no column/table concept
```

In Rust, these are traits because async functions need a concrete type to carry state. Architecturally they are function signatures — the trait is a pragmatic carrier, not a design choice.

```rust
// PROCESS layer — defines WHAT, uses only domain types, no storage concepts
pub trait DataReader: Send + Sync {
    async fn read(&self) -> Result<Vec<Row>, HexforgeError>;
}
pub trait DataWriter: Send + Sync {
    async fn write(&self, rows: Vec<ProcessedRow>) -> Result<Report, HexforgeError>;
}

pub async fn process_fn(
    reader: &impl DataReader,
    writer: &impl DataWriter,
) -> Result<Report, HexforgeError> {
    let raw = reader.read().await?;
    let result = logic::transform(raw);      // pure logic, Layer 2
    writer.write(result).await
}

// ADAPTER layer — implements read and write SEPARATELY and INDEPENDENTLY
// Could equally be CsvReader, ElasticSearchWriter, ApiReader, etc.
struct PgReader { pool: PgPool }
impl DataReader for PgReader {
    async fn read(&self) -> Result<Vec<Row>, HexforgeError> {
        // SQL, column names, table names — all hidden here
    }
}

struct PgWriter { pool: PgPool }
impl DataWriter for PgWriter {
    async fn write(&self, rows: Vec<ProcessedRow>) -> Result<Report, HexforgeError> {
        // UPDATE statements, column names — all hidden here
    }
}

// COMPOSITION — the only layer that knows all others; wires concrete impls to process.
// The strategic decision (which backend, which concrete adapters) belongs here — in
// build_app(). Composition constructs the concrete adapters and injects them into
// handlers via AppState.
let reader = PgReader::new(pool.clone());
let writer = PgWriter::new(pool.clone());
let state = AppState { reader, writer, /* shared data sources, etc. */ };

// ADAPTER layer — HTTP handler: extract request → call process with pre-wired
// dependencies from AppState → format response.
// The handler does NOT instantiate adapters or choose backends — composition already
// made those decisions. The handler only extracts and forwards.
pub async fn my_endpoint(State(state): State<AppState>) -> impl IntoResponse {
    let report = process_fn(&state.reader, &state.writer).await?;
    Json(report)
}
```

**Why separate read and write traits:** If you switch from Postgres to MongoDB, you only write `MongoReader` and `MongoWriter`. Process, logic, and domain are untouched. You could equally mix: `CsvReader` with `PgWriter`, or `ApiReader` with `ElasticSearchWriter`. They are fully swappable without touching any other layer.

**Why composition makes the strategic decisions:** Composition is the only layer that knows all others. It constructs concrete adapter instances and delivers them to handlers via `AppState`. Handlers receive pre-wired dependencies — they never instantiate adapters, choose backends, or create pools.

### Example: search (single combined trait)

Some features combine read + orchestration into a single trait when there is no separate write step:

```rust
// PROCESS layer
pub trait BibitemSearcher {
    async fn search(&self, query: &str, filters: &SearchFilters, limit: i64, offset: i64)
        -> Result<(Vec<BibItem>, i64), HexforgeError>;
}

pub async fn perform_search(
    searcher: &impl BibitemSearcher,
    request: SearchRequest,
) -> Result<SearchResponse, HexforgeError> { /* orchestration */ }

// ADAPTER layer
struct PgBibitemSearcher { pool: PgPool }
impl BibitemSearcher for PgBibitemSearcher { /* pg_trgm SQL */ }
```

**Key rules:**

1. **Process NEVER has concrete I/O.** No `PgPool`, no `query()`, no `sqlx`, no filesystem. If it needs I/O, it receives a trait.
2. **Process defines traits for its I/O needs.** Read and write traits are defined **separately**. No table, column, or schema names anywhere in process.
3. **Adapters implement read and write independently.** `PgReader` and `PgWriter` are separate structs that can be swapped without touching each other.
4. **Composition constructs and wires concrete implementations.** Adapter instantiation (`PgReader::new(pool)`) belongs in `build_app()`, never in handlers.
5. **Handlers are thin HTTP adapters only.** Extract request → call process with injected dependencies → format response. Zero business logic. Zero wiring decisions. Zero adapter instantiation.
6. **hexforge's `DataSource<T>` trait** is already an abstract contract. For standard CRUD operations, process uses `&impl DataSource<T>` without depending on any adapter.

## hexforge Integration

Alexandria Nexus is built on the hexforge SDK. **Leverage hexforge maximally.** Always use hexforge's built-in abstractions before writing custom consumer code. Custom code is only justified when hexforge genuinely cannot express the operation — if it should but doesn't, ask for a fix upstream in hexforge rather than working around it in Alexandria.

hexforge provides derive macros, CRUD routing, pagination, expansion, filtering, OpenAPI generation, abstract data access (`DataSource<T>`), and concrete PostgreSQL adapters (`DataStore<T, Q>`). Alexandria code only adds what is domain-specific: search orchestration, bibliography rendering, import/export pipelines, LaTeX conversion.

The key architectural contract: `DataSource<T>` is hexforge's abstract CRUD trait. Process can use `&impl DataSource<T>` for standard operations without depending on any adapter. For operations beyond standard CRUD, process defines its own traits (see [The Process / Adapter Contract](#the-process--adapter-contract)).

For the full SDK reference — which hexforge abstractions Alexandria uses, when to define custom traits vs. use `DataSource`, raw SQL policy, escape-hatch re-exports — see [HEXFORGE_USAGE.md](HEXFORGE_USAGE.md).

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

Contains: validation rules, pure rendering, request/response types, parsed row types, pure transformation helpers.

**No external format concerns live here.** Parsing OR serializing an external format (CSV, JSON, XML, SQL array literals) is a boundary concern — exactly like SQL. It belongs in the adapters layer. The logic layer only sees already-typed structs and never produces format-specific output. Process functions return domain types; adapters convert them to wire/storage formats.

Although the dependency rules allow logic to import from domain, logic should only import **data types and constants** — never async trait definitions like `DataSource<T>`. Accepting a trait-bounded parameter would make the function an orchestration point (Layer 3), not a pure computation. `async` implies awaiting an external effect — I/O, timers, coordination — which breaks the deterministic, side-effect-free guarantee that makes logic functions trivially testable.

**Test**: if a function in `logic/` has `async` or imports anything from `adapters/`, `process/`, or `composition/`, it's in the wrong layer.

### Process (`src/process/`)

Orchestration. Defines **what** needs to happen via traits. Receives I/O as injected trait implementations.

- Defines traits for I/O operations not covered by hexforge's `DataSource`
- Uses `&impl DataSource<T>` for standard CRUD (find_by_id, insert, update)
- Uses custom traits for specialized operations (search, junction inserts, batch lookups)
- Zero concrete I/O: no `PgPool`, no `query()`, no `sqlx`, no SQL strings
- **Returns domain types** — process functions return `Vec<Author>`, `BibitemExportData`, etc. Serialization to CSV or any other format happens in the adapter layer.
- `latex_citations.rs` — `CitationResolver` trait + `pre_compile_citations`: resolves `\cite*{}` commands into plain text before pylatexenc
- `latex_columns.rs` — `LatexBatchConverter`/`LatexColumnFetcher`/`LatexColumnWriter` + `convert_all_columns`: orchestrates 15-column LaTeX→Unicode pipeline; writes are sequential to avoid row-lock deadlocks across same-table columns

**Test**: if a function in `process/` imports from `adapters/` or `composition/`, uses `PgPool`/`sqlx`/`query()` directly, or returns `Vec<Vec<String>>` (CSV rows), it's in the wrong layer.

### Adapters (`src/adapters/`)

Concrete I/O implementations. **All format-specific parsing AND serialization (CSV, JSON, wire formats, SQL array literals) lives here** — it's a boundary concern, exactly like SQL queries. The process layer only sees domain types; only adapters produce external formats.

- `db/` — database enum mappings, query filters (all generated)
- `field_parsing/` — CSV field parsers for the full-CSV import pipeline
- `csv_rows.rs` — CSV row-builders for entity/bibitem export and snapshot: `build_author_rows`, `build_bibitem_*_rows`, header constants, `text_array()`. Everything that produces `Vec<Vec<String>>` lives here.
- Root files (`search.rs`, `render.rs`, `import.rs`, `export.rs`, etc.) — implement process-layer traits with concrete I/O
- `handlers/` — **thin** HTTP handlers only: extract request → call process with injected dependencies → format response. No business logic. No adapter instantiation — composition provides all dependencies via `AppState`.
- `handlers/bulk_import.rs` — `POST /api/v1/admin/bulk-import/{table}`: PostgreSQL COPY-based bulk loader for post-wipe corpus releases (~50× faster than upsert import)

### Composition (`src/composition/`)

Wires everything together. The only layer that knows about all others.

- `mod.rs` — `build_app()`: registers CRUD resources, junctions, expansions, custom handlers
- `state.rs` — `AppState` with `DataStore<Entity, Query>` per entity and `DatabasePool`

## Error Flow

Errors flow through the layers with increasing specificity:

| Layer | Error responsibility |
|-------|---------------------|
| **Domain** | Pure types — no error creation, no error handling |
| **Logic** | Pure functions return `Result` or `Option` — no error creation, just propagation |
| **Process** | Propagates errors with `?` — never inspects adapter-specific details, never creates `DataSourceError` directly |
| **Adapters** | Create `HexforgeError` from concrete failures (e.g., map PostgreSQL error codes to semantic variants). HTTP handlers map `HexforgeError` to HTTP status codes. |
| **Composition** | No error handling — wiring only |

Error messages are sanitized at the adapter boundary — no table names, column names, or values are exposed to clients.

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

**3 internal entities:** ApiKey, BibitemNotes, DataVersion
