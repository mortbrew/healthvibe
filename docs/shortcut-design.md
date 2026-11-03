# healthvibe iPhone Shortcut — design

See also: [architecture.md](architecture.md) (Rust stack) and [theming.md](theming.md).

Status: **draft for review**. No code yet; coding is paused until this design is settled.

healthvibe is a self-hosted, multi-user Apple Health dashboard (Rust: Axum, SQLite via sqlx, React SPA (Vite + TypeScript) over a JSON API; see [architecture.md](architecture.md)). Apple Health data only leaves the phone through (a) a one-off `export.xml` from the Health app, or (b) something running on the phone. For daily updates we ship one **master Shortcut** that reads only the metrics each user has enabled and POSTs them to that user's healthvibe server.

## 1. Decisions so far

| Topic | Decision |
|---|---|
| Distribution | A signed `.shortcut` file made on a friend's Mac for each release (`shortcuts sign --mode anyone`), bundled in the release/image and **served by the healthvibe server itself** (QR code 1). A "Build it yourself" guide lets users verify or recreate it ([shortcut-build.md](shortcut-build.md)). Signing without a Mac: see §9. |
| Shape | One master Shortcut with a `Find Health Samples` block for every supported metric ([metrics.md](metrics.md)), each gated on whether that metric is enabled. |
| Metric choice | **Web UI only.** The user must choose metrics (and a trigger) on the Setup page before any QR code is shown; changes are made only on the Data Sources page. The Shortcut has no config menus; it GETs the enabled list every run. The server rejects metrics that aren't enabled. |
| Onboarding | Choose sync settings → **QR code 1** installs the Shortcut from the server → **QR code 2** (server URL + one-time setup code) configures it; the code is exchanged for a per-user upload token stored on the phone (§2a). |
| Trigger | User picks one in the web UI (§6a), which shows the setup steps; default "Alarm is stopped", **Run Immediately**. Health data can only be read while the phone is unlocked. |
| export.xml | Default: store everything, show only enabled metrics. Option at import time: import only selected metrics (§7). |
| Sleep score | Apple's score isn't readable; healthvibe computes its own (§8). |

## 2. Flow

```mermaid
sequenceDiagram
    autonumber
    actor U as User
    participant W as healthvibe web app
    participant S as Shortcut (iPhone)
    participant H as Apple Health

    rect rgb(40,40,60)
    note over U,S: One-time setup (Setup page, web UI)
    U->>W: Log in, open Setup page
    U->>W: Step 1: choose metrics + trigger (required)
    W-->>U: Step 2: QR 1 = install link (login or short-lived signed URL)
    U->>W: Scan QR 1 on iPhone → GET /shortcut/download/<signed>
    W-->>U: healthvibe.shortcut (signed, bundled with this release)
    U->>S: Import Shortcut
    W-->>U: Step 3: QR 2 = server URL + one-time setup code (~10 min, new on each page load)
    U->>S: Run Shortcut manually → no config yet → scan QR 2
    S->>W: POST /api/v1/shortcut/claim {setup_code, device_name, shortcut_version}
    W-->>S: {upload_token, config}
    S->>S: Save hv-config.json in iCloud Drive/Shortcuts
    S->>H: First upload (Health permission sheet)
    W-->>U: Step 4: trigger setup steps, "first upload received ✓"
    U->>U: Create automation for chosen trigger, Run Immediately
    end

    rect rgb(30,50,40)
    note over U,H: Every morning
    U->>S: Stops alarm (phone unlocked) → automation runs
    S->>W: GET /api/v1/shortcut/config (Bearer token)
    W-->>S: {enabled[], since per metric, min_shortcut_version, batch limits}
    loop each metric block in master Shortcut
        alt metric enabled
            S->>H: Find Health Samples (type fixed, start > since)
            H-->>S: samples
        else disabled
            S->>S: skip (never read)
        end
    end
    S->>W: POST /api/v1/ingest/u/<id> (batched JSON)
    W->>W: validate, reject non-enabled, dedupe, store
    W-->>S: {accepted, duplicates, rejected, next_since}
    S->>U: Notification per user setting (Always / Only on errors)
    end
```

