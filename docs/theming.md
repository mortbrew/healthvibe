# Theming

Status: **draft for review**. See also [architecture.md](architecture.md).

healthvibe's whole look comes from a small set of CSS custom properties ("theme settings"). Users don't write CSS: they pick palettes from a large catalog of terminal color schemes, and healthvibe maps each palette onto those settings. The design follows VibeKit's theme store (`mortbrew/vibekit`, `src/theme_store.rs` and `src/theme.rs`).

## Decisions

- **Language: Rust (resolved).** The theme store is a Rust crate, `hv-themes`, in the healthvibe workspace and is called **in-process** by the Axum app (the whole project is Rust, so the earlier helper-binary/subprocess plan is no longer needed).
- **No DIY theme maker or custom CSS upload at launch.** Coding Team Lead's earlier PR13 (custom theme upload) is **deferred**.
- Downloaded themes are parsed as **color values only**.

## Catalog

| Source | Listing | File fetched |
|---|---|---|
| [mbadolato/iTerm2-Color-Schemes](https://github.com/mbadolato/iTerm2-Color-Schemes) `alacritty/` (~900 schemes) | `GET https://api.github.com/repos/mbadolato/iTerm2-Color-Schemes/contents/alacritty` (one entry per `.toml`, uses `download_url`) | the `.toml` |
| [Omarchy](https://github.com/omacom/omarchy) themes | `GET https://api.github.com/repos/omacom/omarchy/contents/themes` (one dir per theme) | `https://raw.githubusercontent.com/omacom/omarchy/HEAD/themes/<name>/colors.toml`, falling back to `alacritty.toml` |
| Built-ins | compiled in | — (always available, offline) |

- Fetched **server-side** only (browsers never call GitHub). The two listings are cached in `./appdata/themes/catalog.json` for 24 h; a "Refresh" button (admin) forces a refetch.
- **Rate limits:** unauthenticated GitHub API = 60 requests/hour per IP. The catalog needs 2 API calls; raw file downloads from raw.githubusercontent.com don't count against the API limit. Optional `GITHUB_TOKEN` in `.env` raises it to 5,000/hour. A 403/429 shows "GitHub's rate limit: try again in an hour" and keeps serving the cached catalog.
- Slugs: lowercase, `[a-z0-9-]` only, prefixed by source (`iterm-`, `omarchy-`) so a download can never shadow a built-in (same rule as VibeKit).
- Timeouts (20 s), size cap per file (64 KB), User-Agent `healthvibe/<version>`.

## UI

Settings → Appearance:

- **Mode toggle (three-way):** Light · Dark · Follow system. Follow system uses `prefers-color-scheme` (CSS media query, no JS needed), so the page flips when the OS does.
- **Palette pickers:** one inside each mode — the user picks a **light palette** and a **dark palette**. Follow system uses whichever matches.
- **Installed tab:** built-ins plus the user's downloads, each with a live preview card (surface, text, ring and chart swatches). Actions: **Use for Light**, **Use for Dark**, **Delete** (built-ins can't be deleted).
- **Search tab:** the full catalog, filtered as you type (HTMX request with a short debounce, server-side substring/fuzzy match on name). Filter chips: source (iTerm2 / Omarchy), "looks light" / "looks dark" (by background luminance). Action: **Download** (then Use).
- Downloads are saved per user in `./appdata/themes/<user_id>/<slug>.toml`. Deleting a theme that's in use falls back to the built-in default for that mode.

## Mapping a scheme onto healthvibe's theme settings

Input: 16 ANSI colors (normal + bright black, red, green, yellow, blue, magenta, cyan, white) plus background, foreground, selection (and Omarchy's `accent`, `muted`, etc. when present). Parsing follows VibeKit's `parse_alacritty` / `parse_colors_toml` (Omarchy 4 names or Omarchy 3 `color0`–`color15`), with `#rrggbb`, `0xrrggbb` or `rrggbb` accepted.

| healthvibe setting | Source | Fallback / check |
|---|---|---|
| `--surface-0` (page) | background | `black` |
| `--surface-1`, `--surface-2` (cards, raised) | `mix(bg, fg, 0.05)`, `mix(bg, fg, 0.10)` | — |
| `--border` | Omarchy `muted` if ≠ bg | `mix(bg, fg, 0.25)` |
| `--text` | foreground | `white`; must reach **4.5:1** on all surfaces, else mixed toward white/black until it does |
| `--text-muted` | `dim_foreground` / `dark_foreground` if ≥ 3:1 | `mix(fg, bg, 0.45)` (VibeKit uses ≥ 2:1; healthvibe raises it to WCAG 3:1 for large text) |
| `--selection` | selection bg if contrast to bg < 2 and to fg ≥ 3 | `mix(bg, fg, 0.14)` |
| `--accent` | Omarchy `accent` | blue |
| `--ok`, `--warn`, `--bad` | green, yellow, red | orange = `mix(red, yellow, 0.5)` if missing |
| Ring colors: Move / Exercise / Stand | red, green, cyan (Apple-like) | bright variants if normal < 3:1 on surface |
| Sleep stages: Awake / REM / Core / Deep | yellow, cyan, blue, magenta | bright variants; stages must differ from each other by ≥ 1.5:1 or the next candidate is used |
| Chart series 1–8 | blue, green, magenta, yellow, cyan, red, orange, bright-blue | any series < **3:1** against `--surface-1` (WCAG non-text contrast) is swapped for its bright/normal pair, then mixed toward `--text` until it passes |

Contrast uses the WCAG 2.x relative-luminance formula and `mix(a, b, t)` linear RGB interpolation, exactly like VibeKit's `luminance`, `contrast` and `mix`. A scheme with no background/foreground or missing core colors is rejected ("not a usable color scheme"). Mode hints: a palette is "light" if background luminance > 0.5; the picker shows all palettes but sorts matching ones first.

Output is a typed `ThemeSettings` struct, rendered into a `<style>` block of `--name: #rrggbb;` lines (hex validated and re-formatted by the server). ECharts reads the same variables via `getComputedStyle` when it initializes and when the mode changes.

## Security

- Downloaded files are parsed with a TOML parser and only `#rrggbb`-style values are extracted; everything else is ignored. The stored file is re-checked on load.
- Nothing from a theme file is ever emitted as raw CSS, HTML or JS. The server emits only values it formatted itself (`#` + 6 hex digits), so injection is impossible.
- Fetching is server-side, limited to `api.github.com` and `raw.githubusercontent.com` URLs built from the listing (no user-supplied URLs), with timeouts and a size cap.
- Per-user directories; slugs are validated, so no path traversal.

## Rust implementation plan (`hv-themes` crate)

VibeKit's theme modules can't be used as a git dependency as-is: `theme.rs` builds `ratatui::style::Color` values and the crate pulls in the TUI. Options, in order of preference:

1. **Upstream a small refactor in vibekit** (separate PR there, with Toby's OK): move the palette parsing, `luminance`/`contrast`/`mix` and the GitHub catalog code into a UI-free module or a `theme-core` crate behind a feature flag (`default-features = false` drops ratatui). Then `hv-themes` depends on it via `git = "https://github.com/mortbrew/vibekit"` pinned to a rev.
2. Until then (or if that isn't wanted): **vendor a copy** of the relevant functions into `crates/hv-themes/src/vendor/vibekit_theme.rs` with an attribution header (VibeKit, MIT, copyright Toby Moon, source path and commit), using plain `(u8, u8, u8)` instead of ratatui colors.

Crate API (used in-process by the Axum handlers):

```rust
pub async fn catalog(cache: &Path, refresh: bool, token: Option<&str>) -> Result<Vec<Entry>>;
pub async fn fetch(entry: &Entry, user_dir: &Path) -> Result<Installed>;   // download + validate + save <slug>.toml
pub fn remove(user_dir: &Path, slug: &str) -> Result<()>;
pub fn installed(user_dir: &Path) -> Vec<Installed>;                       // built-ins + downloads
pub fn parse_any(text: &str) -> Option<Palette>;                           // alacritty or Omarchy colors.toml
pub fn map(p: &Palette) -> ThemeSettings;                                  // contrast checks + fallbacks
```

Dependencies: `reqwest` (rustls), `serde`, `serde_json`, `toml`. A `demo` catalog (no network) for tests, like VibeKit's.

A thin CLI wrapper (`healthvibe themes catalog|fetch|remove|map <file>` printing JSON) is useful for debugging and admin scripts, but the app never shells out to it.

Tests (in CI with the rest of the workspace: `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`): listing parsing, slug rules, every built-in passing the contrast checks, mapping a sample of real schemes (fixtures checked in), injection attempts in theme files being rejected.

## Built-ins

Ship with: **healthvibe Dark** (the mockup look), **healthvibe Light**, **High contrast (dark)**, **High contrast (light)**, plus a few popular schemes compiled in (e.g. Tokyo Night, Catppuccin Latte/Mocha, Solarized Light/Dark) so the store works offline.
