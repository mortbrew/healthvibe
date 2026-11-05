# healthvibe security model and data custody

This document explains, in plain language, where your health data goes when you
use healthvibe, who can read it at each step, and what is and is not protected.
It describes the code as it is today. Anything that is designed but not built
is marked **planned**. If this document and the code ever disagree, the code
wins and this document is a bug.

Short version:

- healthvibe is a single server you run yourself. Your health data is stored
  on that server in a SQLite database file, **not encrypted** by healthvibe.
- Inside the app, every user only sees their own data. Admins cannot browse
  other users' health data through the app.
- Anyone who can read the server's disk (or the database file, or a copy of
  it) can read everyone's health data. Protect the server and its backups
  accordingly.
- healthvibe makes no outbound network calls with your data. There is no
  telemetry, no analytics, and (today) no AI integration.

## 1. Components

| Piece | What it is |
|---|---|
| Server | One Rust process (`healthvibe serve`) serving the API under `/api/v1` and the built web app. |
| Database | One SQLite file, `healthvibe.db`, in `HEALTHVIBE_DB_DIR` (default `/data/db`), in WAL mode, so `healthvibe.db-wal` and `healthvibe.db-shm` sit next to it and also hold recent data. |
| Staging dir | `HEALTHVIBE_STAGING_DIR` (default `/data/staging`): uploaded export zips while they are imported. |
| Reverse proxy | Yours, any brand. Terminates HTTPS in front of healthvibe. healthvibe itself speaks plain HTTP only. |
| Web app | A React single-page app, loaded from the same server. |
| iPhone senders | The Health Auto Export app, or the healthvibe iPhone Shortcut. |

## 2. How data gets in

There are three ways health data reaches the server. All three end up as rows
in the same database, tagged with the channel they came from
(`export_xml`, `health_auto_export`, `shortcut`; synthetic demo data is `demo`).

### 2.1 Apple Health export (export.xml upload)

1. You export all health data on your iPhone (Health app → profile → Export All
   Health Data). Apple produces `export.zip`. While it sits on your phone, in
   iCloud, in AirDrop or in your Downloads folder, its protection is whatever
   those places give it. healthvibe has no control over that.
2. You upload the zip in the web app while signed in. It is sent as a
   `multipart/form-data` POST to `/api/v1/import`, authenticated by your
   session cookie.
3. The server streams the upload to a randomly named file in the staging dir
   (`upload-<random>.zip`). Uploads larger than `HEALTHVIBE_MAX_IMPORT_BYTES`
   (default 4 GiB) are refused with `413 upload_too_large`: early from the
   announced `Content-Length` when there is one, otherwise as soon as the
   streamed bytes pass the limit, and the partial file is deleted. Your
   reverse proxy may enforce its own, smaller limit.
4. A background job reads `export.xml` from the zip and writes records,
   workouts (with their statistics, events, metadata and route file names)
   and Activity ring goals to the database.
5. The staged zip is deleted when the job finishes, whether it succeeded or
   failed. If the process is killed mid-import, the zip stays in the staging
   dir until the next start: on startup healthvibe deletes leftover
   `upload-*`, `*.tmp` and `*.part` files directly in the staging dir (regular
   files only; it does not follow links or descend into folders) and marks
   imports that were still running as failed. The deletion is a normal file
   delete, not a secure wipe.

GPS route files (`workout-routes/*.gpx`) inside the zip are **not** imported.
Only their file names are kept, as part of a workout's details. Clinical
records, ECG files and other zip contents are ignored.

### 2.2 Health Auto Export (third-party iPhone app)

1. In the web app you create an **upload token**, which is shown once. You can
   also open a one-time "phone handoff" link (a QR code) on your iPhone. That
   link is valid for 10 minutes and can be used once. The token is only
   created at the moment the link is opened.
2. You paste the server URL and token into Health Auto Export. From then on the
   app POSTs JSON to `/api/v1/ingest/hae` with the token in a header.
3. The server checks the token, parses the JSON in memory and writes the
   samples. The raw HAE body is not stored.
