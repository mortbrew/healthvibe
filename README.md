# healthvibe

Self-hosted, privacy-first Apple Health dashboard: full-history export.xml import, daily iPhone Shortcut uploads, multi-user, themeable web UI, and a read-only MCP server.

Status: scaffold. The app serves a placeholder page and `/healthz`. Models, import, and accounts come next.

## Dev quickstart

Python 3.13.

```bash
python3.13 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
mkdir -p appdata
healthvibe
```

Open <http://127.0.0.1:8000>. Defaults are in `healthvibe/config.py` (SQLite at `./appdata/healthvibe.db`, bound to localhost). Copy `.env.example` to `.env` only to override them. `.env` is gitignored.

```bash
ruff check .
ruff format --check .
pytest
```

## Docker

The service publishes no ports. Pangolin/Newt should join the `healthvibe_tunnel` network and target `http://healthvibe:8000`. The process runs as uid/gid 1000. `./appdata` is mounted at `/data`.

```bash
cp .env.example .env
mkdir -p appdata
chown 1000:1000 appdata
docker compose up --build
```

## Charts

Apache ECharts 6.1.0 and htmx 2.0.11 are vendored under `healthvibe/static/vendor/` (checksums and licenses in `NOTICE` there). The page does not load a CDN.

## Synthetic data

```bash
python scripts/gen_synthetic.py --days 30 --seed 1
```

Writes a fake `export.xml` and a one-day JSON payload to `appdata/synthetic/`. That directory is gitignored, and the script will not write inside the repo unless git already ignores the destination. Do not commit health exports.

## Database

SQLAlchemy opens SQLite in WAL mode. Alembic is configured (`alembic upgrade head`) and has no revisions yet.
