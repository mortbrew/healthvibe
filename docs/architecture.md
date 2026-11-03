# healthvibe architecture and stack

Status: **draft for review** (Nov 3, 2026). **Decision: healthvibe is written in Rust.** This supersedes the earlier Python/FastAPI plan.

## Stack

| Concern | Choice | Alternative considered |
|---|---|---|
| Web backend | [Axum](https://github.com/tokio-rs/axum) on Tokio, `tower`/`tower-http` middleware (compression, timeouts, request limits, tracing) | Actix Web |
| HTML | Server-rendered templates with [askama](https://github.com/askama-rs/askama) (compile-time checked) | minijinja (runtime templates) |
| Interactivity | HTMX (vendored, served locally) | — |
| Charts | Apache ECharts (vendored, no CDN); colors read from the CSS theme settings | — |
| Database | SQLite (WAL) via [sqlx](https://github.com/launchbadge/sqlx): compile-time checked queries (`query!`, offline `.sqlx` data in the repo for CI) and `sqlx migrate` migrations | rusqlite |
| Auth | `argon2` password hashing, [tower-sessions](https://github.com/maxcountryman/tower-sessions) with a SQLite session store, CSRF tokens on forms; first user is admin, invite links | — |
| API tokens | Random 256-bit, stored hashed, scoped `upload` or `mcp`, one upload token per user | — |
| JSON | `serde` / `serde_json`, validated request structs | — |
| export.xml import | [quick-xml](https://github.com/tafia/quick-xml) streaming reader (constant memory, >1 GB files), batched inserts, same content-hash dedupe as uploads | — |
| MCP server | Read-only, in Rust with the official [rmcp](https://github.com/modelcontextprotocol/rust-sdk) crate, **streamable HTTP** transport at `/mcp`, per-user `mcp` token, no write tools | — |
| Theme store | Rust module `themes` in the same workspace, reusing VibeKit's approach ([theming.md](theming.md)) | — |
| QR codes | `qrcode` crate rendering SVG | — |
| Alerts | SMTP via `lettre`; ntfy via HTTP; Apprise via optional apprise-api sidecar | — |
| HTTP client (themes, ntfy) | `reqwest` (rustls) | ureq |
| Logging | `tracing` + `tracing-subscriber` (JSON logs) | — |

## Workspace layout (proposed)

```
Cargo.toml            # workspace
crates/
  healthvibe/         # the binary: Axum app, routes, templates, MCP, importer
  hv-core/            # models, sqlx queries, dedupe hashing, sleep score
  hv-themes/          # theme store + palette → CSS mapping (theming.md)
migrations/           # sqlx migrations
templates/            # askama templates
assets/               # vendored htmx.min.js, echarts.min.js, CSS, signed .shortcut
```

## One binary, several routes

`healthvibe` serves the web UI, the Shortcut endpoints (`/api/v1/shortcut/*`, `/api/v1/ingest`), the MCP server (`/mcp`), and background jobs (stale-upload alerts, theme catalog refresh) from one process. Token scopes keep them apart: an `upload` token can only read its own Shortcut config and write readings; an `mcp` token can only read; sessions are for the web UI. (Open question 1 below.)

## Testing and CI

- `cargo test` (unit + integration tests with a temp SQLite DB and synthetic data generator), `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- `SQLX_OFFLINE=true` in CI using the committed `.sqlx/` query data.
- Tests prove per-user isolation (one user can never read another's data) and idempotent imports.

## Deployment (Serenity)

- Docker Compose, container runs as `1000:1000`, **no published LAN ports** (Newt/Pangolin reaches `http://healthvibe:8000` on the internal network), data in `./appdata` (SQLite DB, per-user themes), secrets in `.env` (session key, SMTP, optional GitHub token for the theme catalog, optional `APPRISE_API_URL`).
- Multi-stage Dockerfile:
  ```dockerfile
  FROM rust:1-alpine AS build
  RUN apk add --no-cache musl-dev
  WORKDIR /src
  COPY . .
  ENV SQLX_OFFLINE=true
  RUN cargo build --release --locked --target x86_64-unknown-linux-musl

  FROM gcr.io/distroless/static-debian12:nonroot
  COPY --from=build /src/target/x86_64-unknown-linux-musl/release/healthvibe /healthvibe
  USER 1000:1000
  EXPOSE 8000
  ENTRYPOINT ["/healthvibe"]
  ```
  Templates and assets are embedded in the binary (askama compiles templates in; `rust-embed` for assets), so the image is just the static binary (~15–25 MB).
- Healthcheck: `/healthvibe healthcheck` subcommand (distroless has no curl); log rotation in Compose.

## History and superseded work

- Linux Admin Bot's earlier receiver in `/workspace/health` on the shared box is a **Python receiver built for Health Auto Export**. It is a reference only (Compose conventions, store-raw-then-parse, token checks); none of its code is used.
- **PR #1 (Python/FastAPI scaffold) is superseded.** It needs to be replaced with a Rust scaffold (workspace, Axum hello page, sqlx + first migration, Dockerfile, Compose, CI with cargo test/clippy/fmt, synthetic data generator). Toby decides whether to close it.
- The build plan's later PRs map 1:1 onto Rust: models/migrations (sqlx), auth (argon2 + tower-sessions), theming, importer (quick-xml), ingest endpoint, Shortcut setup + QR, dashboard pages, MCP (rmcp), docs.

## Open questions for Toby

1. **Shortcut upload receiver:** fold it into the main binary (**recommended**: one binary, separate route and `upload` token scope, shared DB and dedupe code) or run it as a separate small service?
2. **PR #1:** close it and redo the scaffold in Rust?