4. Size limit: `HEALTHVIBE_HAE_MAX_REQUEST_BYTES` (default 256 MiB). Rate limit:
   2000 uploads per hour per token.

Health Auto Export is a third-party app. Its own data handling and any cloud
features it has are outside healthvibe's control. Check its privacy policy.

### 2.3 iPhone Shortcut

1. In the web app you choose which metrics to sync, then scan a QR code with a
   one-time setup code. The code is valid for about 10 minutes and works once.
2. The Shortcut exchanges the code for an upload token
   (`POST /api/v1/shortcut/claim`, rate limited per IP) and stores it on the
   phone.
3. The Shortcut POSTs JSON samples to `/api/v1/ingest` with the token.
   Default size limit 5 MiB, 120 uploads per hour per token.
4. The raw request body is written to the `ingest_raw` table **before** it is
   parsed. That way a crash cannot lose it. It is deleted right after a
   successful parse. If parsing fails or any sample is invalid, the body is
   kept for 24 hours for debugging and then purged on the next upload. While
   kept it is unencrypted health data in the database, like everything else.

### 2.4 Upload tokens

- Format: `hvu_<prefix>_<secret>`, with a 6-byte random prefix and a 32-byte
  random secret, hex encoded.
- The server stores only the prefix and `sha256(secret)`. The token itself is
  never stored, so it cannot be shown again. Create a new one instead.
- There is **one token per user**, shared by the Shortcut and Health Auto
  Export. Creating a new token, pairing the Shortcut again or opening a new
  handoff link replaces the old token.
- What a token can do:
  - Upload samples.
  - Read the sync config (`GET /api/v1/shortcut/config`): which metrics are
    enabled, the newest sample time already received per metric, and size
    limits.
- What it cannot do: read any health data back, sign in to the web app or
  touch other users.
- Revoke it from the web app's Help page. That calls
  `DELETE /api/v1/sync/token`, which deletes the token row.
- Setup codes and handoff codes are stored only as SHA-256 hashes and expire
  after about 10 minutes.

## 3. Transport

- healthvibe does **not** terminate TLS. It listens on plain HTTP
  (`HEALTHVIBE_BIND`, default `0.0.0.0:8000` inside the container). Encryption
  in transit depends entirely on your reverse proxy serving HTTPS.
- The bundled `compose.yaml` publishes **no** host ports. Only containers on
  the proxy's Docker network can reach the app. The commented-out alternative
  binds to `127.0.0.1` only.
- Traffic between the proxy and healthvibe is plain HTTP on that Docker
  network. Anyone with root on the host can observe it.
- Session cookies carry the `Secure` flag unless you set
  `HEALTHVIBE_ALLOW_HTTP=true`. That setting is for local testing over plain
  HTTP only; never use it on a public address.
- **Trusted proxies.** `HEALTHVIBE_TRUSTED_PROXIES` is a comma-separated list
  of IPs or CIDR blocks (for example `172.16.0.0/12`) for your reverse proxy.
  `X-Forwarded-For`, `X-Real-IP`, `X-Forwarded-Host` and `X-Forwarded-Proto`
  are believed **only** when the TCP peer is in that list; from anyone else
  they are ignored. The list is empty by default, so out of the box no
  forwarding header is believed. An invalid entry stops the server from
  starting.
- Rate limits that key on client IP (login, setup-code claim and handoff
  claim) use the client address worked out like this: from a trusted peer,
  the right-most `X-Forwarded-For` hop that is not itself a trusted proxy
  (entries further left were supplied by the client and are ignored), else
  `X-Real-IP`, else the peer; from any other peer, the peer itself. If you run
  behind a proxy but leave the list empty, every visitor shares the proxy's
  address for these limits. The per-account lockout (section 5) still applies.
- When `HEALTHVIBE_PUBLIC_URL` is unset, the setup QR codes are built from the
  `Host` header of your own request, or from `X-Forwarded-Host` and
  `X-Forwarded-Proto` when the request came through a trusted proxy. Set
  `HEALTHVIBE_PUBLIC_URL` so the address never depends on headers.
