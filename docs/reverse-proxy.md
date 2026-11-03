# Running healthvibe behind a reverse proxy

healthvibe is **proxy-agnostic**: it speaks plain HTTP on one port inside its container and expects any reverse proxy (Caddy, Traefik, Nginx Proxy Manager, Pangolin, nginx, HAProxy, Cloudflare Tunnel…) to terminate TLS in front of it. Nothing in the app depends on a specific proxy. This guide is also linked from the admin step of the setup wizard (see [Setup check](#setup-wizard-check)). See also [architecture.md](architecture.md).

## What healthvibe needs from any proxy

| Requirement | Why | Typical setting |
|---|---|---|
| **TLS at the proxy** | The iPhone Shortcut and logins need HTTPS; healthvibe refuses non-HTTPS in production unless `HEALTHVIBE_ALLOW_HTTP=true` (local testing only) | Let's Encrypt / ACME in the proxy |
| **Forwarded headers** | Correct links, QR codes, secure cookies, per-IP rate limits | `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host` (or `Forwarded`) |
| **Trusted proxies** | healthvibe only believes forwarded headers from these addresses | `HEALTHVIBE_TRUSTED_PROXIES=172.16.0.0/12` (CIDRs, comma-separated) |
| **Public URL** | The canonical URL used in QR codes, emails, the Shortcut config | `HEALTHVIBE_PUBLIC_URL=https://health.example.com` |
| **Body size** | Shortcut uploads can be several MB | ≥ **50 MB** for `/api/v1/ingest/*` |
| **Large imports** | `export.xml` (zipped) can be 100 MB–2 GB | Raise the limit for `/api/v1/import` (e.g. 2 GB) **or** rely on the chunked upload (the SPA uploads in 16 MB chunks, so 50 MB everywhere is enough) |
| **Timeouts** | Slow phone uploads, long import chunks | read/send timeout ≥ 120 s; no buffering limits that break streaming |
| **Keep `Authorization`** | Ingest and MCP use `Authorization: Bearer …` | Don't strip or overwrite it (watch out for SSO/forward-auth middleware) |
| **WebSocket/streaming** | MCP streamable HTTP uses long responses (SSE) | Disable response buffering for `/mcp` |

### Trusted-proxy handling

- `X-Forwarded-*` headers are used **only** when the TCP peer is in `HEALTHVIBE_TRUSTED_PROXIES`; otherwise they're ignored and the peer address is the client.
- For `X-Forwarded-For`, healthvibe walks the list from the right and takes the first address that isn't a trusted proxy.
- `HEALTHVIBE_PUBLIC_URL` always wins for generated links; forwarded `Host`/`Proto` are used for the setup check and to warn about mismatches.

### Path-based routing (restricting the UI, exposing ingest)

You can put the web UI behind SSO (Pangolin auth, Authelia, Authentik, Cloudflare Access…) and still let the iPhone reach the upload endpoints, which are protected by their own bearer token:

| Path | Who calls it | Behind SSO? |
|---|---|---|
| `/api/v1/ingest/*` | iPhone Shortcut | **No** (bearer token) |
| `/api/v1/shortcut/config`, `/api/v1/shortcut/claim` | iPhone Shortcut | **No** (bearer token / one-time claim code) |
| `/shortcut/download/*` | iPhone (QR code 1) | No (signed, 15-min URL) |
| `/mcp` | AI clients | Usually no (bearer `mcp` token) |
| everything else (`/`, `/api/v1/*`) | browser | Optional |

healthvibe's own login still applies behind SSO.

## Compose

By default healthvibe **publishes no ports** and joins an external Docker network that your proxy is also on:

```yaml
services:
  healthvibe:
    image: ghcr.io/mortbrew/healthvibe:latest
    user: "1000:1000"
    env_file: .env
    volumes: ["./appdata:/data"]
    networks: [proxy]
    # Optional instead of a proxy network (proxy on the host):
    # ports: ["127.0.0.1:8000:8000"]
networks:
  proxy:
    external: true
    name: ${HEALTHVIBE_PROXY_NETWORK:-proxy}
```

`.env`:
```
HEALTHVIBE_PUBLIC_URL=https://health.example.com
HEALTHVIBE_TRUSTED_PROXIES=172.16.0.0/12
```

## Caddy

```caddyfile
health.example.com {
    request_body {
        max_size 50MB
    }
    reverse_proxy healthvibe:8000 {
        flush_interval -1          # streaming for /mcp
        transport http {
            read_timeout 120s
            write_timeout 120s
        }
    }
}
```
Caddy sets `X-Forwarded-For/Proto/Host` and passes `Authorization` by default. To protect only the UI with forward auth, use a `@ui not path /api/v1/ingest/* /api/v1/shortcut/* /shortcut/download/* /mcp*` matcher on the `forward_auth` block.

## Traefik (Docker labels)

```yaml
    labels:
      - traefik.enable=true
      - traefik.http.routers.healthvibe.rule=Host(`health.example.com`)
      - traefik.http.routers.healthvibe.entrypoints=websecure
      - traefik.http.routers.healthvibe.tls.certresolver=letsencrypt
      - traefik.http.services.healthvibe.loadbalancer.server.port=8000
      - traefik.http.middlewares.hv-body.buffering.maxRequestBodyBytes=52428800
      - traefik.http.routers.healthvibe.middlewares=hv-body
      # Optional: SSO on the UI only, with a higher-priority router for the token-protected paths
      - traefik.http.routers.healthvibe-api.rule=Host(`health.example.com`) && (PathPrefix(`/api/v1/ingest`) || PathPrefix(`/api/v1/shortcut`) || PathPrefix(`/shortcut/download`) || PathPrefix(`/mcp`))
      - traefik.http.routers.healthvibe-api.priority=100
      - traefik.http.routers.healthvibe-api.entrypoints=websecure
      - traefik.http.routers.healthvibe-api.tls.certresolver=letsencrypt
      - traefik.http.routers.healthvibe-api.middlewares=hv-body
```
Traefik sends forwarded headers by default; set its entrypoint `transport.respondingTimeouts.readTimeout` to ≥ 120s. Add your auth middleware (e.g. Authelia forwardAuth) only to the `healthvibe` router.

## Nginx Proxy Manager

1. Add a Proxy Host: domain `health.example.com`, forward to `healthvibe` port `8000` (both containers on the same network), scheme `http`.
2. SSL tab: request a Let's Encrypt certificate, enable Force SSL and HTTP/2.
3. Advanced tab:
   ```nginx
   client_max_body_size 50m;
   proxy_read_timeout 120s;
   proxy_send_timeout 120s;
   proxy_buffering off;
   proxy_set_header Authorization $http_authorization;
   ```
   NPM already sets `X-Forwarded-For/Proto/Host`.
4. To protect the UI only (Access Lists or Authelia), add custom locations for `/api/v1/ingest`, `/api/v1/shortcut`, `/shortcut/download` and `/mcp` without the access list.

## Pangolin (example)

1. Create a resource for `health.example.com` pointing at `http://healthvibe:8000` through your site's Newt connector (healthvibe on the same Docker network as Newt; no published ports).
2. If you enable Pangolin authentication (SSO) on the resource, add **bypass rules** (path rules set to allow) for `/api/v1/ingest/*`, `/api/v1/shortcut/*`, `/shortcut/download/*` and `/mcp*` so the iPhone and AI clients reach them with their bearer tokens.
3. Pangolin terminates TLS and forwards `X-Forwarded-*`; set `HEALTHVIBE_TRUSTED_PROXIES` to the Docker network range Newt connects from.
4. Check the body size and timeout limits in your Pangolin/Traefik config if large uploads fail.

## Setup wizard check

The first admin step of the setup wizard (before any user onboarding) calls `GET /api/v1/admin/proxy-check` and shows:

- the scheme, host and client IP healthvibe saw, and whether they came from a trusted proxy;
- **warning** if the request wasn't HTTPS, if `X-Forwarded-Proto`/`Host` are missing, if the peer isn't in `HEALTHVIBE_TRUSTED_PROXIES`, or if the host doesn't match `HEALTHVIBE_PUBLIC_URL`;
- a test upload of a 10 MB dummy body to a dry-run ingest path to verify the body limit, and a test with a dummy `Authorization` header to verify it isn't stripped;
- a link to this guide.

QR codes are only shown once `HEALTHVIBE_PUBLIC_URL` is set and HTTPS is confirmed (or the admin explicitly overrides for local testing).
