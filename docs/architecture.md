# healthvibe architecture and stack

Status: **draft for review** (Nov 3, 2026). **Decision: healthvibe is written in Rust.** This supersedes the earlier Python/FastAPI plan.

## Stack

| Concern | Choice | Alternative considered |
|---|---|---|
| Web backend | [Axum](https://github.com/tokio-rs/axum) on Tokio, `tower`/`tower-http` middleware (compression, timeouts, request limits, tracing) | Actix Web |
| Frontend | **SPA built to static files** **Vite + React + TypeScript** (decided, see [Frontend](#frontend-spa)), served by Axum via `tower-http` `ServeDir` or embedded with `rust-embed`, with an `index.html` fallback for client-side routes | — |
| Charts | Bundled in the SPA (ECharts works with either framework; see comparison). No CDN | — |
| API | JSON REST, versioned `/api/v1` (see [API](#api)); OpenAPI via `utoipa` (optional) | — |
| Database | SQLite (WAL) via [sqlx](https://github.com/launchbadge/sqlx): compile-time checked queries (`query!`, offline `.sqlx` data in the repo for CI) and `sqlx migrate` migrations | rusqlite |
| Auth | `argon2` password hashing, [tower-sessions](https://github.com/maxcountryman/tower-sessions) with a SQLite session store, CSRF tokens on forms; first user is admin, invite links | — |
| API tokens | Random 256-bit, stored hashed, scoped `upload` or `mcp`, one upload token per user | — |
| JSON | `serde` / `serde_json`, validated request structs | — |
| export.xml import | [quick-xml](https://github.com/tafia/quick-xml) streaming reader (constant memory, >1 GB files), batched inserts, same content-hash dedupe as uploads | — |
| MCP server | Read-only, in Rust with the official [rmcp](https://github.com/modelcontextprotocol/rust-sdk) crate, **streamable HTTP** transport at `/mcp`, per-user `mcp` token, no write tools; also exposes logged events for correlation ([Event logging](#event-logging-and-ai-insights)) | — |
| Theme store | Rust module `themes` in the same workspace, reusing VibeKit's approach ([theming.md](theming.md)) | — |
| QR codes | `qrcode` crate rendering SVG | — |
| Alerts | SMTP via `lettre`; ntfy via HTTP; Apprise via optional apprise-api sidecar | — |
| HTTP client (themes, ntfy) | `reqwest` (rustls) | ureq |
| Logging | `tracing` + `tracing-subscriber` (JSON logs) | — |

## Frontend (SPA)

**Decided (Nov 3, 2026):** **Vite + React + TypeScript**, built to static files and served by the Axum backend.

The frontend owns the visuals: activity rings, sleep-stage charts, body composition, heart, dark mode and themes (CSS variables), the theme browser UI, the Shortcut setup wizard and QR codes (rendered client-side, e.g. `qrcode` npm package, from a server-issued URL). Rust owns the data layer and the JSON API, plus auth sessions, MCP, ingest and the theme store.

**Why React (over Svelte):** the most widely known framework with the largest ecosystem (chart libraries, component libraries, examples), a very stable API, and the best fit for the coding team and AI-assisted development. Svelte's smaller bundles matter little for a self-hosted dashboard.

### Proposed defaults

| Concern | Proposed |
|---|---|
| Build | Vite, TypeScript (`strict`) |
| Routing | React Router (data router: `createBrowserRouter`, loaders) |
| API data | TanStack Query (caching, refetch, mutations); typed client generated from the OpenAPI spec if `utoipa` is used |
| Charts | ECharts via `echarts-for-react` (or a thin wrapper) for sleep stages, trends, heart, body composition |
| Activity rings | Custom SVG component (three arcs, overflow laps) |
| Themes / dark mode | CSS variables set from the API's theme settings; `prefers-color-scheme` for Follow system |
| Tests | Vitest + React Testing Library; ESLint + Prettier |

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
frontend/             # Vite + React + TypeScript; `npm run build` → frontend/dist
assets/               # signed .shortcut
```

## One binary, several routes

`healthvibe` serves the built SPA, the JSON API (`/api/v1/*`), the Shortcut endpoints (`/api/v1/shortcut/*`, `POST /api/v1/ingest`), the MCP server (`/mcp`), and background jobs (stale-upload alerts, theme catalog refresh) from one process. Token scopes keep them apart: an `upload` token can only read its own Shortcut config and write readings; an `mcp` token can only read; sessions are for the web UI. (Open question 1 below.)

## Testing and CI

- Frontend: `npm run lint`, type check (`tsc --noEmit`), Vitest + React Testing Library, `npm run build`; Playwright smoke test against the built image later.
- `cargo test` (unit + integration tests with a temp SQLite DB and synthetic data generator), `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- `SQLX_OFFLINE=true` in CI using the committed `.sqlx/` query data.
- Tests prove per-user isolation (one user can never read another's data) and idempotent imports.

### Importer test fixtures (real export.xml)

Synthetic data doesn't cover what real exports contain: enum quirks, duplicate/overlapping timestamps, `InBed` vs stage sleep samples (`AsleepCore/Deep/REM`, `Awake`), odd units (`count/min`, `%`, `kcal` vs `Cal`, `mL/(kg·min)`), metadata keys, device/source strings and the inline DTD. **Toby will provide his real `export.xml`** as a fixture for the streaming quick-xml parser. The repo is **public**, so:

- **The raw file is never committed.** It stays local in `tests/fixtures/private/` (gitignored; CI also fails if anything under that path is tracked). Tests using it are `#[ignore]` or gated on `HEALTHVIBE_REAL_EXPORT=/path/to/export.xml` and run only locally.
- **Sanitizer:** `hv-cli sanitize-export <in> <out>` produces a small committed fixture, `tests/fixtures/export-sample.xml`, that keeps the structure, record types, units, metadata keys and duplicate/overlap patterns, but:
  - shifts all dates by one random offset (relative timing preserved),
  - adds slight noise to values,
  - replaces device/source names and serials with generic ones (`iPhone`, `Apple Watch`, `Source A`…),
  - drops `Me` (DOB, sex, blood type), workout GPS routes, ECG and clinical records,
  - caps the output to a few days.
- **Toby reviews the sanitized file before it's committed.**
- Toby may choose to override this plan.

## Deployment

- Docker Compose, container runs as `1000:1000`, **proxy-agnostic**: by default publishes **no ports** and joins an external proxy network the user configures (`HEALTHVIBE_PROXY_NETWORK`), with an option to bind `127.0.0.1:PORT` instead; any reverse proxy terminates TLS (see [reverse-proxy.md](reverse-proxy.md)), data in `./appdata` (SQLite DB, per-user themes), secrets in `.env` (session key, SMTP, optional GitHub token for the theme catalog, optional `APPRISE_API_URL`).
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

## Public URL and proxies

- `HEALTHVIBE_PUBLIC_URL` (e.g. `https://health.example.com`): canonical base for QR codes, the Shortcut config, emails and MCP client instructions.
- `HEALTHVIBE_TRUSTED_PROXIES` (CIDR list): `X-Forwarded-For/Proto/Host` are honored only from these peers; otherwise ignored.
- MCP is exposed at `<public URL>/mcp` through whatever proxy is used; nothing in the app assumes a specific proxy.
- The admin setup step checks scheme, host and forwarded headers and warns if something is off ([reverse-proxy.md](reverse-proxy.md#setup-wizard-check)).

## Ingest endpoint and upload tokens (resolved)

**Decided (Nov 3, 2026):** one fixed ingest path per deployment, **`POST /api/v1/ingest`**. No per-user subdomains, path IDs or tokens in the path.

- Each user's secret travels **only** in `Authorization: Bearer <token>`; the server maps the token to the user. This keeps tokens out of access logs, browser history and referrers.
- Tokens: random 256-bit, format `hvu_<8-char prefix>_<secret>`. The DB stores the prefix (indexed) and a SHA-256 hash of the full token (high-entropy tokens don't need a slow hash). Lookup by prefix, then a **constant-time compare** of the hash (`subtle`); unknown or revoked tokens get the same 401 with no timing difference.
- The Shortcut sets the header in its one **Get Contents of URL** action.
- **Rate limiting:** per token and per client IP (from trusted-proxy headers), e.g. with `tower_governor`; failed auth attempts count against the IP.
- **QR code 2** carries only the public URL and the one-time setup code: `healthvibe://setup?server=<HEALTHVIBE_PUBLIC_URL>&code=<setup code>`. The claim returns the token.
- **Token reset:** issuing a new token (new claim) revokes the old one; the URL never changes.

## Event logging and AI insights

Users log **events** next to their health readings in the web UI: supplements (e.g. magnesium nightly before sleep), medications, dose amounts, and start/change dates (e.g. "started a GLP-1 on Jan 1"). Events live in SQLite, scoped per user, so the read-only MCP server can correlate them with readings.

**What it enables**

- Before/after comparisons around a start date: weight, sleep score/quality, resting HR, HRV, etc.
- Occurrence correlation: do magnesium nights line up with better healthvibe sleep scores?
- AI insights (via an MCP client) that surface trends like "weight dropped and resting HR improved after starting X on date Y". healthvibe supplies the data and simple stats; it does not give medical advice, and the UI says so.

**Schema (proposed)**

```sql
CREATE TABLE events (
  id          INTEGER PRIMARY KEY,
  user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,                 -- "Magnesium glycinate", "Semaglutide"
  type        TEXT NOT NULL CHECK (type IN ('supplement','medication','other')),
  dose_amount REAL,                          -- optional
  dose_unit   TEXT,                          -- "mg", "IU", "ml"
  start_date  TEXT NOT NULL,                 -- ISO date
  end_date    TEXT,                          -- NULL = ongoing
  notes       TEXT,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

-- Dose changes keep history instead of overwriting the row
CREATE TABLE event_changes (
  id             INTEGER PRIMARY KEY,
  event_id       INTEGER NOT NULL REFERENCES events(id) ON DELETE CASCADE,
  effective_date TEXT NOT NULL,
  dose_amount    REAL,
  dose_unit      TEXT,
  notes          TEXT,
  created_at     TEXT NOT NULL
);

-- Optional: per-day/night logging for as-needed items
CREATE TABLE event_occurrences (
  id          INTEGER PRIMARY KEY,
  event_id    INTEGER NOT NULL REFERENCES events(id) ON DELETE CASCADE,
  occurred_at TEXT NOT NULL,                 -- timestamp, or the night's date
  dose_amount REAL,
  dose_unit   TEXT,
  notes       TEXT
);
```

Open design point: track dose changes in `event_changes` (preferred: one event, full history) or as new `events` rows (simpler, but splits the history).

**Proposed read-only MCP tools**

| Tool | Purpose |
|---|---|
| `list_events` | The user's events with dose history, filterable by type/date |
| `get_event_occurrences` | Logged occurrences for an as-needed event in a range |
| `compare_before_after` | For an event and metric(s): mean/median/trend over N days before vs. after start (or a dose change) |
| `correlate_event_with_metric` | Nights/days with vs. without an occurrence against a metric (e.g. sleep score), with sample sizes |
| `event_timeline` | Events and change points overlaid on a metric series, for charting/summaries |

Every tool is scoped to the token's user and stays read-only. Events are created and edited only in the web UI.

**Privacy**

Medication and supplement data is sensitive health data. It stays in the local SQLite database, is never sent anywhere by healthvibe, and appears in MCP results only for the token's own user. Tests use synthetic events only; no real event or medication data is ever committed to the public repo.

## History and superseded work

- Linux Admin Bot's earlier receiver in `/workspace/health` on the shared box is a **Python receiver built for Health Auto Export**. It is a reference only (Compose conventions, store-raw-then-parse, token checks); none of its code is used.
- **PR #1 (Python/FastAPI scaffold) is superseded.** It needs to be replaced with a Rust scaffold (workspace, Axum hello page, sqlx + first migration, Dockerfile, Compose, CI with cargo test/clippy/fmt, synthetic data generator). Toby decides whether to close it.
- The build plan's later PRs map 1:1 onto Rust: models/migrations (sqlx), auth (argon2 + tower-sessions), theming, importer (quick-xml), ingest endpoint, Shortcut setup + QR, dashboard pages, MCP (rmcp), docs.

## Resolved

- **Frontend framework:** React (Vite + React + TypeScript), Nov 3, 2026.
- **Ingest URL:** single fixed `POST /api/v1/ingest`, token only in the `Authorization` header, Nov 3, 2026 ([details](#ingest-endpoint-and-upload-tokens-resolved)).

## Open tasks

- **Real export.xml fixture:** Toby provides his export (kept local, gitignored); build `hv-cli sanitize-export`; Toby reviews `tests/fixtures/export-sample.xml` before it's committed. See [Importer test fixtures](#importer-test-fixtures-real-exportxml).
- **Calibrate the sleep score formula:** Toby provides screenshots of Apple's Sleep Score from his iPhone as ground truth. Compare them against healthvibe's calculated score across 15–25 different nights and iteratively adjust the formula (likely a weighted blend of duration, stage percentages, consistency/bedtime regularity, and interruptions) to match Apple as closely as possible. The current weights (duration 50 / bedtime 30 / interruptions 20, [shortcut-design §8](shortcut-design.md#8-sleep-score)) are only a starting point. Screenshots and real sleep data are never committed to this public repo: keep them local and gitignored (e.g. `tests/fixtures/private/`); if anything is recorded, only anonymized nightly score pairs (Apple vs. healthvibe), after Toby's review.
- **Event logging and AI insights:** build the events/event_changes/event_occurrences tables, the web UI for logging, and the read-only MCP tools (`list_events`, `compare_before_after`, `correlate_event_with_metric`, ...). Decide dose-change modelling. Synthetic data only. See [Event logging and AI insights](#event-logging-and-ai-insights).

## Open questions for Toby

1. **Shortcut upload receiver:** fold it into the main binary (**recommended**: one binary, separate route and `upload` token scope, shared DB and dedupe code) or run it as a separate small service?
2. **PR #1:** close it and redo the scaffold in Rust?
