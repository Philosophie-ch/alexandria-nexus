# Hexforge SDK Usage

Practical reference for how Alexandria Nexus uses the hexforge SDK. For architectural rules and layer boundaries, see [ARCHITECTURE.md](ARCHITECTURE.md). For type safety and coding standards, see [CODING_STANDARDS.md](CODING_STANDARDS.md).

## Principle: leverage hexforge maximally

Always use hexforge's built-in abstractions before writing custom consumer code. Custom code is only justified when hexforge genuinely cannot express the operation. Before writing a workaround, check whether hexforge already provides a derive, builder method, trait, or helper that covers the need. If it should but doesn't, ask for a fix upstream rather than working around it in Alexandria.

## What hexforge provides out of the box

| hexforge abstraction | What it does | Where Alexandria uses it |
|---|---|---|
| `DataSource<T>` trait | Abstract CRUD contract (find, insert, update, delete, stream, count) | Process traits can use `&impl DataSource<T>` instead of defining their own |
| `DataStore<T, Q>` | Concrete PostgreSQL implementation of `DataSource<T>` | Adapters — one per entity, pre-wired in `AppState` |
| `DataStore::fetch_all(query)` | Fetch all matching rows with no pagination limit | `snapshot.rs` entity/notes fetches; `keyword_tree.rs` |
| `DataStore::fetch_all_sorted(query, order)` | `fetch_all` with custom `ORDER BY` via `SortOrder` | `keyword_tree.rs` (level, name ordering) |
| `DataStore::find_by_similarity(search, filters, pagination, fallback)` | pg_trgm fuzzy search + `COUNT(*) OVER()` in one query; returns `(Vec<T>, i64)` | `adapters/search.rs` |
| `TextSearch::on(fields, query).with_threshold(f)` | Abstract fuzzy text search spec; pg_trgm internally | `adapters/search.rs` |
| `SortOrder::by(field).then(field).then_desc(field)` | Sort spec without exposing SQL syntax | `keyword_tree.rs`, `snapshot.rs`, export helpers |
| `#[derive(Entity, Crud)]` | Generates SQL mapping for entity types | All 8 domain entities |
| `#[derive(Filter)]` | Generates query filter types | All entity query structs in `db/queries/` |
| `#[derive(Projection)]` | Generates column subsets for `find_all_projected` | `BibItemSummary` and expansion projections |
| `impl_db_enum!` | Adds sqlx encode/decode to domain enums without polluting domain | `adapters/db/db_mappings.rs` |
| `WhereClause` | SQL WHERE builder without exposing sqlx types | Adapter impls that need ad-hoc conditions |
| `crud_auto()` | Registers all CRUD routes + OpenAPI schema from type params | `composition/mod.rs` — 8 entities |
| `.lookup_by(column)` | Auto `GET /by-key/{key}` route with full `?expand` and `?expand=all` support | All 8 entities in `composition/mod.rs` |
| `.junction(JunctionConfig)` | Auto junction CRUD routes | `bibitem_authors`, `bibitem_keywords` |
| `.expand_fk_projected` | Declarative FK expansion with `?expand=` | Author, journal, publisher, etc. expansions |
| `.expand_junction_projected_by_key` | String-key junction expansion with optional enum-safe role filter | `bibitem_authors` (role), `bibitem_keywords` |
| `.view("summary", ...)` | Projection dispatch via `?view=` | BibItemSummary view |
| `HexforgeError` | Unified error type for process and adapter layers | All process functions return this |

## When to define a custom process trait vs. use DataSource directly

Use `&impl DataSource<T>` (hexforge's built-in abstract trait) when the operation is standard CRUD: find by ID, list with filters, insert, update, delete. No custom trait needed.

Define a custom process trait when the operation is outside what `DataSource` provides:
- Full-text / fuzzy search (pg_trgm — `BibitemSearcher`)
- Junction table inserts with conflict handling (`BibitemJunctionStore`)
- Bulk transactional import / delete (`NameVariantStore`, `ReferenceStore`)
- LaTeX batch conversion with unnest updates (`LatexColumnFetcher`, `LatexColumnWriter`)
- Multi-table LEFT JOINs (e.g., citation resolution — `CitationResolver`)

## Raw SQL policy in adapters

Adapters use raw SQL only where hexforge abstractions genuinely cannot express the operation.

**Justified raw SQL** (no hexforge equivalent):
- `unnest($1::int8[], $2::text[])` batch updates — `adapters/latex_columns.rs`
- Complex LEFT JOINs with grouping — `adapters/latex_citations.rs`
- Bulk transactional import: batch deletes, dependency traversal — `adapters/full_import.rs`
- PostgreSQL `array_append`, sequence resets, junction inserts with conflict handling — `adapters/import.rs`
- Junction snapshot fetches with PostgreSQL enum casts in SELECT — `adapters/snapshot.rs`
- API key lookup (intentionally narrow, runs on every request) — `adapters/auth.rs`

**Replaced by hexforge abstractions**:
- pg_trgm similarity + `COUNT(*) OVER()` → `DataStore::find_by_similarity` + `TextSearch`
- `SELECT * FROM keywords ORDER BY level, name` → `DataStore::fetch_all_sorted(&SortOrder::by("level").then("name"))`
- `SELECT * FROM {entity} ORDER BY id` (snapshot) → `DataStore::fetch_all`
- JOIN-by-integer-ID junction fetches → generated `fetch_{junction}_by_owner_ids` functions in `adapters/db/queries/junctions.rs`

## Dependency setup

Alexandria uses hexforge via git dep (patched to local path during dev):

```toml
hexforge = { git = "ssh://...", tag = "v0.2.0", features = ["axum", "postgres"] }

# Local dev override:
[patch."ssh://git@gitlab.com/alebg/hexforge.git"]
hexforge = { path = "../../../GitRepos/hexforge" }
```

## Escape-hatch re-exports

For scenarios beyond standard CRUD, hexforge re-exports framework types so consumers don't need direct dependencies:

| Module | Provides | When to use |
|--------|----------|-------------|
| `db_exports` | `PgPool`, `FromRow`, `Row`, `Encode`, `Decode`, `Type`, `PgHasArrayType`, `query_as`, `query`, `Transaction`, `Arguments` | Custom SQL queries, junction row types, raw DB access |
| `axum_exports` | `Router`, `Json`, `Path`, `State`, `StatusCode`, routing functions | Custom handlers beyond CRUD |
| `serde_exports` | `Serialize`, `Deserialize`, `serde_json`, `json!` | Serialization without adding serde to Cargo.toml |
| `async_exports` | `async_trait` | Implementing async traits |

```rust
use hexforge::db_exports::{PgPool, query, query_as, FromRow};
use hexforge::axum_exports::{Json, State};
```

## When to fix in hexforge vs Alexandria

- **Boilerplate that repeats across consumers** → fix in hexforge (new derive, builder method, etc.)
- **Domain-specific logic** → stays in Alexandria (search, export, render, validation)
- **Architecture violation** → fix wherever it is
- **Framework leak** → ALWAYS fix in hexforge
