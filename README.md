# Alexandria Nexus

Bibliography and knowledge engine for [Philosophie.ch](https://philosophie.ch). Built on [hexforge](https://gitlab.com/alebg/hexforge).

## Prerequisites

- Rust 1.94+
- Docker + Docker Compose
- Make

## Setup

```bash
# Copy environment config
cp .env.example .env
# Edit .env with your values (all required, app fails if missing)
```

## Development Workflow

```bash
make dev-start          # Start DB + Adminer, run the app
make dev-stop           # Stop all containers
make dev-purge          # Stop + delete all containers and volumes (data loss!)
make dev-db             # Start only DB + Adminer (no app)
make check              # Format, lint, audit, build
make test               # Full test suite (unit + integration)
make test-unit          # Unit tests only (fast, no Docker)
make test-integration   # Integration tests (Docker, sequential)
make generate           # Generate code from hexforge.yml
make generate-migration # Generate code + SQL migration
make generate-dry-run   # Preview generation (no writes)
make list-leaked        # Show leaked test containers
```

The app reads all config from environment variables. In dev, `make dev-start` sources `.env` before running. No defaults -- every required var must be set.

## Architecture

Alexandria Nexus follows hexagonal architecture with four layers:

```
domain/         -- pure types, entities, projections, enums
logic/          -- pure business logic (validation, search, export, import, renderer)
adapters/       -- I/O (HTTP handlers, DB queries, auth)
composition/    -- wiring (build_app, AppState)
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for details.

## API Endpoints

### CRUD Resources

All CRUD resources follow the same pattern: `GET` (list), `GET /:id` (by id), `GET /by-key/:key` (by key, where applicable), `POST` (create), `PUT /:id` (update), `DELETE /:id` (delete).

| Resource | Base Path | Query Filters | By-Key Field |
|---|---|---|---|
| Authors | `/api/v1/authors` | `family_name`, `search_term` | `author_key` |
| Journals | `/api/v1/journals` | `name` | `journal_key` |
| Publishers | `/api/v1/publishers` | `name` | `publisher_key` |
| Institutions | `/api/v1/institutions` | `name` | `institution_key` |
| Schools | `/api/v1/schools` | `name` | `school_key` |
| Series | `/api/v1/series` | `name` | `series_key` |
| Keywords | `/api/v1/keywords` | `level`, `name` | -- |
| BibItems | `/api/v1/bibitems` | `entry_type`, `year_from`, `year_to`, `author_id`, `journal_id`, `epoch`, `search_term` | `bibkey` |

### BibItem Extras

**Projection views:**
- `?view=summary` -- returns `BibItemSummary` (6 fields: id, bibkey, entry_type, title_simplified, date_year, pubstate) instead of the full entity.

**Expansion:**
- `?expand=journal,authors,editors,guesteditors,keywords,publisher,institution,school,series,crossref`
- `?expand=all` -- expands all relations at once.

**Streaming:**
- `?stream=true` -- returns NDJSON (newline-delimited JSON) streaming response.

**Junction routes** (many-to-many management):
- `GET /api/v1/bibitems/:id/authors` -- list junction entries
- `POST /api/v1/bibitems/:id/authors` -- add junction entries
- `PUT /api/v1/bibitems/:id/authors` -- replace junction entries
- `DELETE /api/v1/bibitems/:id/authors` -- remove junction entries
- Same pattern for `/api/v1/bibitems/:id/keywords`

### Custom Endpoints

| Method | Path | Description | Permission |
|---|---|---|---|
| `POST` | `/api/v1/search` | Full-text search (pg_trgm similarity) | Read |
| `POST` | `/api/v1/render` | HTML bibliography renderer (max 1000 items) | Public |
| `GET` | `/api/v1/keywords/tree` | Keywords grouped by level (1, 2, 3) | Read |

### Admin Endpoints

All admin endpoints require Admin permission.

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/v1/admin/import/{entity}` | CSV import (multipart file upload) |
| `POST` | `/api/v1/admin/export/{entity}` | CSV export (JSON body, returns CSV) |

Supported entities: `authors`, `journals`, `publishers`, `institutions`, `schools`, `series`, `keywords`, `bibitems`.

See [docs/IMPORT_EXPORT.md](docs/IMPORT_EXPORT.md) for CSV column formats and usage.

### Documentation and Health

| Method | Path | Description |
|---|---|---|
| `GET` | `/docs/openapi.json` | OpenAPI 3.0 spec |
| `GET` | `/docs` | Swagger UI |
| `GET` | `/health` | Health check (returns `OK`) |

## Authentication

API keys are passed as Bearer tokens:

```
Authorization: Bearer <api_key>
```

### Permission Levels

| Level | Access |
|---|---|
| Public | No authentication required |
| Read | `GET` endpoints (list, get by id, get by key, search, keyword tree) |
| Write | `POST` (create), `PUT` (update) |
| Admin | `DELETE`, import, export |

CRUD resources use `CrudPermissions::standard()`: Read for list/get, Write for create/update, Admin for delete.

### Dev API Key

Set `SEED_API_KEY` and `SEED_API_KEY_NAME` in `.env` to seed an admin-level API key on startup.

## Query Parameters

### Pagination

Offset-based:
```
?page=1&per_page=20
```

Cursor-based:
```
?cursor=xxx&limit=20
```

### Expansion

Expand related entities inline (BibItems only):
```
?expand=journal,authors
?expand=all
```

### Projection Views

Return a reduced field set (BibItems only):
```
?view=summary
```

### Streaming

NDJSON streaming response (BibItems only):
```
?stream=true
```

### Filters

Entity-specific query parameters. Examples:
```
?family_name=kant&search_term=philosophy    # Authors
?entry_type=article&year_from=2000          # BibItems
?epoch=vienna-circle&journal_id=5           # BibItems
?level=1                                    # Keywords
?name=oxford                                # Publishers, Journals, etc.
```

## Code Generation

The project uses `hexforge.yml` as the schema source of truth. The hexforge CLI generates entities, queries, DB mappings, state, and composition wiring. Consumer-owned files (validation, handlers) are preserved across regeneration.

```bash
make generate-dry-run   # Preview what would be generated
make generate           # Generate code
make generate-migration # Generate code + SQL migration
```

## Environment Variables

### Required (app)

| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | PostgreSQL connection string |
| `HOST` | Bind address (e.g., `0.0.0.0`) |
| `PORT` | Bind port (e.g., `8080`) |
| `ALLOWED_ORIGINS` | Comma-separated CORS origins |

### Optional (app)

| Variable | Description |
|----------|-------------|
| `SEED_API_KEY` | Seed an admin API key on startup |
| `SEED_API_KEY_NAME` | Name for the seeded key (required if `SEED_API_KEY` is set) |
| `RUST_LOG` | Log filter (default: `alexandria_nexus=debug,hexforge=debug`) |

### Docker Compose only

| Variable | Description |
|----------|-------------|
| `POSTGRES_USER` | DB user |
| `POSTGRES_PASSWORD` | DB password |
| `POSTGRES_DB` | DB name |
| `DB_PORT` | Exposed Postgres port |
| `ADMINER_PORT` | Exposed Adminer port |

## License

PolyForm Noncommercial 1.0.0 -- Copyright Philosophie.ch
