# Thin wrappers around the commands CI runs, so local and CI behaviour cannot drift.
COMPOSE := docker compose -f deploy/compose.yml
COMPOSE_FULL := $(COMPOSE) -f deploy/compose.full.yml --profile full
CARGO_DOCKER := docker run --rm -v "$(CURDIR)":/w -w /w \
	-v pqcas-cargo:/usr/local/cargo/registry -v pqcas-target:/w/target \
	rust:1.90-slim bash -c

.PHONY: help dev dev-full down logs ps seed test test-rust test-web test-go lint fmt e2e build deploy clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

dev: ## Build and start the lab stack (OpenSSL + Go adapters)
	$(COMPOSE) up -d --build
	@echo "web: http://localhost:$${HTTP_PORT:-8088}/en"

dev-full: ## Same, plus the BoringSSL and wolfSSL adapters (slow first build)
	$(COMPOSE_FULL) up -d --build

down: ## Stop everything
	$(COMPOSE_FULL) down

logs: ## Tail service logs
	$(COMPOSE) logs -f --tail=100

ps: ## Show service status
	$(COMPOSE) ps

seed: ## Re-run the idempotent seeders
	$(COMPOSE) run --rm api-seed

test: test-rust test-go test-web ## Run every unit test suite

test-rust: ## cargo test for the workspace
	$(CARGO_DOCKER) 'export PATH=/usr/local/cargo/bin:$$PATH; cargo test --workspace'

test-web: ## vitest for the Next.js app
	pnpm --filter web test

test-go: ## go test for the Go adapter
	cd adapters/go && go test ./...

lint: ## clippy + eslint + gofmt + fmt check
	$(CARGO_DOCKER) 'export PATH=/usr/local/cargo/bin:$$PATH; cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings'
	pnpm --filter web lint
	@test -z "$$(cd adapters/go && gofmt -l .)" || (echo "gofmt needed in adapters/go" && exit 1)

fmt: ## Apply formatting
	$(CARGO_DOCKER) 'export PATH=/usr/local/cargo/bin:$$PATH; cargo fmt --all'
	cd adapters/go && gofmt -w .

e2e: ## Playwright against a running stack
	pnpm --filter web e2e

build: ## Build every image without starting anything
	$(COMPOSE_FULL) build

vectors: ## Regenerate the CAVP answer keys and the algorithm catalog
	cd tools && npm install --silent && node gen-algorithms.mjs && node gen-cavp-vectors.mjs

clean: ## Remove containers, volumes and build caches
	$(COMPOSE_FULL) down -v
	docker volume rm -f pqcas-cargo pqcas-target
