//! Runtime configuration, read from `HEALTHVIBE_*` environment variables.

use std::net::IpAddr;
use std::path::PathBuf;

/// One `HEALTHVIBE_TRUSTED_PROXIES` entry: an address block like
/// `10.0.0.0/8`, `fd00::/8`, or a single address (`/32` or `/128`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    addr: IpAddr,
    prefix: u8,
}

impl Cidr {
    pub fn parse(s: &str) -> Option<Cidr> {
        let s = s.trim();
        let (a, p) = match s.split_once('/') {
            Some((a, p)) => (a, Some(p)),
            None => (s, None),
        };
        let addr: IpAddr = a.trim().parse().ok()?;
        let addr = addr.to_canonical();
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix = match p {
            Some(p) => p.trim().parse::<u8>().ok().filter(|&n| n <= max)?,
            None => max,
        };
        Some(Cidr { addr, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let ip = ip.to_canonical();
        let bits = |n: u32, len: u32| if n == 0 { 0 } else { u128::MAX << (len - n) };
        match (self.addr, ip) {
            (IpAddr::V4(a), IpAddr::V4(b)) => {
                let m = bits(self.prefix as u32, 32) as u32;
                u32::from(a) & m == u32::from(b) & m
            }
            (IpAddr::V6(a), IpAddr::V6(b)) => {
                let m = bits(self.prefix as u32, 128);
                u128::from(a) & m == u128::from(b) & m
            }
            _ => false,
        }
    }
}

/// Parse a comma/space separated list of addresses or CIDR blocks.
pub fn parse_cidrs(s: &str) -> Result<Vec<Cidr>, String> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(|p| Cidr::parse(p).ok_or_else(|| p.to_string()))
        .collect()
}

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
    /// Largest accepted Health Auto Export upload in bytes (default 256 MiB).
    pub hae_max_request_bytes: usize,
    /// Largest accepted export.zip upload in bytes (default 4 GiB).
    pub max_import_bytes: u64,
    /// `Strict-Transport-Security` max-age in seconds. Off unless set: only
    /// turn it on once HTTPS works for this hostname.
    pub hsts_max_age: Option<u64>,
    /// Reverse proxies whose `X-Forwarded-*` / `X-Real-IP` headers are
    /// believed (`HEALTHVIBE_TRUSTED_PROXIES`). Empty: those headers are
    /// ignored and the connecting address is the client.
    pub trusted_proxies: Vec<Cidr>,
}

/// Default for `HEALTHVIBE_MAX_IMPORT_BYTES`: 4 GiB.
pub const DEFAULT_MAX_IMPORT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

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
            hae_max_request_bytes: get("HEALTHVIBE_HAE_MAX_REQUEST_BYTES", "268435456")
                .parse()
                .unwrap_or(256 * 1024 * 1024),
            max_import_bytes: std::env::var("HEALTHVIBE_MAX_IMPORT_BYTES")
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .filter(|&n: &u64| n > 0)
                .unwrap_or(DEFAULT_MAX_IMPORT_BYTES),
            hsts_max_age: std::env::var("HEALTHVIBE_HSTS_MAX_AGE")
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .filter(|&n: &u64| n > 0),
            trusted_proxies: parse_cidrs(&get("HEALTHVIBE_TRUSTED_PROXIES", "")).unwrap_or_else(
                |bad| {
                    panic!(
                        "HEALTHVIBE_TRUSTED_PROXIES: '{bad}' is not an IP address or CIDR block (e.g. 172.16.0.0/12)"
                    )
                },
            ),
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.db_dir.join("healthvibe.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_parse_and_contains() {
        let c = parse_cidrs("10.0.0.0/8, 192.168.1.5 fd00::/8").unwrap();
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(c[0].contains(ip("10.255.1.2")));
        assert!(!c[0].contains(ip("11.0.0.1")));
        assert!(c[1].contains(ip("192.168.1.5")));
        assert!(!c[1].contains(ip("192.168.1.6")));
        assert!(c[2].contains(ip("fd12::1")));
        assert!(!c[2].contains(ip("2001:db8::1")));
        // IPv4-mapped IPv6 peers match IPv4 blocks.
        assert!(c[0].contains(ip("::ffff:10.1.2.3")));
        assert!(Cidr::parse("0.0.0.0/0").unwrap().contains(ip("8.8.8.8")));
        assert!(parse_cidrs("").unwrap().is_empty());
        assert_eq!(parse_cidrs("10.0.0.0/33").unwrap_err(), "10.0.0.0/33");
        assert!(parse_cidrs("proxy.local").is_err());
    }
}
