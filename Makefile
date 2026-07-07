.DEFAULT_GOAL := help

# Local Postgres (matches docker-compose.yml). Override on the CLI if needed:
#   make test-int TEST_DATABASE_URL=postgres://...
DATABASE_URL      ?= postgres://cctf:cctf@localhost:5432/cctf
TEST_DATABASE_URL ?= $(DATABASE_URL)

.PHONY: help build build-server build-docs test test-int test-all check clippy fmt fmt-check run docs-dev db db-reset db-down docker docker-up logs nuke clean

help: ## Show this help
	@grep -hE '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-13s\033[0m %s\n", $$1, $$2}'

# ---- build ----
build: build-docs build-server ## Build the docs bundle + release server

build-server: ## Release build of the Rust server
	cargo build --release --locked

build-docs: ## Build the API docs into apidocs/dist (single self-contained file)
	cd apidocs && npm install && npm run build

# ---- test / lint ----
test: ## Unit tests + doctests (no database needed)
	cargo test

test-int: ## Postgres-gated integration tests (needs `make db` + a fresh schema)
	TEST_DATABASE_URL=$(TEST_DATABASE_URL) cargo test -- --ignored

test-all: ## Full suite: spin up DB, run pg+http integration + unit/doctests, then wipe the DB
	@set -e; \
	docker compose up -d --wait db; \
	trap 'code=$$?; docker compose down -v || true; exit $$code' EXIT; \
	TEST_DATABASE_URL=$(TEST_DATABASE_URL) cargo test --test pg -- --ignored; \
	TEST_DATABASE_URL=$(TEST_DATABASE_URL) cargo test --test http -- --ignored; \
	cargo test

check: fmt-check clippy ## fmt --check + clippy (warnings = errors)

clippy: ## Lint with clippy
	cargo clippy --all-targets -- -D warnings

fmt: ## Format the code
	cargo fmt

fmt-check: ## Verify formatting without changing files
	cargo fmt --check

# ---- run (dev) ----
run: ## Run the server (needs .env + `make db`)
	cargo run

docs-dev: ## Live docs viewer on :5173 (proxies the API on :8080)
	cd apidocs && npm run dev

# ---- database (docker) ----
db: ## Start Postgres in the background
	docker compose up -d db

db-reset: ## Wipe + recreate Postgres (drops the data volume)
	docker compose down -v && docker compose up -d db

db-down: ## Stop the compose stack
	docker compose down

# ---- docker (full image) ----
docker: ## Build the full container image
	docker compose build

docker-up: ## Build + run the full stack (detached)
	docker compose up --build -d

logs: ## Follow logs from the running stack (Ctrl-C to detach)
	docker compose logs -f

nuke: ## Tear down the full stack: containers, DB volume, and built image
	docker compose down -v --rmi local --remove-orphans

# ---- misc ----
clean: ## Remove build artifacts (target/, apidocs/dist, node_modules)
	cargo clean
	rm -rf apidocs/dist apidocs/node_modules
