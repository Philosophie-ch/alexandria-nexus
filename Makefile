SHELL := /bin/bash
.PHONY: setup dev stop db lint test test-unit test-integration

# Install required development tools (run once after cloning)
setup:
	cargo install cargo-audit
	cargo install cargo-geiger

# Build DATABASE_URL from the compose env vars
DB_URL = postgres://$${POSTGRES_USER}:$${POSTGRES_PASSWORD}@localhost:$${DB_PORT:-5433}/$${POSTGRES_DB}

# Start app on its own (DB must already be running)
dev-raw:
	set -a && source .env && set +a && DATABASE_URL=$(DB_URL) cargo run --bin alexandria-nexus

# Build the app image (requires SSH agent for hexforge private dep)
dev-build:
	DOCKER_BUILDKIT=1 docker compose build app

# Build a fresh release binary, start the stack, and hot-swap the binary in.
dev-start:
	docker compose up -d
	cargo build --release --bin alexandria-nexus
	docker cp target/release/alexandria-nexus alexandria-nexus:/app/alexandria-nexus
	docker compose restart app

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
	@echo "=== unsafe dependency audit ===" && \
	UNSAFE_CRATES=$$(cargo geiger --all-features 2>/dev/null | grep -E "^\S.*!  " | sed 's/.*!  //' | sed 's/[│├└─]//g' | sed 's/^ *//' | grep -oP '^[a-zA-Z][a-zA-Z0-9_-]+' | sort -u) && \
	ALLOWED=$$(grep -v '^\s*#' .geiger-allow | grep -v '^\s*$$' | sort -u) && \
	NEW=$$(comm -23 <(echo "$$UNSAFE_CRATES") <(echo "$$ALLOWED")) && \
	if [ -n "$$NEW" ]; then \
		echo "FAIL: unapproved unsafe dependencies detected — add to .geiger-allow with justification:"; \
		echo "$$NEW"; \
		exit 1; \
	else \
		echo "OK: all unsafe dependencies are explicitly approved"; \
	fi

# Unit tests only (fast, no Docker)
test-unit:
	cargo test --lib -j 2

# Integration tests (uses Docker — one test at a time to save resources)
test-integration:
	cargo test --test '*' -j 2 -- --test-threads=1

# All tests
test:
	cargo test --lib -j 2 && cargo test --test '*' -j 2 -- --test-threads=1

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
