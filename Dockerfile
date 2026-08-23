# syntax=docker/dockerfile:1

FROM node:24-bookworm-slim AS web-builder

WORKDIR /app/web

RUN corepack enable \
    && corepack prepare pnpm@11.22.0 --activate

COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY web/ ./
RUN pnpm build

FROM rust:1.97.1-bookworm AS rust-builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN cargo fetch --locked

COPY migrations/ ./migrations/
COPY --from=web-builder /app/web/dist/ ./web/dist/

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 locus-desk \
    && useradd --uid 10001 --gid locus-desk --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin locus-desk \
    && mkdir --parents /data \
    && chown locus-desk:locus-desk /data

COPY --from=rust-builder /app/target/release/locus-desk /usr/local/bin/locus-desk

ENV APP_ENV=production \
    APP_BIND=0.0.0.0:7310 \
    APP_DATA_DIR=/data \
    RUST_LOG=info

USER locus-desk
VOLUME ["/data"]
EXPOSE 7310

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl --fail --silent --show-error http://127.0.0.1:7310/api/v1/health || exit 1

ENTRYPOINT ["locus-desk"]
