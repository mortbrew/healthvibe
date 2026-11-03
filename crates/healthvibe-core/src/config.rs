//! Runtime configuration, read from `HEALTHVIBE_*` environment variables.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// Address the HTTP server binds to (default `0.0.0.0:8000`).
    pub bind: String,
    /// Directory holding `healthvibe.db` (default `/data/db`).
    pub db_dir: PathBuf,
    /// Directory for temporary uploads (default `/data/staging`).
    pub staging_dir: PathBuf,
    /// Allow session cookies without the `Secure` flag (plain-HTTP testing only).
    pub allow_http: bool,
    /// Directory with the built web app (default `/web`).
    pub web_dir: PathBuf,
    /// Public HTTPS address the iPhone Shortcut uploads to, e.g.
    /// `https://health.example.com` (put in the setup QR code). When unset,
    /// the address the browser used is shown instead.
    pub public_url: Option<String>,
    /// Largest accepted Shortcut upload in bytes (default 5 MiB).
    pub max_request_bytes: usize,
}

impl Config {
    pub fn from_env() -> Self {
        let get = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        Self {
            bind: get("HEALTHVIBE_BIND", "0.0.0.0:8000"),
            db_dir: get("HEALTHVIBE_DB_DIR", "/data/db").into(),
            staging_dir: get("HEALTHVIBE_STAGING_DIR", "/data/staging").into(),
            allow_http: matches!(
                get("HEALTHVIBE_ALLOW_HTTP", "false")
                    .to_ascii_lowercase()
                    .as_str(),
                "1" | "true" | "yes"
            ),
            web_dir: get("HEALTHVIBE_WEB_DIR", "/web").into(),
            public_url: std::env::var("HEALTHVIBE_PUBLIC_URL")
                .ok()
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty()),
            max_request_bytes: get("HEALTHVIBE_MAX_REQUEST_BYTES", "5242880")
                .parse()
                .unwrap_or(5 * 1024 * 1024),
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.db_dir.join("healthvibe.db")
    }
}
