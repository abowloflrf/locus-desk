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
	cd web && pnpm exec concurrently --kill-others --names api,web --prefix-colors green,blue "cargo run --manifest-path ../Cargo.toml" "vite"

fmt:
	cargo fmt
	cd web && pnpm format

check:
	cargo fmt --check
	cargo clippy --all-targets --all-features -- -D warnings
	cd web && pnpm format:check
	cd web && pnpm check

test:
	cargo test

build:
	cd web && pnpm build
	cargo build --release

docker-build:
	docker build --network=host --tag locus-desk:local .

docker-up:
	docker compose --file docker-compose.yml up --build

docker-down:
	docker compose --file docker-compose.yml down
