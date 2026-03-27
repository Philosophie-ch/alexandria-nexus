SHELL := /bin/bash
.PHONY: dev stop db lint test

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

# Tests
test:
	cargo test-all