## 2a. Onboarding (Setup page)

The Setup page is a wizard; each step unlocks the next.

0. **Admin only, first run: proxy check.** Shows the scheme/host/client IP healthvibe was reached on and warns if it isn't HTTPS, forwarded headers are missing, the proxy isn't trusted or the host doesn't match `HEALTHVIBE_PUBLIC_URL`. Links to [reverse-proxy.md](reverse-proxy.md). QR codes use `HEALTHVIBE_PUBLIC_URL`.

1. **Choose what to sync (required).** Metric checkboxes (presets: Recommended / Everything / custom, see [metrics.md](metrics.md)) and the trigger (§6a). No QR code is shown until both are saved.
2. **QR code 1: install the Shortcut.** Links to `https://<server>/shortcut/download/<token>` where `<token>` is a short-lived (15 min) HMAC-signed URL bound to the user; a logged-in session also works. The server returns the signed `healthvibe.shortcut` bundled in the release/Docker image (`assets/shortcut/healthvibe-<version>.shortcut`, embedded in the binary or copied into the image), with `Content-Type: application/octet-stream` and a filename so iOS offers to import it. No GitHub or iCloud needed. **Alternative for every user:** build the Shortcut yourself from the block-by-block guide and, if you have a Mac, sign it yourself (`shortcuts sign`), then transfer it to the iPhone and continue with QR code 2 — see [Build and sign it yourself](shortcut-build.md#build-and-sign-it-yourself-mac). The wizard shows this option next to QR code 1.
3. **QR code 2: configure.** Encodes `healthvibe://setup?server=https://<server>&code=<setup code>` as plain text (the Shortcut parses it). Setup code: one-time, ~10 min TTL, a **new code is generated each time the page loads** (older unused codes are invalidated). The user runs the Shortcut once by hand; with no `hv-config.json` it opens the camera ("Scan QR/Barcode"), claims the code (§3.4) and saves the token.
4. **Finish.** The page polls for the first upload, then shows the trigger setup steps for the chosen trigger (§6a) and an "Open Shortcuts automations" button.

The Shortcut's only interactive step is that first-run QR scan; there are no metric or trigger menus in it.

## 3. Configuration

### 3.1 On the phone
`iCloud Drive/Shortcuts/healthvibe/hv-config.json` (via Shortcuts' "Save File"/"Get File"):

```json
{ "server": "https://health.example.com", "token": "hvu_…", "device": "Toby's iPhone", "configured_at": "2026-11-03T07:00:00-05:00" }
```

Nothing else is stored on the phone; the metric list and cursors come from the server every run, so changes in the web app take effect on the next run without reinstalling.

### 3.2 `GET /api/v1/shortcut/config`
Auth: `Authorization: Bearer <upload token>`.

```json
{
  "schema": 1,
  "user": "toby",
  "min_shortcut_version": "0.1.0",
  "latest_shortcut_version": "0.1.0",
  "latest_shortcut_url": "https://github.com/mortbrew/healthvibe/releases/download/v0.1.0/healthvibe.shortcut",
  "enabled": ["stepCount", "heartRate", "sleepAnalysis", "bodyMass"],
  "since": {
    "stepCount": "2026-11-02T06:58:11-05:00",
    "heartRate": "2026-11-02T06:59:40-05:00",
    "sleepAnalysis": "2026-11-02T06:45:00-05:00",
    "bodyMass": "2026-10-30T07:10:00-05:00"
  },
  "default_lookback_days": 7,
  "max_lookback_days": 30,
  "max_samples_per_metric": 5000,
  "max_request_bytes": 5242880,
  "notify": "errors_only",
  "trigger": "alarm_stopped"
}
```

`notify` is a per-user setting in the web app: `always` (notification after every run with counts) or `errors_only` (default; notify only on failures, 426 or rejected metrics). The Shortcut reads it each run.

Metric keys are the HealthKit identifier without the `HKQuantityTypeIdentifier` / `HKCategoryTypeIdentifier` prefix (see [metrics.md](metrics.md)). A metric with no prior upload gets `since = now - default_lookback_days`.

### 3.3 No write-back from the phone
There is no endpoint for the Shortcut to change settings. Metrics, trigger and notification choice are edited only in the web UI, so a user can't break their setup from the phone. The upload token's scope is `upload` + reading its own config.

### 3.4 `POST /api/v1/shortcut/claim`
Body `{"setup_code": "…", "device_name": "…", "shortcut_version": "0.1.0"}` → `{"upload_token": "hvu_…", "config": {…same as 3.2…}}`. **One device per user:** each user has a single upload token; claiming a new setup code revokes the previous token (moving to a new phone = scan a new QR). Setup code: 128-bit random, 10-minute TTL, single use, hashed at rest, rate-limited per IP.

## 4. Upload JSON schema — `POST /api/v1/ingest/u/<id>`

```json
{
  "schema": 1,
  "shortcut_version": "0.1.0",
  "device": "Toby's iPhone",
  "run_id": "6f1c…(UUID made by the Shortcut)",
  "batch": { "index": 0, "count": 3 },
  "tz": "America/New_York",
  "metrics": {
    "stepCount": {
      "unit": "count",
      "samples": [
        { "start": "2026-11-02T07:00:00-05:00", "end": "2026-11-02T07:10:00-05:00", "value": 812, "source": "Toby's Apple Watch" }
      ]
    },
    "sleepAnalysis": {
      "unit": null,
      "samples": [
        { "start": "2026-11-03T01:12:00-05:00", "end": "2026-11-03T02:01:00-05:00", "value": "asleepCore", "source": "Toby's Apple Watch" }
      ]
    }
  }
}
```

Workouts are sent in a separate top-level `workouts` array (from the Shortcut's "Find Workouts" action), not inside `metrics`:

```json
"workouts": [
  { "type": "running", "start": "2026-11-02T17:30:00-05:00", "end": "2026-11-02T18:05:12-05:00",
    "duration_s": 2112, "active_energy_kcal": 412, "distance_km": 6.1, "avg_hr": 151, "max_hr": 174,
    "source": "Toby's Apple Watch" }
]
```
Fields other than `type`, `start`, `end` are optional. Dedupe hash = `user_id | workout | type | start | end | source`. Workouts are gated by the `workouts` key in `enabled`.

Rules:
- `value` is a number for quantity types (in the unit listed in metrics.md; the Shortcut converts with "Get Details of Health Sample → Value" in a fixed unit) and a string enum for category types.
- `start`/`end` are ISO-8601 with offset. Server stores UTC + original offset.
- The raw request JSON is **deleted as soon as it has been parsed** and committed; only the parsed rows are kept (failed parses are kept for 24 h for debugging, then deleted).
- Unknown metric → `422`. Metric not enabled for this user → dropped and listed in `rejected` (not a whole-request failure, so a stale phone doesn't lose everything).
- Response:
```json
{ "accepted": 1203, "duplicates": 31, "rejected": {"vo2Max": "not_enabled"}, "next_since": {"stepCount": "2026-11-03T06:58:00-05:00"} }
```

## 5. Picking up where the last run stopped, and dedupe

- **Server-side cursors.** For each (user, metric) the server keeps `last_sample_end` from the last *successful* ingest. `config.since` hands it to the Shortcut, which filters `Start Date is after <since>`. The phone keeps no state, so a failed run simply re-sends the same window next time.
- **Overlap is deliberate.** `since` is moved back by 10 minutes (Watch data syncs to the phone late), so some readings arrive twice.
- **Content hash.** Every reading gets `hash = sha256(user_id | metric | start | end | value | unit | source)`. `readings` has `UNIQUE(user_id, hash)` and inserts use `INSERT … ON CONFLICT DO NOTHING`. The same path is used by the export.xml importer, so a daily upload that overlaps an import never doubles up.
- **Cursor advance** happens only after a batch commits. With multiple batches, each batch advances only the metrics it fully contains.
- **Gaps.** If the user skipped days (phone off, alarm not used), `since` is older than `max_lookback_days`; the Shortcut caps at that and the web app shows "gap: import export.xml or run the Shortcut manually with backfill".
- **Edits/deletions in Health** are not detected (Shortcuts has no anchored query). Known limitation; a fresh export.xml import fixes it.

## 6a. Choosing a trigger (web UI)

Shortcuts can't create automations itself, and settings live only in the web UI, so the trigger is chosen on the Setup page (step 1) and changed on the Settings page; the Shortcut never asks.

1. User picks a trigger from the list below; saved as `trigger` in the user's server config (used for the stale-upload warning, sent in `config` for display only).
2. The web UI shows step-by-step instructions for that trigger (text + planned screenshots).
3. An **"Open Shortcuts automations"** button (shown on iPhone) links to `shortcuts://automations` (or `shortcuts://create-automation` for the new-automation sheet). Neither is in Apple's documented URL-scheme list ([Apple: Shortcuts URL schemes](https://support.apple.com/guide/shortcuts/apda283236d7/ios)), but both are reported working ([Stack Overflow](https://stackoverflow.com/questions/69969746/how-to-deep-link-to-automation-in-shortcut-app), [r/shortcuts](https://www.reddit.com/r/shortcuts/comments/1fdkqaj/shortcuts_automation_url_scheme/)). Test on iOS 26/27; fall back to `shortcuts://` + "tap Automation".

| Option | Trigger in Shortcuts | Run without asking? | Notes |
|---|---|---|---|
| When I stop my alarm (recommended) | Alarm → Is Stopped → Wake-up / Any | **Yes** (Alarm is on Apple's list) | Phone is usually being unlocked, so Health reads work. |
| At a set time | Time of Day → time, Daily | **Yes** | Fails silently if the phone is locked at that time; next run catches up. |
| When I arrive home | Arrive → Home location | **Yes** (Arrive on list) | Location permission needed; phone may be locked → catch-up. |
| When I plug in the charger | Charger → Is Connected | **Yes** (Charger on list) | Often fires while locked at night → mostly catch-up runs; pair with another trigger. |
| When Sleep Focus turns off | Focus → Sleep → When Turning Off | **Unverified.** Apple's list names "Do Not Disturb", not every Focus. Alternative with confirmed support: **Sleep → Waking Up** (needs a Health sleep schedule). | Recommend offering "Waking up (sleep schedule)" in place of / alongside this. |
| Manual only | none | n/a | User runs it from the widget, Siri ("healthvibe"), or the Action button. |

Sources for supported triggers and the run-without-asking list: [Apple: Add automations](https://support.apple.com/guide/shortcuts/add-automations-apdfbdbd7123/ios) (lists Time of Day, Alarm, Sleep, Arrive, Leave, CarPlay, …, Do Not Disturb, Low Power Mode, Battery Level, Charger as runnable automatically), [Apple: Setting triggers](https://support.apple.com/guide/shortcuts/setting-triggers-apde31e9638b/ios) (Focus When Turning On/Off, Charger Is Connected/Disconnected), [What's new in Shortcuts 26](https://support.apple.com/en-us/125148) (Alarm "When my alarm is stopped").

Instruction text template (alarm example):
1. Tap **+** (New Automation) in the Automation tab.
2. Choose **Alarm** → **Is Stopped** → **Wake-Up** (or Any).
3. Choose **Run Immediately** (on iOS 27: turn **Confirm Before Run** off) and turn off **Notify When Run** if you like.
4. Tap **Next** → pick the **healthvibe** shortcut → **Done**.
5. Tomorrow, check the web app's Status card for "last upload".

Automated runs never hit an interactive step (that would block Run Immediately); the only interactive step is the QR scan when `hv-config.json` is missing. Changing the trigger in the web UI just updates the instructions — the user edits the automation on the phone.

**Catch-up and stale warnings.** Every run uploads everything since the last successful upload (§5), so missed days catch up automatically (capped at `max_lookback_days`). The server records `last_upload_at` per device; the web app shows a banner and dashboard Status card when no upload arrived for longer than expected for the chosen trigger (default: 36 h for daily triggers, off for Manual), e.g. "No upload from Toby's iPhone since Sun 7:02 AM — check the automation".

Stale-upload alerts (36 h default) always show in the web app, plus optional per-user alert channels set on the user's Notifications page:
- **Email (SMTP):** server admin configures `SMTP_HOST/PORT/USER/PASSWORD/FROM` in `.env`; each user sets their address and turns email on/off.
- **Push via ntfy:** user enters an ntfy topic URL (ntfy.sh or self-hosted) and optional access token.
- **Push via Apprise:** user enters one or more Apprise URLs (covers Pushover, Gotify, Telegram, Discord, Matrix, etc.); Apprise is Python, so healthvibe talks to an optional [apprise-api](https://github.com/caronc/apprise-api) sidecar container over HTTP (`APPRISE_API_URL` in `.env`); without it, only ntfy and email are offered.
- One alert per stale period (re-armed after the next successful upload); a "Send test" button per channel. Run errors reported by the server (e.g. repeated 426/401) use the same channels.

## 6b. "Help, I messed up my Shortcut" page

A web page (linked from Setup, Settings and every Shortcut error notification) that resets everything on the phone and reinstalls from scratch:

1. **Delete the automation:** Shortcuts → Automation → swipe left on the healthvibe automation → Delete.
2. **Delete the Shortcut:** Shortcuts → long-press **healthvibe** → Delete.
3. **Delete the saved settings file:** Files → iCloud Drive → Shortcuts → `healthvibe` → delete `hv-config.json` (or the whole folder).
4. **Reset the token:** "Reset my upload token" button on the page (revokes the current token immediately; uploads from the old Shortcut get `401`). Sync settings and stored data are kept.
5. **Reinstall:** the page shows **QR code 1** (fresh signed download link). Or build the Shortcut yourself and, with a Mac, sign it yourself: [Build and sign it yourself](shortcut-build.md#build-and-sign-it-yourself-mac).
6. **Reconfigure:** the page shows **QR code 2** (fresh setup code), then the trigger instructions again.

Optional: Settings → Health → Data Access → Shortcuts to review or reset Health permissions. Each step gets a screenshot (planned, see [shortcut-build.md](shortcut-build.md#screenshots)).

## 6. Versioning and Shortcut updates

- The Shortcut has a `HV_VERSION` text action at the top (semver, matches the GitHub release tag) and sends it in every request.
- Server returns `min_shortcut_version` / `latest_shortcut_version` / `latest_shortcut_url` in config (a signed download link on the user's own server).
  - Below min: server answers `426 Upgrade Required`; Shortcut shows an alert with the download link and stops. Updated server images bundle the new signed file, so updating = scan QR code 1 again.
  - Below latest: upload continues; a one-line notification suggests updating.
- Updating = import the new signed file and replace the old one. Config lives in `hv-config.json`, so it survives; the alarm automation must be re-pointed at the new Shortcut if the name changed (keep the name `healthvibe` stable to avoid this).
- Adding a metric = new block in the master Shortcut → new release + re-sign. The server can list metrics the user's Shortcut version doesn't support yet ("update Shortcut to sync X").
- `schema` in requests is versioned separately; server supports current and previous schema.

## 7. export.xml import options

Reconciling "store everything, show enabled" with "choose at import time":
- **Default: Store everything.** Every record is stored (same hash/dedupe). Dashboard and MCP only show enabled metrics; enabling a metric later instantly shows its history.
- **Option: Only selected metrics.** Import page lists metrics found in the file (first pass counts types while streaming) with checkboxes pre-set to the user's enabled list. Unticked types are skipped during parsing and never stored.
- Either way, nothing from the file is kept after import except the parsed rows (the raw upload is deleted after a successful parse unless the user asks to keep it).
- Streaming `quick-xml` reader + batched inserts in transactions; files over 1 GB are fine.

## 8. Sleep score

Apple's Sleep Score (iOS 26 / watchOS 26) is **not exposed** to HealthKit or Shortcuts. Apple DTS confirmed "As of today, there is no API that allows the access to sleep scores" ([Apple Developer Forums 800403](https://developer.apple.com/forums/thread/800403)); vendors such as Terra report it as always null ([Terra docs](https://docs.tryterra.co/help-center/help-topics/data-api-sdk/provider-coverage-and-per-provider-data-availability/apple-sleep-score-not-exposed-by-healthkit)).

healthvibe computes its own score from `sleepAnalysis` stage samples, using Apple's published weighting (duration 50, bedtime 30, interruptions 20; [iDrop News](https://www.idropnews.com/news/apple-seeds-first-ios-262-beta-to-developers-heres-whats-new/255110/)):

| Part | Points | Calculation (v1, tunable) |
|---|---|---|
| Duration | 50 | asleep (core+deep+REM+unspecified) vs. user goal (default 8 h): full points at ≥ goal, linear down to 0 at 4 h |
| Bedtime | 30 | sleep onset vs. user's 14-day median onset: full within ±30 min, linear to 0 at ±2.5 h |
| Interruptions | 20 | awake segments ≥ 2 min after onset: 0 → 20, each one −4, also −1 per 5 min awake |

The v1 formula is **fixed** (not user-tunable; only the sleep goal is a user setting). The dashboard labels it "healthvibe sleep score", shows the breakdown, and documents that it will not exactly match the Watch.

## 9. Signing a Shortcut without a Mac — findings

iOS only imports `.shortcut` files that are signed (AEA archive with an Apple-issued CMS signature chain). Apple's official tool is `shortcuts sign` on macOS.

| Option | How | Feasibility |
|---|---|---|
| Friend's Mac (chosen) | `shortcuts sign --mode anyone -i healthvibe.shortcut -o healthvibe-signed.shortcut` while signed into an Apple ID | **Works.** Once per release. |
| GitHub Actions `macos-*` runner | Run `shortcuts sign` in CI | **Not practical.** The command needs an Apple ID signed into iCloud on that machine; runners are fresh VMs and an interactive iCloud login with 2FA can't be scripted. Storing Apple ID credentials in CI is also a security/ToS risk. A self-hosted Mac runner would work but needs a Mac. |
| iCloud link | Share → Copy iCloud Link from the iPhone | **Works, no Mac,** but deferred: not approved for now. |
| [0xilis/shortcut-sign](https://github.com/0xilis/shortcut-sign) (+ [libshortcutsign](https://github.com/0xilis/libshortcutsign)) | Open-source Linux/macOS CLI reimplementing Apple's signing | **Technically possible, not recommended.** Needs the signer's Apple ID private key and auth data, which must be dumped from a jailbroken iPhone or an AMFI-disabled Mac ([appleid-key-dumper](https://github.com/0xilis/appleid-key-dumper)). Still beta (v1.0.0-beta.5). Once keys exist, re-signing could run in CI. |
| HubSign (RoutineHub) via [wynx1123/shortcut-signer](https://github.com/wynx1123/shortcut-signer) | POST the unsigned plist to a third-party service | **Works on Linux, risky.** Depends on an unofficial community service; the Shortcut (with server URL patterns) goes to a third party; availability and terms can change. OK for experimenting, not for releases. |
| [scaxyz/shortcut-signing-server](https://github.com/scaxyz/shortcut-signing-server) | Self-hosted HTTP wrapper | Still runs `shortcuts sign` underneath, so it needs a Mac. |

Decision: the friend-signed `.shortcut` file is bundled in each release and Docker image and **served by each healthvibe server** (QR code 1); it's also attached to the GitHub release for reference. That's the only distribution channel. Publishing an iCloud link is a **deferred option** (not approved; it would point at Toby's iCloud account). Revisit shortcut-sign or the iCloud link only if the friend's Mac becomes unavailable.

## 10. Security

- Upload tokens: random 256-bit, prefix `hvu_`, stored hashed (argon2/sha256), scoped `upload` only (can't read data back, can't use the MCP), one per user (single device), revocable/replaceable in the web app, never shown.
- QR code 2 holds only URL + one-time setup code (~10 min, single use, replaced on every page load). Screenshots of an old QR are useless.
- QR code 1 is a short-lived signed download URL (15 min) or requires login; the file holds no secrets anyway.
- Settings can only be changed in the web UI (logged-in session + CSRF); the upload token cannot change settings.
- HTTPS only, terminated by whatever reverse proxy the admin uses ([reverse-proxy.md](reverse-proxy.md)); forwarded headers trusted only from `HEALTHVIBE_TRUSTED_PROXIES`. Server refuses plain HTTP in production config. If the UI is behind SSO, the ingest/config/claim/download paths must bypass it (bearer token protects them).
- Server enforces enabled metrics and per-user isolation; ingest can only write to the token's user.
- Rate limits on claim, config and ingest; request size cap (`max_request_bytes`).
- The signed Shortcut contains **no** secrets or server URLs; it's identical for every user. `hv-config.json` in iCloud Drive holds the token — documented, and users can revoke if the phone or iCloud account is lost.
- Shortcut is readable by anyone (open the actions) and [shortcut-build.md](shortcut-build.md) lists every action, so users can verify exactly what it reads and sends, or build their own.

## 11. Shortcut limits to design around

- **`Find Health Samples` can't take a variable type.** The type is chosen when building the Shortcut, so the master needs one block per metric (~40) each wrapped in `If <enabled list> contains <key>`. Large but straightforward; build it with a consistent block template.
- **Phone must be unlocked.** HealthKit data is encrypted while locked. "Alarm is stopped" usually fires while the user is unlocking; if the read fails, the Shortcut exits quietly and the next run catches up (server cursors).
- **Runtime.** Shortcuts run in the foreground-ish with no hard documented limit, but automations that take tens of seconds or loop over thousands of items get slow/killed. Mitigations: per-metric `Limit` (`max_samples_per_metric`), small `since` windows, and building JSON per metric with "Get Dictionary"/"Combine Text" rather than looping per sample where possible.
- **Payload size.** Heart rate can be thousands of samples per day. Send one request per metric group (batches), keep each under `max_request_bytes` (5 MB default).
- **Units.** Must request a fixed unit per metric in the Shortcut so values are consistent.
- **"Run Immediately"** must be set by the user; Apple doesn't allow imported Shortcuts to create automations.
- **Health permissions** are prompted per type on first run; with ~40 types the first run shows a long permission sheet. Users can deny types they don't want.
- **No anchored queries** → deletions/edits in Health aren't synced (§5).

## 12. Resolved decisions (Nov 3, 2026)

- **Sync settings:** web UI only; must be chosen before any QR code is shown; the Shortcut has no metric or trigger menus (§2a, §3.3).
- **Onboarding:** two QR codes — 1 installs from the healthvibe server, 2 configures with a one-time code (~10 min, new each page load) (§2a).
- **Distribution:** signed file bundled in the release/image and served by the user's server; "Build it yourself" guide; iCloud link deferred (§9).
- **Recovery:** "Help, I messed up my Shortcut" page (§6b).
- **Trigger:** chosen and explained in the web UI (§6a).
- **Devices:** one device per user, single upload token (§3.4).
- **Raw upload JSON:** deleted once parsed (§4).
- **Run notifications:** per-user setting, Always or Only on errors; default Only on errors (§3.2).
- **Sleep score:** fixed v1 formula (§8).
- **No-upload warning:** 36 h, in web app plus per-user email (SMTP) / push (ntfy, Apprise) (§6a).
- **Workouts:** in v1 (§4, metrics.md).
- **Lookback:** 7 days on first run, 30-day max (§3.2).
- **Metric presets in the web UI:** Recommended / Everything / custom.

- **Stack:** Rust (Axum, sqlx/SQLite, React SPA + JSON API), see [architecture.md](architecture.md). Theming per [theming.md](theming.md); custom theme upload (PR13) deferred.

## 13. Open questions

1. **Sleep Focus trigger:** Toby will test on a real iPhone whether "Focus → Sleep → When Turning Off" can run without asking. If not, replace it with "Sleep → Waking Up".
2. **Stack questions** (receiver in the main binary or separate; close PR #1 and redo the scaffold in Rust): see [architecture.md](architecture.md#open-questions-for-toby).
3. **Per-user upload URL** (`/api/v1/ingest/u/<id>` + bearer token is the working assumption here): see [architecture.md](architecture.md#per-user-upload-url-open).