- The rate-limit counters are in memory and reset when the server restarts.

## 4. Storage at rest

### 4.1 What is in the database

| Data | Table | Stored as |
|---|---|---|
| Health samples: steps, heart rate, sleep, body measurements etc., with source device names and time zone | `readings` | **Plain text, not encrypted** |
| Workouts and their raw details (statistics, events, metadata, route file names) | `workouts` | **Plain text** |
| Activity ring goals | `activity_goals` | **Plain text** |
| Profile: display name, birth date, sex, height, wheelchair use, avatar settings, units | `profiles` | **Plain text** |
| Pinned metrics, dashboards, theme | various | Plain text |
| Failed Shortcut upload bodies, for up to 24 h | `ingest_raw` | **Plain text** |
| Import history (counts per type, errors) | `import_jobs` | Plain text |
| Usernames (which may be email addresses), admin flag | `users` | Plain text |
| Passwords | `users.password_hash` | argon2id hash (PHC string, random salt, library defaults) |
| Session tokens | `sessions` | SHA-256 of the token |
| Upload tokens, setup codes, handoff codes | `upload_tokens`, `setup_codes`, `handoffs` | SHA-256 of the secret |
| Failed-login counters, keyed by attempted username | `auth_failures` | Plain text |

**healthvibe does not encrypt health data at rest.** The database file is a
normal SQLite file. Anyone with a copy can open it with any SQLite tool and
read every user's data. If you want encryption at rest, use full-disk or
volume encryption on the host (section 9). That protects a stolen or
discarded disk, but not a running, unlocked server.

Deleted rows can survive for a while inside the SQLite file, its WAL file, and
filesystem or backup snapshots. healthvibe does not run `VACUUM` or enable
`secure_delete`.

### 4.2 Per-user encryption, API keys, AI history: **planned**

A split-key / per-user data key model is **planned and not built**. Nothing in
the code encrypts per-user data today, and there are no stored per-user API
keys and no AI chat history. The recovery-key design that would go with
per-user keys is in [design/recovery-key.md](design/recovery-key.md). Until it
ships, treat all data in the database as readable by whoever holds the file.

### 4.3 Scheduler tokens: **planned**

There is no background scheduler and there are no scheduler tokens in the code
today. Purging expired raw uploads happens inline when the next upload arrives.

### 4.4 Backups: **planned**

healthvibe has no built-in backup command. Backing up today means copying the
database directory yourself. Do it with the container stopped, or with
SQLite's online backup (`sqlite3 healthvibe.db ".backup out.db"`). Copying
only `healthvibe.db` while the server runs can miss data still in the WAL
file.

A backup contains **everything** in section 4.1 for **every** user, in plain
text, including password hashes. Anyone who has the backup can read all
health data, and can try to guess passwords offline against the argon2id
hashes. Encrypt backups and store them as carefully as the server itself.

### 4.5 Logs

Logs go to stdout. In the default compose file Docker keeps 3 × 10 MB files.
They record events such as user ids, "shortcut paired", import failures and
upload counts. They are not meant to contain health values or tokens, but
error messages can include fragments of a malformed upload (for example a
JSON parse error near some text). Treat logs as sensitive.

## 5. Accounts, sessions and admins

- **First run:** until someone sets the first password, the server accepts a
  one-time setup at `/api/v1/setup`. **Whoever reaches it first becomes the
  admin.** Finish setup before exposing the server to the internet.
- **Passwords:** at least 12 characters, hashed with argon2id. Admins create
  users with a temporary password, and the server refuses every other API call
  until the user changes it.
- **Login protection:** 5 failed attempts on a username locks it for 15
  minutes. Attempts on unknown names are counted too. There is also a per-IP
  limit of 60 login attempts per 10 minutes (see trusted proxies in section 3).
- **Sessions:** a random 32-byte token in a cookie that is `HttpOnly`,
  `SameSite=Strict` and `Secure` (unless allow-http), valid for 30 days.
  Only the token's hash is stored. You can sign out other sessions.
  Changing your password does **not** currently end your other sessions.
