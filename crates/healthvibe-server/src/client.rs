//! Who is on the other end of a request.
//!
//! `X-Forwarded-For`, `X-Real-IP`, `X-Forwarded-Host` and `X-Forwarded-Proto`
//! are only believed when the TCP peer is in `HEALTHVIBE_TRUSTED_PROXIES`.
//! Otherwise anyone could set them, so the connecting address is the client
//! and the `Host` header is the address.

use crate::AppState;
use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{request::Parts, HeaderMap},
};
use healthvibe_core::config::Cidr;
use std::net::{IpAddr, SocketAddr};

/// The resolved client for a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// Client address as text (`"unknown"` when there is no socket, e.g. tests).
    pub ip: String,
    /// The TCP peer is a trusted reverse proxy, so its forwarding headers count.
    pub via_trusted_proxy: bool,
}

fn trusted(list: &[Cidr], ip: IpAddr) -> bool {
    list.iter().any(|c| c.contains(ip))
}

/// Resolve the client from the socket peer and headers.
///
/// From a trusted peer: walk `X-Forwarded-For` right to left (each proxy
/// appends the address it saw) and take the first hop that isn't itself a
/// trusted proxy; then `X-Real-IP`; then the peer. From any other peer the
/// headers are ignored.
pub fn resolve(list: &[Cidr], peer: Option<IpAddr>, h: &HeaderMap) -> Client {
    let Some(peer) = peer.map(|p| p.to_canonical()) else {
        return Client {
            ip: "unknown".into(),
            via_trusted_proxy: false,
        };
    };
    if !trusted(list, peer) {
        return Client {
            ip: peer.to_string(),
            via_trusted_proxy: false,
        };
    }
    let parse = |s: &str| s.trim().parse::<IpAddr>().ok().map(|i| i.to_canonical());
    let hops: Vec<IpAddr> = h
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(parse)
        .collect();
    let ip = hops
        .iter()
        .rev()
        .copied()
        .find(|ip| !trusted(list, *ip))
        .or_else(|| {
            h.get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .and_then(parse)
                .filter(|ip| !trusted(list, *ip))
        })
        .unwrap_or(peer);
    Client {
        ip: ip.to_string(),
        via_trusted_proxy: true,
    }
}

pub fn from_parts(st: &AppState, p: &Parts) -> Client {
    let peer = p
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    resolve(&st.config.trusted_proxies, peer, &p.headers)
}

impl FromRequestParts<AppState> for Client {
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(p: &mut Parts, st: &AppState) -> Result<Self, Self::Rejection> {
        Ok(from_parts(st, p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use healthvibe_core::config::parse_cidrs;

    fn hm(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        h
    }

    #[test]
    fn forwarding_headers_only_from_trusted_peers() {
        let list = parse_cidrs("10.0.0.0/8").unwrap();
        let ip = |s: &str| Some(s.parse().unwrap());
        let spoof = hm(&[("x-forwarded-for", "1.2.3.4"), ("x-real-ip", "1.2.3.4")]);
        // Untrusted peer: headers ignored.
        assert_eq!(resolve(&list, ip("203.0.113.9"), &spoof).ip, "203.0.113.9");
        assert_eq!(resolve(&[], ip("10.0.0.2"), &spoof).ip, "10.0.0.2");
        // Trusted peer: rightmost untrusted hop (client-supplied left part ignored).
        let h = hm(&[("x-forwarded-for", "6.6.6.6, 198.51.100.7, 10.0.0.3")]);
        let c = resolve(&list, ip("10.0.0.2"), &h);
        assert_eq!((c.ip.as_str(), c.via_trusted_proxy), ("198.51.100.7", true));
        // X-Real-IP fallback, then the peer.
        assert_eq!(
            resolve(&list, ip("10.0.0.2"), &hm(&[("x-real-ip", "198.51.100.8")])).ip,
            "198.51.100.8"
        );
        assert_eq!(
            resolve(
                &list,
                ip("10.0.0.2"),
                &hm(&[("x-forwarded-for", "garbage")])
            )
            .ip,
            "10.0.0.2"
        );
        // Multiple XFF headers are one list.
        let h = hm(&[
            ("x-forwarded-for", "6.6.6.6"),
            ("x-forwarded-for", "198.51.100.9"),
        ]);
        assert_eq!(resolve(&list, ip("10.0.0.2"), &h).ip, "198.51.100.9");
        // IPv4-mapped peer.
        assert!(resolve(&list, ip("::ffff:10.0.0.2"), &h).via_trusted_proxy);
        assert_eq!(resolve(&list, None, &spoof).ip, "unknown");
    }
}
