.DEFAULT_GOAL := help

.PHONY: help install dev fmt check test build docker-build docker-up docker-down

help:
	@echo "Locus Desk development commands"
	@echo "  make install       Install frontend dependencies"
	@echo "  make dev           Start the Axum and Vite development servers"
	@echo "  make fmt           Format Rust and frontend sources"
	@echo "  make check         Run backend and frontend static checks"
	@echo "  make test          Run the current test suite"
	@echo "  make build         Build the release binary with embedded frontend assets"
	@echo "  make docker-build  Build the container image with host networking"
	@echo "  make docker-up     Build and start the Compose service"
	@echo "  make docker-down   Stop the Compose service"

install:
	cd web && pnpm install --frozen-lockfile

dev:
	web/node_modules/.bin/concurrently --kill-others --names api,web --prefix-colors green,blue "cargo run --locked" "pnpm --dir web exec vite"

fmt:
	cargo fmt
	cd web && pnpm format

check:
	cargo fmt --check
	cargo clippy --all-targets --all-features --locked -- -D warnings
	cd web && pnpm format:check
	cd web && pnpm check

test:
	cargo test --all-targets --locked
	cd web && pnpm test

build:
	cd web && pnpm build
	cargo build --release --locked

docker-build:
	docker build --network=host --build-arg LOCUS_GIT_COMMIT="$$(git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)" --tag locus-desk:local .

docker-up:
	LOCUS_GIT_COMMIT="$$(git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)" docker compose --file docker-compose.yml up --build -d

docker-down:
	docker compose --file docker-compose.yml down
