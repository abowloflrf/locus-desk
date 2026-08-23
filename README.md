# Locus Desk

Locus Desk is a local-first, self-hosted workspace for Markdown notes and daily tasks. The Svelte frontend is embedded in the Rust binary, so production only needs one executable and one writable data directory.

Phase 0 includes:

- Single-owner password authentication and workspace-isolated sessions.
- Markdown note CRUD, search, tags, pinning, and archive.
- Today and full task views with priority, date, optional time, completion, and recovery.
- Sanitized GFM rendering and responsive desktop/mobile workspaces.
- Consistent SQLite backup, portable JSON/Markdown export, and safe restore commands.

See the [living design document](docs/rust-svelte-mvp-design.md) for the product boundaries and architecture.

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

The frontend runs at `http://127.0.0.1:5173` and proxies `/api` requests to the backend at `http://127.0.0.1:7310`. The backend defaults to this loopback-only address unless `APP_BIND` is set explicitly.

`make dev` reads application variables from the shell. Export at least the owner credentials before the first start:

```bash
export APP_ADMIN_USERNAME=admin
export APP_ADMIN_PASSWORD='replace-with-a-strong-password'
export APP_DATA_DIR=./var/dev
make dev
```

The credentials are only required for an empty database. Later starts ignore them and preserve the stored owner account.

## Checks

```bash
make check
make test
make build
```

`make test` runs both Rust and frontend tests. `make build` creates `target/release/locus-desk` with the current frontend embedded.

## Production

Production requires an absolute data directory. Create it as the service account with owner-only
permissions before the first start:

```bash
install -d -m 0700 /srv/locus-desk/data
```

If the directory already exists, verify its owner and run `chmod 0700 /srv/locus-desk/data`.
Locus Desk refuses a filesystem root, a symbolic link, or an existing data
directory with any group/other permission; it never changes permissions on an existing top-level
directory.

Start the service with:

```bash
APP_ENV=production \
APP_BIND=0.0.0.0:7310 \
APP_DATA_DIR=/srv/locus-desk/data \
APP_TIMEZONE=Asia/Singapore \
APP_ADMIN_USERNAME=admin \
APP_ADMIN_PASSWORD='replace-with-a-strong-password' \
APP_COOKIE_SECURE=true \
./target/release/locus-desk
```

Production deployments must set `APP_BIND` explicitly when the service should accept non-loopback traffic. The container configuration uses `0.0.0.0:7310` explicitly.

Set `APP_COOKIE_SECURE=true` when browsers reach Locus Desk through HTTPS; mutation requests must
then carry a matching HTTPS `Origin`. Plain HTTP with secure cookies disabled is appropriate only
for localhost or a trusted private network.

The application stores SQLite at `APP_DATA_DIR/db/locus-desk.sqlite3`. Managed subdirectories use
mode `0700`, and generated database, backup, and export files use mode `0600` on Unix. It creates a
consistent backup in `APP_DATA_DIR/backups` before applying any later schema migration. Protect the
entire data directory and all backup files as sensitive data.

## Data operations

Data commands use `APP_DATA_DIR` and never overwrite an existing artifact:

```bash
# SQLite snapshot in APP_DATA_DIR/backups
./locus-desk backup
./locus-desk backup before-upgrade.sqlite3

# Portable files in APP_DATA_DIR/exports
./locus-desk export json
./locus-desk export markdown notes-and-tasks.md

# Restore into a new absolute data directory (or an empty existing directory already at 0700)
./locus-desk restore /srv/locus-desk/data/backups/backup.sqlite3 /srv/locus-desk-restored
```

SQLite backups include authentication and session data for complete recovery. Each snapshot records
its creation time, application version, Git commit, and schema version. Restore verifies the exact
embedded migration history and schema shape. Default manual and pre-migration snapshots are retained
independently as seven daily and four older weekly backups; the newly created snapshot is protected
from clock rollback, while custom filenames and invalid managed-looking files are never pruned.
Portable exports intentionally exclude password hashes, sessions, and internal database IDs.

Run `./locus-desk --help` for the command summary and `./locus-desk --version` for application, commit, and schema versions.

## Docker

```bash
export APP_ADMIN_USERNAME=admin
export APP_ADMIN_PASSWORD='replace-with-a-strong-password'
make docker-up
```

Docker builds and Compose services use host networking. Application data is stored in the `locus-desk_locus-data` volume.

`make docker-build` and `make docker-up` pass the current Git commit into the image so `--version` and `/api/v1/health` identify the source build.
