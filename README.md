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

## Development

```bash
make dev-start    # Starts Postgres + Adminer, runs the app
make dev-stop     # Stops all containers
make dev-purge    # Stops and removes all containers + volumes (data loss!)
make dev-db       # Starts only Postgres + Adminer (no app)
make check        # Format, lint, audit, and build
make test         # Full test suite
```

The app reads all config from environment variables. In dev, `make dev-start` sources `.env` before running. No defaults, every required var must be set.

## Endpoints

- `GET /health`: health check
- `GET /docs/openapi.json`: OpenAPI spec
- Adminer (DB UI): `http://localhost:{ADMINER_PORT}`

## Environment variables

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

PolyForm Noncommercial 1.0.0 — Copyright Philosophie.ch
