# healthvibe

Self-hosted, privacy-first Apple Health dashboard: full-history export.xml import, daily iPhone Shortcut uploads, multi-user, themeable web UI, and a read-only MCP server.

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

### iPhone Shortcut sync

1. Set `HEALTHVIBE_PUBLIC_URL` in `.env` to the HTTPS address your iPhone uses.
2. In the web app: **Settings → iPhone Shortcut sync**, choose what to send, **Save**.
3. **Help → Build the Shortcut yourself** lists every Shortcuts action; **Help → Set up
   the iPhone Shortcut** shows the one-time setup QR code (new on every page load,
   10 minutes, single use).

Endpoints: `POST /api/v1/shortcut/claim` (setup code → upload token `hvu_…`),
`GET /api/v1/shortcut/config` and `POST /api/v1/ingest` (both with
`Authorization: Bearer hvu_…`). Only a hash of the token is stored; reset it from
**Help → I messed up my Shortcut**. Simulate the phone with curl:

```sh
TOKEN=$(curl -s -X POST -H 'Content-Type: application/json' -d '{"setup_code":"<code from the setup page>","device_name":"curl"}' http://127.0.0.1:8000/api/v1/shortcut/claim | jq -r .upload_token) && curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/v1/shortcut/config && curl -s -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' -d '{"metrics":{"stepCount":{"samples":[{"start":"2026-11-02T07:00:00-05:00","end":"2026-11-02T07:10:00-05:00","value":812,"source":"curl"}]}}}' http://127.0.0.1:8000/api/v1/ingest
```

### Developing

- Server: `cargo run -p healthvibe-server` (listens on `HEALTHVIBE_BIND`; serves the
  built web app from `HEALTHVIBE_WEB_DIR`, default `/web`).
- Web app: `cd web && npm ci && npm run dev` (Vite proxies `/api` to `127.0.0.1:8000`).
- Test data: `python3 scripts/gen_synthetic_export.py synthetic.zip` builds a
  fake export (no real health data) you can upload on the Import page.