- **CSRF:** `SameSite=Strict`, and state-changing endpoints accept only JSON
  or multipart bodies, not plain form posts.
- **Isolation inside the app:** every data query is filtered by the signed-in
  user's id. There is no endpoint that lets one user, including an admin, read
  another user's health data. API tests cover this.
- **What admins can do in the app:** list accounts (username, admin flag,
  settings flags) and create accounts. That is all. There is no admin password
  reset, user deletion or impersonation in the code today (admin password
  reset is **planned**, see the recovery-key design).
- **What admins cannot be stopped from doing outside the app:** the person who
  runs the server, or anyone with access to its disk, the container, the
  database file or a backup, can read **all** users' data directly. App
  permissions do not protect against the operator. If you host data for
  family or friends, they are trusting you as the operator, not just the app.

## 6. MCP (read-only AI tool access): **planned**

A read-only MCP server is **planned and not built** (the README says so too).
There is no MCP endpoint and no MCP token in the code today. Planned shape: a separate,
read-only, per-user token that can query only that user's data and can be
revoked independently. When it ships, anything an MCP client reads leaves the
server for wherever that client (and any AI model behind it) sends it.

## 7. AI providers: **planned**

There is no AI integration today. healthvibe sends no health data to any AI
provider or any other third party. Planned: bring-your-own-key (BYOK) AI
features. This section must then document exactly what is sent and when, and
where provider keys are stored (planned: encrypted under the per-user key, see
4.2). Data sent to an AI provider is governed by that provider's terms.

## 8. To the browser

- The web app is served from the same origin as the API. Health data reaches
  the browser as JSON over your HTTPS proxy, only for the signed-in user.
- The app loads no third-party scripts, fonts or trackers. Fonts are bundled.
  The only external links are plain links (the App Store page for Health Auto
  Export, a sponsor page). They are fetched only if you click them.
- The browser keeps your theme choice in `localStorage`. Health data is not
  deliberately persisted in browser storage, but it lives in page memory and
  may sit in the browser's normal HTTP cache. Use a private window on shared
  computers and sign out afterwards.
- "Export CSV" downloads your readings and workouts as a plain-text file
  (columns `type,value,unit,start_utc,end_utc,source`; each workout is a
  `Workout:<type>` row plus one `Workout:<type>:<field>` row per stored
  value). Once downloaded it is outside healthvibe's protection.
- "Delete all my health data" removes your readings, workouts, ring goals,
  pending raw uploads, sync cursors and pinned metrics.
  It keeps your account, profile, dashboards and paired device. Copies in
  backups, WAL or disk snapshots are not touched.
