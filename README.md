# Locus Desk

Locus Desk is a local-first, self-hosted workspace for notes and tasks.

The project is in early development. Its scope and architecture may change as the product takes shape. See the [design draft](docs/rust-svelte-mvp-design.md) for the current direction.

## Development

Requirements:

- Rust 1.97.1
- Node.js 24
- pnpm 11.22.0
- GNU Make

```bash
corepack enable
make install
make dev
```

The frontend runs at `http://127.0.0.1:5173` and proxies `/api` requests to the backend at `http://127.0.0.1:7310`.

## Checks

```bash
make check
make test
make build
```

## Docker

```bash
export APP_ADMIN_USERNAME=admin
export APP_ADMIN_PASSWORD='replace-with-a-strong-password'
make docker-up
```

Docker builds and Compose services use host networking. Application data is stored in the `locus-desk_locus-data` volume.
