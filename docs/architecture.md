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

### Multi-LLM support (bring your own key)

Insights can use almost any LLM: Gemini, Grok, ChatGPT/OpenAI, Claude, or any OpenAI-compatible endpoint, including local Ollama.

- **Bring your own key.** healthvibe ships no keys and never provides them. Keys must **not** be persisted in the database or the repo. *Open question:* a server-side call still needs the key somewhere at run time. Options include per-session in memory only (entered in the UI and dropped at logout or restart), or operator-supplied via `.env` for single-user or trusted installs.
- **Model quality.** The app warns against very small models, including small Ollama models. It recommends a minimum tier: large context window, reliable tool calling, and strong reasoning. This stays a *recommendation* shown in the UI and docs, not a hard-coded list of model names that would go stale.
- **Provider abstraction.** A thin Rust trait keeps each provider small to add:

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &str;                       // "openai", "anthropic", "gemini", "xai", "openai-compatible"
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse>; // messages + tool definitions
    fn supports_tools(&self) -> bool;
}
```

  `ChatRequest` carries messages and tool schemas; `ChatResponse` returns text and/or tool calls. Most providers map onto one OpenAI-compatible implementation, plus native adapters where needed (Anthropic, Gemini).

**How the read-only MCP server fits — two paths**

1. **External MCP clients** (e.g. Grok Bot, Claude Desktop) connect directly to `<public URL>/mcp` with the user's `mcp` token. healthvibe makes no LLM call; the client's own model does the reasoning.
2. **In-app insights.** healthvibe's backend calls the user's chosen provider and gives it the **same read-only tools** (`list_events`, `compare_before_after`, `correlate_event_with_metric`, ...). Both paths share one tool implementation in `hv-core`, so behaviour and user scoping are identical. Only the aggregated or needed results of tool calls are sent, never a bulk dump of raw readings.

**Privacy:** with a cloud provider, the health and medication data in tool results **leaves the server**. In-app insights are off by default. The user must explicitly opt in per provider, and the UI says what is sent and where. A local Ollama endpoint keeps the data on the user's own network.

### Scheduled insights (background runs)

**Decision (Toby):** insights run in the background on a **user-created schedule** (daily, weekly, or custom cron-like), not only on demand. This lets users cap LLM token use. A manual **"Run now"** button is optional.

```sql
CREATE TABLE insight_schedules (
  id              INTEGER PRIMARY KEY,
  user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  cron            TEXT NOT NULL,          -- presets "daily"/"weekly" map to cron; custom allowed
  provider        TEXT NOT NULL,          -- LlmProvider id
  model           TEXT NOT NULL,
  scope           TEXT NOT NULL,          -- JSON: metrics/events/date window to analyse
  max_tokens      INTEGER,                -- optional per-run token cap
  notify          INTEGER NOT NULL DEFAULT 0,
  enabled         INTEGER NOT NULL DEFAULT 1,
  last_run        TEXT,
  next_run        TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

CREATE TABLE insight_runs (
  id          INTEGER PRIMARY KEY,
  schedule_id INTEGER REFERENCES insight_schedules(id) ON DELETE SET NULL, -- NULL = manual run
  user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  started_at  TEXT NOT NULL,
  finished_at TEXT,
  provider    TEXT NOT NULL,
  model       TEXT NOT NULL,
  tokens_in   INTEGER,
  tokens_out  INTEGER,
  status      TEXT NOT NULL CHECK (status IN ('running','ok','error','capped')),
  error       TEXT,
  findings_md TEXT                        -- markdown findings, reviewed in the web UI
);
```

- **Scheduler:** in-process in the main binary (e.g. `tokio-cron-scheduler`), loaded from enabled `insight_schedules` at startup and reloaded on edit. Each job runs the same shared tool layer as MCP. If the per-run token cap is hit, the run stops and is saved as `capped`.
- **Results:** stored in `insight_runs` and listed in an "Insights" page in the web UI for later review.
- **Notification:** optional per schedule, sent through the existing missed-upload alert channels (email / ntfy / apprise).

**Key supply (updates the BYOK open question):** scheduled runs fire with no user session, so the key must be available server-side when a job fires. **Per-session in-memory keys therefore don't work for scheduled runs**; they would only work for "Run now". Options (**resolved toward option 2**, with option 1 as an optional fallback; see [Provider-neutral insight memory](#provider-neutral-insight-memory)):

1. **Operator `.env` per provider** (e.g. `HEALTHVIBE_OPENAI_API_KEY`): simplest, never in the DB, but shared by all users of the instance.
2. **Per-user key encrypted at rest** (~~master key from `.env`~~ superseded: split-key DEK design, see [Encryption](#encryption-of-keys-and-insight-memory-resolved)). This is true BYOK per user, but it relaxes the earlier "never persisted in the DB" rule (encrypted only, never in plaintext or the repo). The keys are lost if the master key is lost.

### Provider-neutral insight memory

**Decision (Toby):** API keys are stored **per user, encrypted**. This resolves the key-supply question above toward option 2. An operator `.env` key per provider stays as an optional fallback, e.g. for single-user installs.

AI history lives in a **local encrypted file per user**, next to that user's encrypted keys:

```
appdata/users/<id>/memory.enc
appdata/users/<id>/keys.enc
```

It is **not** a raw chat transcript tied to one LLM. It is a structured, versioned, provider-neutral record. A user who switches providers mid-year (e.g. Gemini → Grok) feeds the same file to the new provider and keeps full context. This fixes the problem of every API call starting fresh.

```json
{
  "schema_version": 1,
  "user_id": 1,
  "updated_at": "2026-11-03T12:00:00Z",
  "summary": "Compact rolling summary of what is known so far…",
  "insights": [
    { "id": "ins_…", "text": "Resting HR down ~4 bpm since starting X", "metrics": ["restingHeartRate"],
      "date_range": ["2026-01-01", "2026-03-31"], "confidence": "medium",
      "provider": "gemini", "model": "…", "created_at": "…", "superseded_by": null }
  ],
  "correlations": [
    { "id": "cor_…", "event": "magnesium", "metric": "sleep_score", "effect": "+3.1 avg",
      "evidence": { "n_with": 42, "n_without": 38, "method": "mean diff" },
      "date_range": ["…", "…"], "provider": "xai", "model": "…", "created_at": "…" }
  ],
  "questions": [ { "id": "q_…", "text": "Does caffeine timing affect deep sleep?", "status": "open", "created_at": "…" } ],
  "conclusions": [ { "id": "con_…", "text": "…", "based_on": ["ins_…", "cor_…"], "created_at": "…" } ],
  "feedback": [ { "target": "ins_…", "rating": "useful|wrong|irrelevant", "note": "…", "created_at": "…" } ]
}
```

Every entry records the **provider and model** that produced it, plus timestamps. `schema_version` allows migrations.

**How it is used**

- **Prompt injection each run:** the compact `summary`, plus entries relevant to the run's scope (matching metrics, events, date window, open questions, recent feedback). The whole file is never sent.
- **MCP:** a read-only `get_insight_memory` tool (filters: metric, event, date range, kind) lets external clients and in-app runs fetch more on demand.
- **Write-back:** after a run, new insights, correlations and conclusions are merged in. Contradicted entries are marked `superseded_by`, not deleted.
- **Compaction:** when the file grows past a budget, older or superseded entries are folded into `summary` (an LLM or rule-based pass), so injected context stays within the model's window.

**Loss and backups:** a lost key means lost keys and lost memory. There are no backups in v1 (as decided), and the UI and docs warn about this.

### Encryption of keys and insight memory (resolved)

**Decision (Toby):** a best-practice split-key design that mirrors Bitwarden's CLI/background auth. *Details to be expanded later.*

- **Data key (DEK):** each user gets a random 256-bit DEK. It encrypts `keys.enc` and `memory.enc`. **Recommended cipher: XChaCha20-Poly1305**, with `age` as an alternative; the final pick is made at implementation.
- **Interactive unlock:** the DEK is wrapped by a key derived from the user's **password via argon2id** (per-user salt). At login the server unwraps the DEK into memory for the session.
- **Background unlock (scheduler token):** at setup the user generates a separate, long-lived **scheduler token** once.
  - **Narrow scope:** it can only run scheduled insights. That means read-only health data plus writing insight results and memory. No settings, no login, no other API.
  - It is **revocable and rotatable** in the web UI.
  - A **second wrapped copy of the DEK** is wrapped by a key derived from the token (argon2id/HKDF).
  - The server stores **only the token's hash** and the token-wrapped DEK, never the token itself.
- **Each scheduled run:** the scheduler presents the token. The server verifies it against the hash, unwraps the DEK **in memory for that run only**, and **zeroizes** it afterwards (`zeroize` crate). Plaintext keys and memory are never written to disk.
- **Where the token lives:** the scheduler process must hold it. The user provides it into a secrets file or `.env` (e.g. `appdata/secrets/scheduler-<id>.token`, mode 0600), **not the DB**. Caveat: compromise of the host **plus** the token exposes that user's keys and memory.
- **Password change:** re-wraps the password copy of the DEK. Data is not re-encrypted. **Token rotation:** re-wraps the token copy.
- **Loss:** a lost password **and** a lost token means the data is lost. Either one alone can still unwrap the DEK. There are no backups in v1.
- **Superseded:** the earlier "master key in `.env`" option (`HEALTHVIBE_SECRET_KEY`) is dropped.

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
- **Multi-LLM insights (BYOK):** implement the `LlmProvider` trait (OpenAI-compatible first, then Anthropic/Gemini adapters), shared tool layer for MCP + in-app insights, opt-in UI and data-sent disclosure, model-quality recommendation. Decide how keys are supplied at run time. Scheduled runs need a server-side key: operator `.env` per provider vs. per-user key encrypted at rest (see [Scheduled insights](#scheduled-insights-background-runs)). See [Multi-LLM support](#multi-llm-support-bring-your-own-key).
- **Scheduled insights:** `insight_schedules` + `insight_runs` tables, in-process scheduler (`tokio-cron-scheduler`), per-run token cap, Insights review page, optional completion notification and "Run now".
- **Insight memory:** per-user encrypted `memory.enc` + `keys.enc`, versioned JSON schema, relevance-based prompt injection, `get_insight_memory` MCP tool, write-back and compaction. Encryption per [the split-key design](#encryption-of-keys-and-insight-memory-resolved).
- **Split-key encryption:** per-user DEK, argon2id password wrap, scheduler token (scoped, hashed, revocable/rotatable UI) + token wrap, in-memory unwrap with zeroize, rewrap on password change/rotation; pick XChaCha20-Poly1305 vs `age`; expand the design doc.

## Open questions for Toby

1. **Shortcut upload receiver:** fold it into the main binary (**recommended**: one binary, separate route and `upload` token scope, shared DB and dedupe code) or run it as a separate small service?
2. **PR #1:** close it and redo the scaffold in Rust?
3. ~~**Encryption for keys and insight memory**~~ **Resolved:** split-key DEK (password-wrapped + scheduler-token-wrapped). See [Encryption](#encryption-of-keys-and-insight-memory-resolved).