- Every response carries security headers:
  - `Content-Security-Policy: default-src 'self'; script-src 'self';
    style-src 'self'; img-src 'self' data: blob:; font-src 'self' data:;
    connect-src 'self'; object-src 'none'; base-uri 'self';
    form-action 'self'; frame-ancestors 'none'` (no inline or eval'd script);
  - `X-Content-Type-Options: nosniff`;
  - `Referrer-Policy: no-referrer`;
  - `X-Frame-Options: DENY`;
  - `Permissions-Policy` turning off camera, microphone, geolocation, payment
    and USB;
  - `Cross-Origin-Opener-Policy: same-origin`.
  `Strict-Transport-Security` is sent only when `HEALTHVIBE_HSTS_MAX_AGE` is
  set (seconds); turn it on once HTTPS works for the hostname, or let your
  proxy send it instead.

### 8.1 What healthvibe shows

healthvibe shows only what was imported, plus daily totals and unit
conversions. It does not work out heart-rate zones, workout heart-rate
averages from samples, sleep sessions or naps, streaks, or default or
carried-forward ring goals, and there is no awards feature. Sleep is shown as
the raw stage samples, each counted on the local date its sample **ends**. A
day without an imported ring goal shows no goal.

## 9. Threat model

| Threat | Protected? | How / why not |
|---|---|---|
| Someone on the network sniffs traffic | Yes, if your proxy serves HTTPS | healthvibe relies on the proxy for TLS. |
| A stranger signs in by guessing a password | Mostly | 12+ character passwords, argon2id, per-user lockout, per-IP limit. |
| A stranger finishes first-run setup before you | No | Setup is open until done. Finish it before exposing the server. |
| A stolen session cookie | Partly | HttpOnly and SameSite=Strict, 30-day expiry, sign out others. No binding to device or IP. |
| A cross-site request forgery | Yes | SameSite=Strict plus JSON/multipart only. |
| A stolen phone or leaked upload token | Partly | A token can push fake data into your account and read your sync config, but cannot read your health data. Revoke it in the app. |
| Another user on the same server reading your data via the app | Yes | All queries are scoped to the signed-in user. |
| An admin reading your data via the app | Yes | No admin endpoint exposes health data. |
| The operator, or anyone with disk/container/backup access, reading your data | **No** | Data is unencrypted at rest. Per-user encryption is planned (4.2). |
| A stolen or discarded server disk | Only with host disk encryption | healthvibe does not encrypt. |
| A leaked backup | **No**, unless you encrypt backups | Backups contain everything. |
| Offline password guessing from a leaked database | Slowed | argon2id with salt, but weak passwords can still fall. |
| A malicious export.zip or upload | Mostly | Streaming XML parser, size limits on all uploads (export zips: `HEALTHVIBE_MAX_IMPORT_BYTES`, default 4 GiB), user-scoped writes, leftovers cleaned on startup. Several large uploads at once can still fill the staging disk. |
| A client forging its IP or host via `X-Forwarded-*` | Yes | Those headers count only from `HEALTHVIBE_TRUSTED_PROXIES`. |
| The app being framed (clickjacking) or loading injected script | Yes | `frame-ancestors 'none'`, `X-Frame-Options: DENY`, a CSP without inline script. |
| Data sent to third parties | No such flows today | No telemetry, no AI, no outbound calls with data. |
| A compromised third-party iPhone app (Health Auto Export) | Out of scope | It sees your data on the phone by design. |

## 10. Recommendations for self-hosters

1. Put healthvibe behind a reverse proxy that serves **HTTPS only**. Redirect
   HTTP, and enable HSTS (at the proxy, or with `HEALTHVIBE_HSTS_MAX_AGE`).
   Do not publish port 8000 to the internet or LAN.
2. Set `HEALTHVIBE_PUBLIC_URL`, and leave `HEALTHVIBE_ALLOW_HTTP` unset (false).
3. Set `HEALTHVIBE_TRUSTED_PROXIES` to your proxy's address or network, make
   sure the proxy appends a correct `X-Forwarded-For`, and make sure clients
   cannot reach healthvibe except through it.
4. Finish the first-run admin setup before the address is reachable by others.
5. healthvibe sends its own security headers (section 8). If your proxy adds
   headers too, don't loosen the CSP. Consider putting the whole site behind
   your own SSO or VPN as an extra layer. Set `HEALTHVIBE_MAX_IMPORT_BYTES`
   to suit your export size and staging disk.
6. Use full-disk or volume encryption on the host, and restrict who has shell,
   Docker or root access. Everyone with that access can read all health data.
7. Keep `./appdata` owned by the container user and not readable by others
   (`chmod 700`). Keep `.env` at `chmod 600`.
8. Back up with SQLite's online backup or with the container stopped.
   **Encrypt** backups (for example with `age` or `restic`) and keep the key
   separate from the backup.
9. Leftover uploads in `./appdata/staging` are deleted on the next start;
   restart healthvibe after a crash during an import.
10. Treat container logs as sensitive and keep retention short.
11. Use long, unique passwords, revoke upload tokens for phones you no longer
    use, and use "sign out other sessions" if a device is lost.
12. If you host for others, tell them plainly that you, as the operator, can
    technically read their data.

## 11. Reporting a security issue

Please report vulnerabilities privately to the maintainer through GitHub's
private vulnerability reporting on the repository, rather than in a public
issue.
