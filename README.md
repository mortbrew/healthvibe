# healthvibe

Self-hosted, privacy-first Apple Health dashboard: full-history export.xml import, daily iPhone Shortcut uploads, multi-user, themeable web UI, and a read-only MCP server.

Status: early development.

## Running locally (development build)

```sh
sudo mkdir -p /opt/stacks/healthvibe && sudo chown "$USER" /opt/stacks/healthvibe && git clone https://github.com/mortbrew/healthvibe /opt/stacks/healthvibe && cd /opt/stacks/healthvibe && cp .env.example .env && chmod 600 .env && mkdir -p appdata && sudo chown -R 1000:1000 appdata && docker network create proxy || true && docker compose build && docker compose up -d
```

The image is built locally from source; nothing is pulled from a registry
except the public base images. See comments in `compose.yaml` and `.env.example`.
