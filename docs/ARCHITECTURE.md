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

**Handlers and process:** Handlers import process **types** (e.g., `ResolveResult`) and process **functions** (e.g., `render_bibitems_to_html`) to call process orchestration. Handlers are **thin HTTP adapters only**: extract request → forward to process → format response. They do not contain business logic and do not decide which adapter implementation to use — that is composition's job.

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

// COMPOSITION — the only layer that knows all others; wires concrete impls to process
// This is in build_app() or the router setup, NOT in the handler
process_fn(
    reader = PgReader::new(pool),
    writer = PgWriter::new(pool),
)

// HANDLER — thin HTTP adapter only: extract request → forward → format response
pub async fn my_handler(State(state): State<AppState>) {
    let report = process_fn(
        &PgReader::new(state.pool.pool()),
        &PgWriter::new(state.pool.pool()),
    ).await?;
    Ok(Json(report))
}
```

**Why separate read and write traits:** If you switch from Postgres to MongoDB, you only write `MongoReader` and `MongoWriter`. Process, logic, and domain are untouched. You could equally mix: `CsvReader` with `PgWriter`, or `ApiReader` with `ElasticSearchWriter`. They are fully swappable without touching any other layer.

**Why composition wires (not handlers):** Composition is the only layer that knows all others. Wiring decisions — which concrete adapter to use — belong there. Handlers know nothing about storage technologies; they receive the already-wired application state from composition and remain thin HTTP glue.

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
4. **COMPOSITION wires concrete implementations.** `process_fn(reader=PgReader::new(pool), writer=PgWriter::new(pool))` — this decision belongs in `build_app()`, not in handlers.
5. **Handlers are thin HTTP adapters only.** Extract request → call wired process → format response. Zero business logic. Zero wiring decisions.
6. **hexforge's `DataSource<T>` trait** is already an abstract contract. Process can use `&impl DataSource<T>` for standard CRUD operations without depending on adapters.

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

**CSV parsing does NOT live here.** Parsing an external format (CSV, JSON, XML) is a boundary concern — exactly like SQL. It belongs in the adapters layer. The logic layer only sees already-typed structs.

**Test**: if a function in `logic/` has `async` or imports anything from `adapters/`, `process/`, or `composition/`, it's in the wrong layer.

### Process (`src/process/`)

Orchestration. Defines **what** needs to happen via traits. Receives I/O as injected trait implementations.

- Defines traits for I/O operations not covered by hexforge's `DataSource`
- Uses `&impl DataSource<T>` for standard CRUD (find_by_id, insert, update)
- Uses custom traits for specialized operations (search, junction inserts, batch lookups)
- Zero concrete I/O: no `PgPool`, no `query()`, no `sqlx`, no SQL strings

**Test**: if a function in `process/` imports from `adapters/` or `composition/`, or uses `PgPool`/`sqlx`/`query()` directly, it's in the wrong layer.

### Adapters (`src/adapters/`)

Concrete I/O implementations. **All format-specific parsing (CSV, JSON, wire formats) lives here** — parsing external formats is a boundary concern, exactly like SQL queries. The process layer only sees typed structs; only adapters see raw bytes or format-specific APIs.

- `db/` — database enum mappings, query filters (all generated)
- `field_parsing/` — CSV field parsers for the full-CSV import pipeline
- Root files (`search.rs`, `render.rs`, `import.rs`, etc.) — implement process-layer traits with Postgres-specific code
- `handlers/` — **thin** HTTP handlers only: extract request → construct adapter impls → call process → format response. No business logic.

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

**3 internal entities:** ApiKey, BibitemNotes, DataVersion
