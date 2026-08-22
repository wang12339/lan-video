use axum::extract::{ConnectInfo, Request};
use std::net::{IpAddr, SocketAddr};

use crate::util::cloudflare_ips::is_cloudflare_peer;

/// Extract the real client IP from Cloudflare headers (when present) and fall
/// back to the direct socket peer. Used by rate limiters and auth middleware.
///
/// SECURITY: this IP feeds per-IP rate limits, so trusting a spoofable header
/// would let attackers evade them. `cf-connecting-ip` is therefore only
/// honoured when the direct peer is inside Cloudflare's published ranges
/// (origin sits behind Cloudflare in production), or unconditionally when
/// `TRUSTED_PROXY=1` is set for custom proxies. A peer connecting straight to
/// the origin can never spoof the client IP used for rate limiting.
pub fn client_ip(req: &Request) -> String {
    let trusted_proxy = std::env::var("TRUSTED_PROXY")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>();

    if trusted_proxy {
        // X-Forwarded-For is "client, proxy1, proxy2" — the leftmost entry is
        // the client. cf-connecting-ip (Cloudflare) may also be present.
        for name in ["cf-connecting-ip", "x-forwarded-for"] {
            if let Some(ip) = req
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .map(str::trim)
                .and_then(|s| s.parse::<IpAddr>().ok())
            {
                return ip.to_string();
            }
        }
    } else if peer.map(|p| is_cloudflare_peer(p.0.ip())).unwrap_or(false) {
        if let Some(ip) = req
            .headers()
            .get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<IpAddr>().ok())
        {
            return ip.to_string();
        }
    }

    peer.map(|addr| addr.0.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}
