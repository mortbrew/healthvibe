# healthvibe architecture and stack

Status: **draft for review** (Nov 3, 2026). **Decision: healthvibe is written in Rust.** This supersedes the earlier Python/FastAPI plan.

## Stack

| Concern | Choice | Alternative considered |
|---|---|---|
| Web backend | [Axum](https://github.com/tokio-rs/axum) on Tokio, `tower`/`tower-http` middleware (compression, timeouts, request limits, tracing) | Actix Web |
| Frontend | **SPA built to static files** (Svelte or React, **choice open**, see [Frontend](#frontend-spa)), served by Axum via `tower-http` `ServeDir` or embedded with `rust-embed`, with an `index.html` fallback for client-side routes | — |
| Charts | Bundled in the SPA (ECharts works with either framework; see comparison). No CDN | — |
| API | JSON REST, versioned `/api/v1` (see [API](#api)); OpenAPI via `utoipa` (optional) | — |
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

## Frontend (SPA)

**Decided:** a single-page app built to static files and served by the Axum backend. **Open:** Svelte or React.

The frontend owns the visuals: activity rings, sleep-stage charts, body composition, heart, dark mode and themes (CSS variables), the theme browser UI, the Shortcut setup wizard and QR codes (rendered client-side, e.g. `qrcode` npm package, from a server-issued URL). Rust owns the data layer and the JSON API, plus auth sessions, MCP, ingest and the theme store.

### Svelte vs React (neutral comparison)

| | Svelte (SvelteKit `adapter-static`, or Vite + Svelte) | React (Vite + React) |
|---|---|---|
| Bundle size | Smaller: compiler, very small runtime (framework ~few KB gz) | Larger: React + ReactDOM ~45 KB gz baseline |
| Charts / rings | ECharts (works with both, via a thin wrapper); d3 directly; LayerCake (Svelte-native, composable SVG); custom SVG rings are trivial | ECharts (`echarts-for-react` or thin wrapper); Recharts (simple, popular); visx (d3 primitives, flexible); custom SVG rings are trivial |
| Theming | CSS variables work natively; scoped styles built in | CSS variables work natively; styling via CSS modules, Tailwind or CSS-in-JS (pick one) |
| Routing / structure | SvelteKit gives routing and layouts out of the box (static adapter, SPA fallback) | Needs a router (React Router or TanStack Router) and data library (TanStack Query) |
| Team fit | Easy to learn, less boilerplate; fewer devs and AI-generated examples; smaller ecosystem | Most widely known; largest ecosystem, component libraries, examples and hiring pool; more patterns to agree on |
| Long-term maintenance | Svelte 5 (runes) changed the model in 2024, so older examples are outdated; fewer dependencies overall | Very stable API, huge community; more dependencies to keep updated (router, query, chart libs) |
| TypeScript | Supported | Supported |

Both build with Vite to static files, both fit the same Axum serving and API, and both use the same CSS-variable theme settings, so the choice doesn't affect the backend.

## API

- Versioned under **`/api/v1`**, JSON only (`serde`), consistent error body `{ "error": { "code", "message" } }`, cursor pagination for time series, ISO 8601 timestamps with offsets.
- **SPA auth:** login sets an `HttpOnly`, `Secure`, `SameSite=Lax` session cookie (tower-sessions). State-changing requests need a **CSRF token** (fetched from `GET /api/v1/session`, sent in an `X-CSRF-Token` header and checked against the session). Same-origin only, no CORS.
- **Shortcut and MCP** use bearer tokens (`upload`, `mcp` scopes), not cookies, so CSRF doesn't apply to them.
- Main groups: `auth`/`session`, `me` and settings, `metrics` (enabled list), `readings` and daily summaries (rings, sleep, heart, body), `import` (export.xml upload, job status), `shortcut` (config, claim, setup QR URLs, ingest), `themes` (catalog, installed, download, delete, mapped settings), `admin` (users, invites).
- **OpenAPI (optional):** `utoipa` derives an OpenAPI spec from the Rust handlers and types; the frontend can generate a typed client from it (e.g. `openapi-typescript`), keeping both sides in sync.

## Workspace layout (proposed)

```
Cargo.toml            # workspace
crates/
  healthvibe/         # the binary: Axum app, JSON API, static SPA serving, MCP, importer
  hv-core/            # models, sqlx queries, dedupe hashing, sleep score
  hv-themes/          # theme store + palette → CSS mapping (theming.md)
migrations/           # sqlx migrations
frontend/             # SPA (Svelte or React) + Vite; `npm run build` → frontend/dist
assets/               # signed .shortcut
```

## One binary, several routes

`healthvibe` serves the built SPA, the JSON API (`/api/v1/*`), the Shortcut endpoints (`/api/v1/shortcut/*`, `/api/v1/ingest`), the MCP server (`/mcp`), and background jobs (stale-upload alerts, theme catalog refresh) from one process. Token scopes keep them apart: an `upload` token can only read its own Shortcut config and write readings; an `mcp` token can only read; sessions are for the web UI. (Open question 1 below.)

## Testing and CI

- Frontend: `npm run lint`, type check (`svelte-check` or `tsc`), unit tests with Vitest, `npm run build`; Playwright smoke test against the built image later.
- `cargo test` (unit + integration tests with a temp SQLite DB and synthetic data generator), `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- `SQLX_OFFLINE=true` in CI using the committed `.sqlx/` query data.
- Tests prove per-user isolation (one user can never read another's data) and idempotent imports.

## Deployment (Serenity)

- Docker Compose, container runs as `1000:1000`, **no published LAN ports** (Newt/Pangolin reaches `http://healthvibe:8000` on the internal network), data in `./appdata` (SQLite DB, per-user themes), secrets in `.env` (session key, SMTP, optional GitHub token for the theme catalog, optional `APPRISE_API_URL`).
- Multi-stage Dockerfile:
  ```dockerfile
  FROM node:22-alpine AS web
  WORKDIR /web
  COPY frontend/package*.json ./
  RUN npm ci
  COPY frontend/ .
  RUN npm run build            # → /web/dist

  FROM rust:1-alpine AS build
  RUN apk add --no-cache musl-dev
  WORKDIR /src
  COPY . .
  COPY --from=web /web/dist frontend/dist   # embedded by rust-embed (or copied below for ServeDir)
  ENV SQLX_OFFLINE=true
  RUN cargo build --release --locked --target x86_64-unknown-linux-musl

  FROM gcr.io/distroless/static-debian12:nonroot
  COPY --from=build /src/target/x86_64-unknown-linux-musl/release/healthvibe /healthvibe
  USER 1000:1000
  EXPOSE 8000
  ENTRYPOINT ["/healthvibe"]
  ```
  Three stages: node builds the SPA, rust builds the static musl binary, the final image holds the binary plus static assets (embedded via `rust-embed`, or `COPY --from=web /web/dist /static` and served with `ServeDir`). Expected image ~20–30 MB.
- Healthcheck: `/healthvibe healthcheck` subcommand (distroless has no curl); log rotation in Compose.

## History and superseded work

- Linux Admin Bot's earlier receiver in `/workspace/health` on the shared box is a **Python receiver built for Health Auto Export**. It is a reference only (Compose conventions, store-raw-then-parse, token checks); none of its code is used.
- **PR #1 (Python/FastAPI scaffold) is superseded.** It needs to be replaced with a Rust scaffold (workspace, Axum hello page, sqlx + first migration, Dockerfile, Compose, CI with cargo test/clippy/fmt, synthetic data generator). Toby decides whether to close it.
- The build plan's later PRs map 1:1 onto Rust: models/migrations (sqlx), auth (argon2 + tower-sessions), theming, importer (quick-xml), ingest endpoint, Shortcut setup + QR, dashboard pages, MCP (rmcp), docs.

## Open questions for Toby

1. **Shortcut upload receiver:** fold it into the main binary (**recommended**: one binary, separate route and `upload` token scope, shared DB and dedupe code) or run it as a separate small service?
2. **PR #1:** close it and redo the scaffold in Rust?
3. **Frontend framework:** Svelte (SvelteKit static adapter or Vite + Svelte) or React (Vite + React)? See the comparison above; Toby wants to discuss.
