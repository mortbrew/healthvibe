# healthvibe

Self-hosted, privacy-first Apple Health dashboard: full-history export.xml import, automatic iPhone sync via the Health Auto Export app, multi-user, themeable web UI, and a read-only MCP server.

Status: early development.

## Running locally (development build)

```sh
sudo mkdir -p /opt/stacks/healthvibe && sudo chown "$USER" /opt/stacks/healthvibe && git clone https://github.com/mortbrew/healthvibe /opt/stacks/healthvibe && cd /opt/stacks/healthvibe && cp .env.example .env && chmod 600 .env && mkdir -p appdata && sudo chown -R 1000:1000 appdata && docker network create proxy || true && docker compose build && docker compose up -d
```

The image is built locally from source; nothing is pulled from a registry
except the public base images. See comments in `compose.yaml` and `.env.example`.

### Trying it without a reverse proxy

1. Create the shared network once (compose expects it to exist):
   `docker network create proxy`
2. In `compose.yaml`, uncomment the `ports:` block so the app is reachable at
   `http://127.0.0.1:8000` from this machine only.
3. In `.env`, set `HEALTHVIBE_ALLOW_HTTP=true` (login cookies otherwise require HTTPS).
4. `docker compose build && docker compose up -d`, open http://127.0.0.1:8000 and
   create the first account; it becomes the admin.

Undo steps 2 and 3 before putting the app behind your reverse proxy.

### iPhone sync (Health Auto Export)

v1 syncs with the [Health Auto Export](https://apps.apple.com/app/id1115567069) iOS app
(Premium, REST API automations). New users get a guided setup after first sign-in;
re-open it from **Settings** or **Help → Sync your iPhone with Health Auto Export**.

1. **Settings → iPhone sync → Create my token** shows the URL and header once.
2. In Health Auto Export: Automations → + → REST API.
   - URL: `https://<HEALTHVIBE_PUBLIC_URL>/api/v1/ingest/hae`
   - Header: `Authorization: Bearer hvu_…` (or `X-Healthvibe-Token: hvu_…`; `api-key` also works)
   - Export Format **JSON**, Data Type **Health Metrics** (and a second automation for
     **Workouts**, v2), Batch Requests **on**, Aggregate Data off for every reading and sleep stage.
3. Turn on all metrics in Health Auto Export except raw Heart Rate (keep Resting HR,
   Walking HR Average and HRV). heart_rate is still accepted if sent. healthvibe stores every metric it receives
   (known ones mapped, unknown ones kept as `healthvibe.hae.<name>`). Re-sent readings
   are updated, never doubled. Max body: `HEALTHVIBE_HAE_MAX_REQUEST_BYTES` (256 MiB).
4. Data only syncs while the iPhone is unlocked (iOS protects Health data when locked).

Simulate the phone (synthetic fixture):

```sh
curl -X POST http://localhost:8080/api/v1/ingest/hae -H "Authorization: Bearer hvu_…" \
  -H 'Content-Type: application/json' --data @crates/healthvibe-core/tests/fixtures/hae.json
```

The DIY iPhone Shortcut path (`/api/v1/shortcut/*`) is still in the code but hidden from
Help for v1.

### A free sync app?

A free healthvibe iPhone app needs a paid Apple developer account. If
[GitHub Sponsors](https://github.com/sponsors/mortbrew) and donations cover the yearly Apple
developer fee, I'll build a free data sync app available to everyone. — Toby

### Developing

- Server: `cargo run -p healthvibe-server` (listens on `HEALTHVIBE_BIND`; serves the
  built web app from `HEALTHVIBE_WEB_DIR`, default `/web`).
- Web app: `cd web && npm ci && npm run dev` (Vite proxies `/api` to `127.0.0.1:8000`).
- Test data: `python3 scripts/gen_synthetic_export.py synthetic.zip` builds a
  fake export (no real health data) you can upload on the Import page.
