# Architecture

Alexandria Nexus follows hexagonal architecture with four layers. Each layer has strict dependency rules: inner layers never depend on outer layers, and I/O is confined to the adapters layer.

## Layers

### Domain (`src/domain/`)

Pure types with no framework dependencies. Contains business types, entity structs, enums, and projections.

**Entities:**
- `Author` -- bibliography author with bib_string name fields (latex, unicode, simplified)
- `Journal` -- academic journal with ISSN identifiers
- `Publisher` -- publisher with default address
- `Institution` -- institutional affiliation with default address
- `School` -- school/university with default address
- `Series` -- publication series
- `Keyword` -- classification keyword with level (1, 2, 3)
- `BibItem` -- bibliography item with 46+ fields covering all BibTeX entry types

Each entity has corresponding `Create*` and `Update*` structs and transform functions.

**Enums:**
- `EntryType` -- BibTeX entry types (article, book, incollection, inproceedings, etc.)
- `PubState` -- publication state (unpub, forthcoming, inwork, submitted, published)
- `LangId` -- language identifier (18 languages + unknown)
- `Epoch` -- historical philosophical epoch (34 periods from presocratics to vienna-circle)
- `AuthorRole` -- role in a bibliography item (author, editor, guesteditor)
- `RefType` -- reference type between items (further_ref, depends_on)
- `PermissionLevel` -- API key access level (public, read, write, admin)

**Projections** (`src/domain/projections.rs`):
- `BibItemSummary` -- 6-field summary for list views (id, bibkey, entry_type, title_simplified, date_year, pubstate)
- `AuthorExpanded` -- name-only author for expansion responses
- `JournalExpanded`, `PublisherExpanded`, `InstitutionExpanded`, `SchoolExpanded`, `SeriesExpanded` -- key + name projections
- `KeywordExpanded` -- id, name, level
- `BibItemCrossref` -- identity + title for crossref expansion

### Logic (`src/logic/`)

Pure business logic. No HTTP types, no framework coupling. All functions accept domain types and return domain types or plain results.

- `validation/` -- input validation per entity (one file per entity, consumer-owned)
- `csv_parsing/` -- pure parsers for human-readable CSV fields (authors, dates, bibkeys, pages, keywords). Ported from the Python bib-sdk. Zero I/O, full unit test coverage.
- `full_import.rs` -- orchestration for human-readable CSV import pipeline:
  - `validate_full_csv()` -- parse CSV, batch-check all references, report missing/ambiguous/stale
  - `import_entities_from_full_csv()` -- create missing authors, journals, publishers, etc.
  - `import_full_csv()` -- resolve names to IDs, upsert bibitems + junctions, delete stale (CSV is source of truth)
- `import.rs` -- ID-based CSV import with bulk reference validation (for machine-generated CSVs)
- `export.rs` -- CSV formatting and relation data resolution (expanded vs. IDs format)
- `search.rs` -- full-text search query building using pg_trgm similarity
- `keyword_tree.rs` -- keyword hierarchy builder (groups by level 1/2/3)
- `render/` -- HTML bibliography renderer (formats entries by type with author/editor/year display)

### Adapters (`src/adapters/`)

Concrete I/O implementations. This is where framework types and database access live.

- `auth.rs` -- API key validator (hashes bearer tokens, checks permission level)
- `db/` -- database enum mappings (`sqlx::Type` derives) and query filter structs per entity
- `handlers/` -- HTTP handler functions (thin wrappers that parse requests, delegate to logic, and format responses):
  - `search.rs` -- `POST /api/v1/search`
  - `render.rs` -- `POST /api/v1/render`
  - `keyword_tree.rs` -- `GET /api/v1/keywords/tree`
  - `export.rs` -- CSV export handlers for all entities
  - `import.rs` -- ID-based CSV import handlers for all entities
  - `full_import.rs` -- human-readable CSV import handlers (validate, import-entities, import-bibitems)

### Composition (`src/composition/`)

Wires everything together. This is the only layer that knows about all other layers.

- `mod.rs` -- `build_app()` function that assembles the full router:
  - Registers all CRUD resources with validators, transforms, and permissions
  - Configures junction tables (bibitem_authors, bibitem_keywords) with extra columns
  - Sets up FK and junction expansions with projected types
  - Mounts custom handlers (search, render, keyword tree, import, export)
  - Configures OpenAPI spec and Swagger UI
  - Applies CORS
- `state.rs` -- `AppState` holding all `DataStore` instances and the database pool

## Code Generation

The project uses `hexforge.yml` as the single schema source of truth. The hexforge CLI reads this file and generates:

- Entity structs with derives
- Database enum type mappings
- Query filter structs
- DataStore instances in AppState
- Composition wiring in `build_app()`

**Consumer-owned files** are never overwritten by the generator:
- Validation functions (`src/logic/validation/`)
- Custom handlers (`src/adapters/handlers/`)
- Custom logic (`src/logic/search.rs`, `src/logic/export.rs`, etc.)

```bash
make generate-dry-run   # Preview what would be generated
make generate           # Regenerate source files (safe, idempotent)
make generate-migration # Regenerate SQL migration only
make generate-all       # Source + migration
```

## Data Model

**8 primary entities:** Author, Journal, Publisher, Institution, School, Series, Keyword, BibItem.

**3 junction tables:**
- `bibitem_authors` -- links BibItem to Author with extra columns `role` (author/editor/guesteditor) and `position` (ordering)
- `bibitem_keywords` -- links BibItem to Keyword with extra column `keyword_level`
- `bibitem_refs` -- links BibItem to BibItem with `ref_type` (further_ref/depends_on)

**7 foreign key relations on BibItem:**
- `journal_id` -> Journal
- `publisher_id` -> Publisher
- `institution_id` -> Institution
- `school_id` -> School
- `series_id` -> Series
- `crossref_id` -> BibItem (self-referencing)
- `person_id` -> Author (subject philosopher)

**2 internal entities:**
- `ApiKey` -- authentication keys with permission level, stored as hashed values
- `BibitemNotes` -- 1:1 JSONB workflow notes per bibitem
