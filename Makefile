SHELL := /bin/bash
.PHONY: dev stop db lint test test-unit test-integration

# Start DB + Adminer and run the app
dev-start:
	docker compose up -d db adminer
	@echo "Waiting for Postgres to be ready..."
	@until docker compose exec db pg_isready -U $${POSTGRES_USER:-bib} > /dev/null 2>&1; do sleep 0.5; done
	@echo "Postgres ready"
	set -a && source .env && set +a && cargo run

# Stop all containers
dev-stop:
	docker compose down

# Stop all containers and remove volumes (data will be lost)
dev-purge:
	docker compose down -v

# Start only the database
dev-db:
	docker compose up -d db adminer

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

# List leaked test containers (if Ctrl+C interrupted tests)
# Review the list, then remove manually: docker rm -f <id>
list-leaked:
	@docker ps --filter label=org.testcontainers=true --format "table {{.ID}}\t{{.Image}}\t{{.CreatedAt}}\t{{.Names}}"
