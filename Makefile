SHELL := /bin/bash
.PHONY: dev stop db lint test test-unit test-integration

# Build DATABASE_URL from the compose env vars
DB_URL = postgres://$${POSTGRES_USER}:$${POSTGRES_PASSWORD}@localhost:$${DB_PORT:-5433}/$${POSTGRES_DB}

# Start app on its own (DB must already be running)
dev-raw:
	set -a && source .env && set +a && DATABASE_URL=$(DB_URL) cargo run --bin alexandria-nexus

# Build the app image (requires SSH agent for hexforge private dep)
dev-build:
	DOCKER_BUILDKIT=1 docker compose build app

# Start DB + Adminer + app (build image first with: make dev-build)
dev-start:
	docker compose up -d

# Stop all containers
dev-stop:
	docker compose down

# Stop all containers and remove volumes (data will be lost)
dev-purge:
	docker compose down -v --remove-orphans

# Start only the database
dev-db:
	docker compose up -d alexandria-db adminer

# checks
check:
	cargo fmt --all && cargo lint && cargo audit && cargo build

# Unit tests only (fast, no Docker)
test-unit:
	cargo test --lib

# Integration tests (uses Docker — one test at a time to save resources)
test-integration:
	cargo test --test '*' -- --test-threads=1

# All tests
test:
	cargo test --lib && cargo test --test '*' -- --test-threads=1

# Code generation from hexforge.yml
generate:
	cargo run --bin generate -- generate --schema hexforge.yml --output . --source-only

generate-migration:
	cargo run --bin generate -- generate --schema hexforge.yml --output . --migration-only --migration-overwrite

generate-all:
	cargo run --bin generate -- generate --schema hexforge.yml --output . --migration-overwrite

generate-dry-run:
	cargo run --bin generate -- generate --schema hexforge.yml --output . --dry-run

# List leaked test containers (if Ctrl+C interrupted tests)
# Review the list, then remove manually: docker rm -f <id>
list-leaked:
	@docker ps --filter label=org.testcontainers=true --format "table {{.ID}}\t{{.Image}}\t{{.CreatedAt}}\t{{.Names}}"
