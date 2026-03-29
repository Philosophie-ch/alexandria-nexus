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
- `search.rs` -- full-text search query building using pg_trgm similarity
- `export.rs` -- CSV formatting and relation data resolution (expanded vs. IDs format)
- `import.rs` -- CSV parsing, reference validation, and batch insert logic
- `keyword_tree.rs` -- keyword hierarchy builder (groups by level 1/2/3)
- `renderer/` -- HTML bibliography renderer (formats entries by type with author/editor/year display)

### Adapters (`src/adapters/`)

Concrete I/O implementations. This is where framework types and database access live.

- `auth.rs` -- API key validator (hashes bearer tokens, checks permission level)
- `db/` -- database enum mappings (`sqlx::Type` derives) and query filter structs per entity
- `handlers/` -- HTTP handler functions (thin wrappers that parse requests, delegate to logic, and format responses):
  - `search.rs` -- `POST /api/v1/search`
  - `render.rs` -- `POST /api/v1/render`
  - `keyword_tree.rs` -- `GET /api/v1/keywords/tree`
  - `export/` -- CSV export handlers for all entities
  - `import/` -- CSV import handlers for all entities

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
make generate           # Generate code
make generate-migration # Generate code + SQL migration
```

## Data Model

**8 primary entities:** Author, Journal, Publisher, Institution, School, Series, Keyword, BibItem.

**2 junction tables:**
- `bibitem_authors` -- links BibItem to Author with extra columns `role` (author/editor/guesteditor) and `position` (ordering)
- `bibitem_keywords` -- links BibItem to Keyword with extra column `keyword_level`

**6 foreign key relations on BibItem:**
- `journal_id` -> Journal
- `publisher_id` -> Publisher
- `institution_id` -> Institution
- `school_id` -> School
- `series_id` -> Series
- `crossref_id` -> BibItem (self-referencing)

**1 internal entity:**
- `ApiKey` -- authentication keys with permission level, stored as hashed values
