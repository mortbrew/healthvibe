# Build the healthvibe Shortcut yourself

Server side: see [architecture.md](architecture.md).

The signed Shortcut served by your healthvibe server (Setup → QR code 1) is built exactly as described here. Use this guide to **verify** what the installed Shortcut does (open it in Shortcuts and compare action by action) or to **recreate** it on your own iPhone without installing anyone else's file. No Mac needed.

Status: draft. Action names are from the iOS 26 Shortcuts app; verify them when the master Shortcut is built. Screenshots are planned (see [Screenshots](#screenshots)).

Conventions: `Var` = a named variable (Set Variable). "→" = the action's output feeds the next action. Name the Shortcut exactly **healthvibe** so automations and update links keep working.

## Block 0 — Version and constants
1. **Text** `0.1.0` → Set Variable `HV_VERSION`.
2. **Text** `healthvibe/hv-config.json` → Set Variable `CONFIG_PATH`.

## Block 1 — Load settings (or first-run QR setup)
3. **Get File** from Shortcuts folder, path `CONFIG_PATH`, *Error If Not Found* **off**.
4. **If** File *does not have any value*:
   1. **Scan QR or Barcode** → Set Variable `QR`.
   2. **Get URLs from Input** `QR` / **Get Components of URL** → query `server`, `code` (or **Match Text** `server=([^&]+)` and `code=([^&]+)`).
   3. **Get Contents of URL** `POST {server}/api/v1/shortcut/claim`, JSON body `{"setup_code": code, "device_name": Device Name, "shortcut_version": HV_VERSION}`.
   4. **Get Dictionary Value** `upload_token` → `TOKEN`.
   5. **Dictionary** `{server, token: TOKEN, device: Device Name, configured_at: Current Date}` → **Save File** to `CONFIG_PATH`, *Ask Where to Save* off, *Overwrite* on.
5. **Otherwise**: **Get Dictionary from Input** (file) → `SERVER`, `TOKEN`.
6. **End If**.

## Block 2 — Fetch config
7. **Get Contents of URL** `GET {SERVER}/api/v1/shortcut/config`, header `Authorization: Bearer {TOKEN}`, `X-HV-Version: {HV_VERSION}`.
8. **If** status is `426`: **Show Alert** "Update healthvibe: open your healthvibe Setup page" → **Stop This Shortcut**. If `401`: **Show Notification** "healthvibe token was reset — see Help page" → **Stop**.
9. **Get Dictionary Value** `enabled` → `ENABLED`; `since` → `SINCE`; `notify` → `NOTIFY`; `max_samples_per_metric` → `LIMIT`.
10. **Dictionary** (empty) → Set Variable `PAYLOAD_METRICS`.

## Block 3 — One block per metric (template)
Repeat for every row of [metrics.md](metrics.md); the Health type and unit are fixed per block (Shortcuts can't use a variable type).

11. **If** `ENABLED` *contains* `stepCount`:
    1. **Get Dictionary Value** `stepCount` from `SINCE` → `S`.
    2. **Find Health Samples** where *Type is Steps* and *Start Date is after* `S`, sort by Start Date ascending, *Limit* `LIMIT`.
    3. **Repeat with Each** sample: **Dictionary** `{start: Start Date (ISO 8601), end: End Date (ISO 8601), value: Value (in count), source: Source}` → **Add to Variable** `ROWS`.
    4. **Dictionary** `{unit: "count", samples: ROWS}` → **Set Dictionary Value** key `stepCount` in `PAYLOAD_METRICS`. Clear `ROWS`.
12. **End If**.

Category types (e.g. Sleep Analysis, Stand Hour) use the same block; `value` is the sample's category value text (`asleepCore`, `stood`, …).

## Block 4 — Workouts
13. **If** `ENABLED` contains `workouts`: **Find Workouts** where *Start Date is after* `SINCE.workouts`, then per workout a Dictionary `{type, start, end, duration_s, active_energy_kcal, distance_km, avg_hr, max_hr, source}` → `WORKOUTS`.

## Block 5 — Upload
14. **Dictionary** `{schema: 1, shortcut_version: HV_VERSION, device: Device Name, run_id: (UUID via Text "Current Date" hashed/random number), tz: Current Time Zone, metrics: PAYLOAD_METRICS, workouts: WORKOUTS}`.
15. **Get Contents of URL** `POST {SERVER}/api/v1/ingest` (same fixed URL for every user), header `Authorization: Bearer {TOKEN}` (the token is never put in the URL), body = JSON (the dictionary). Large runs are split into several requests by metric group (batch index/count).
16. **If** request failed or response has `rejected` entries, **or** `NOTIFY` = `always`: **Show Notification** with `accepted`/`duplicates`/error text.

That's the whole Shortcut. It never shows a menu; the only prompt is the QR scan when no settings file exists.

## Build and sign it yourself (Mac)

Any user can skip the server-served file (QR code 1): build the Shortcut from this guide and, if you have a Mac, sign it yourself.

**Requirements:** a Mac with macOS 12 Monterey or later (the `shortcuts` command-line tool ships with macOS at `/usr/bin/shortcuts`), signed into an Apple ID/iCloud, with iCloud sync for Shortcuts on if you build on iPhone. Network access is needed for `--mode anyone` (Apple notarizes through iCloud and receives a copy of the Shortcut). **Xcode is not needed** — the `shortcuts` tool is part of macOS, not Xcode.

1. **Build** the Shortcut named **healthvibe**, block by block as above, in the Shortcuts app on macOS — or build it on your iPhone and let iCloud sync it to the Mac.
2. **Export** it: in Shortcuts on the Mac, **File → Export** (or **Share → Export File**). Save as `healthvibe.shortcut`. (Export itself asks who can run it and produces a signed file; the CLI below does the same thing explicitly and is easy to script.)
3. **Sign** it in Terminal:
   ```sh
   # anyone can import (notarized via iCloud)
   shortcuts sign --mode anyone --input healthvibe.shortcut --output healthvibe-signed.shortcut
   # or: sign locally; only your own devices / people who have you in Contacts can import
   shortcuts sign --mode people-who-know-me --input healthvibe.shortcut --output healthvibe-signed.shortcut
   ```
   Short flags: `-m`, `-i`, `-o`. Default mode is `people-who-know-me`, which is enough for your own iPhone. Exit code 0 = success.
4. **Transfer** `healthvibe-signed.shortcut` to the iPhone (AirDrop, Files/iCloud Drive, or email to yourself) and tap it to import into Shortcuts.
5. **Configure** with **QR code 2** on the Setup page (or Help page), then set up the trigger as shown there.

No Mac? You can still build it directly on your iPhone; a Shortcut you create on your own phone doesn't need signing at all. Then configure with QR code 2.

**Contract a self-built Shortcut must keep** (the server checks it):
- Name **healthvibe** (automations and help text refer to it).
- Send `X-HV-Version` / `shortcut_version` matching a supported version (≥ `min_shortcut_version` from config; else `426`). Use the version printed at the top of this guide for your server's release.
- Save settings to `iCloud Drive/Shortcuts/healthvibe/hv-config.json` with keys `server`, `token`, `device`, `configured_at`.
- Use the claim/config/ingest endpoints and the upload JSON schema in [shortcut-design.md](shortcut-design.md) §3–4 exactly: metric keys and units from [metrics.md](metrics.md), only metrics in `enabled`, samples after `since`.
- When the server releases a new Shortcut version, re-check this guide and update your copy.

Sources: `shortcuts(1)` man page ([mirror](https://keith.github.io/xcode-man-pages/shortcuts.1.html)), `shortcuts sign -h` output ([Late Night Software forum](https://forum.latenightsw.com/t/can-sd-notary-sign-a-shortcut/4324)), [ss64: shortcuts](https://ss64.com/mac/shortcuts.html).

## Screenshots
Planned, captured on iOS 26 for each block above and for:
- Setup page steps 1–4 (choose metrics/trigger, QR 1, QR 2, finish).
- Importing the signed file from QR code 1.
- Health permission sheet on first run.
- Creating each trigger's automation with Run Immediately.
- Every step of the "Help, I messed up my Shortcut" page.

Stored under `docs/img/shortcut/` with names like `block-03-find-health-samples.png`.
