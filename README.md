# Locus Desk

Locus Desk is a local-first, self-hosted workspace for Markdown memos, personal tasks, and saved articles. The Svelte frontend is embedded in the Rust binary, so production only needs one executable and one writable data directory.

Current features:

- Single-owner password authentication and workspace-isolated sessions.
- Markdown memo CRUD, search, tags, pinning, and archive.
- An all-open Todo rail and a full task view with priority, date, optional time, completion, and recovery.
- A Library with URL deduplication, saved selections and notes, searchable article content, and persistent background capture with retries.
- An article reader with a table of contents and adjustable font, text size, line spacing, and width.
- Article refresh that keeps saved content available and asks whether to accept a substantially shorter replacement.
- Sanitized GFM rendering and responsive desktop/mobile workspaces.
- Consistent SQLite backup, portable JSON/Markdown export, and safe restore commands.

Saved article text can be read without contacting the source website, but the browser still needs
access to the Locus Desk service. Article images may load from external websites and are not cached
for offline reading. Production builds include a web app manifest and service worker for browser
installation where supported. The service worker caches only the application HTML shell, not its
scripts, styles, or API data; it does not provide full offline access.

Portable exports include saved reader content; SQLite backups also retain source snapshots and
content versions. Phase 1 remains in progress: browser-extension authentication, offline images,
and the 50-site acceptance run are still planned.

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

# Set owner credentials before starting an empty database.
export APP_ADMIN_USERNAME=admin
export APP_ADMIN_PASSWORD='replace-with-a-strong-password'
export APP_DATA_DIR=./var/dev
make dev
```

The credentials are only required for an empty database. Later starts ignore them and preserve the stored owner account.

The frontend runs at `http://127.0.0.1:5173` and proxies `/api` requests to the backend at `http://127.0.0.1:7310`. The backend defaults to this loopback-only address unless `APP_BIND` is set explicitly.

## Configuration

[env.example](env.example) lists the application and development settings. `make dev` reads
application variables from the shell; the backend does not automatically load `.env` files.
To keep local settings in a file, copy `env.example` to the Git-ignored `.env`, edit the credentials
and other values, then load it before starting:

```bash
set -a
. ./.env
set +a
make dev
```

Use shell-compatible assignments and quote values containing spaces or shell metacharacters.

| Variable | Default | Purpose |
| --- | --- | --- |
| `APP_ENV` | `development` | `development`, `test`, or `production`. |
| `APP_BIND` | `127.0.0.1:7310` | Backend listen address. |
| `APP_DATA_DIR` | `./var/dev` in development; `./var/test` in tests | Data directory; an explicit absolute path is required in production. |
| `APP_TIMEZONE` | `Asia/Singapore` | Workspace timezone as an IANA timezone name. |
| `APP_ADMIN_USERNAME` | None | Owner username for the first start. |
| `APP_ADMIN_PASSWORD` | None | Owner password for the first start. |
| `APP_COOKIE_SECURE` | `false` | Use secure session cookies when accessed through HTTPS. |
| `VITE_DEV_PORT` | `5173` | Frontend development port. |
| `VITE_API_TARGET` | `http://127.0.0.1:7310` | Development API proxy target; update it when changing the backend address. |
| `RUST_LOG` | `info` | Backend logging filter. |

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

Run these commands from the repository root after `make build`, as the service account. Backup and
export use `APP_DATA_DIR`; set it explicitly to the directory of the instance you intend to operate
on. Output commands never overwrite an existing artifact.

```bash
export APP_DATA_DIR=/srv/locus-desk/data

# SQLite snapshot in APP_DATA_DIR/backups
./target/release/locus-desk backup
./target/release/locus-desk backup before-upgrade.sqlite3

# Portable files in APP_DATA_DIR/exports
./target/release/locus-desk export json
./target/release/locus-desk export markdown locus-desk.md

# Restore into a new absolute data directory (or an empty existing directory already at 0700)
./target/release/locus-desk restore /srv/locus-desk/data/backups/before-upgrade.sqlite3 /srv/locus-desk-restored
```

Restore uses the supplied backup and target paths. To serve the restored instance, start the binary
with `APP_DATA_DIR` pointing to the restored directory.

SQLite backups include authentication and session data for complete recovery. Each snapshot records
its creation time, application version, Git commit, and schema version. Restore verifies the exact
embedded migration history and schema shape. Default manual and pre-migration snapshots are retained
independently as seven daily and four older weekly backups; the newly created snapshot is protected
from clock rollback, while custom filenames and invalid managed-looking files are never pruned.
Portable exports intentionally exclude password hashes, sessions, and internal database IDs.

Run `./target/release/locus-desk --help` for the command summary and `./target/release/locus-desk --version` for application, commit, and schema versions.

## Docker

### Published image

GitHub Actions builds the Linux amd64 image and publishes it to
`ghcr.io/abowloflrf/locus-desk`. Pushes to `main` update `latest`; every published build also
has a `sha-<full-commit>` tag. Git tags matching `v*` publish a matching image tag. Pull requests
build the image without publishing it. Formatting, type checks, and tests remain available
through the local `make check` and `make test` commands.

```bash
export APP_ADMIN_USERNAME=admin
export APP_ADMIN_PASSWORD='replace-with-a-strong-password'
docker pull ghcr.io/abowloflrf/locus-desk:latest
docker run --detach --name locus-desk \
  --publish 127.0.0.1:7310:7310 \
  --volume locus-data:/data \
  --env APP_ADMIN_USERNAME \
  --env APP_ADMIN_PASSWORD \
  --restart unless-stopped \
  ghcr.io/abowloflrf/locus-desk:latest
```

Open `http://127.0.0.1:7310`. For HTTPS through a reverse proxy, also pass
`--env APP_COOKIE_SECURE=true` when creating the container.

### Build locally

```bash
export APP_ADMIN_USERNAME=admin
export APP_ADMIN_PASSWORD='replace-with-a-strong-password'
make docker-up
```

Docker builds and Compose services use host networking. Application data is stored in the `locus-desk_locus-data` volume.

Compose requires both owner credential variables on every invocation, even after the database is
initialized; the application still preserves the stored account. Compose defaults to
`APP_COOKIE_SECURE=false`; set it to `true` when serving the application through HTTPS.

`make docker-build` and `make docker-up` pass the current Git commit into the image so `--version` and `/api/v1/health` identify the source build.
