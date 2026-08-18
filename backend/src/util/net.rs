use axum::extract::{ConnectInfo, Request};
use std::net::{IpAddr, SocketAddr};

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

/// Cloudflare's published IPv4 ranges (https://www.cloudflare.com/ips/).
/// Only peers in these ranges may send `cf-connecting-ip` when
/// `TRUSTED_PROXY` is not set.
const CLOUDFLARE_IPV4: &[(&str, u8)] = &[
    ("173.245.48.0", 20),
    ("103.21.244.0", 22),
    ("103.22.200.0", 22),
    ("103.31.4.0", 22),
    ("141.101.64.0", 18),
    ("108.162.192.0", 18),
    ("190.93.240.0", 20),
    ("188.114.96.0", 20),
    ("197.234.240.0", 22),
    ("198.41.128.0", 17),
    ("162.158.0.0", 15),
    ("104.16.0.0", 13),
    ("104.24.0.0", 14),
    ("172.64.0.0", 13),
    ("131.0.72.0", 22),
];

/// Cloudflare's published IPv6 ranges.
const CLOUDFLARE_IPV6: &[(&str, u8)] = &[
    ("2400:cb00::", 32),
    ("2606:4700::", 32),
    ("2803:f800::", 32),
    ("2405:b500::", 32),
    ("2405:8100::", 32),
    ("2a06:98c0::", 29),
    ("2c0f:f248::", 32),
];

fn ipv4_in_network(ip: u32, network: u32, prefix: u8) -> bool {
    let mask = if prefix >= 32 {
        u32::MAX
    } else {
        u32::MAX << (32 - prefix)
    };
    (ip & mask) == (network & mask)
}

fn ipv6_in_network(ip: u128, network: u128, prefix: u8) -> bool {
    let mask = if prefix >= 128 {
        u128::MAX
    } else {
        u128::MAX << (128 - prefix)
    };
    (ip & mask) == (network & mask)
}

fn is_cloudflare_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let ip = u32::from(v4);
            CLOUDFLARE_IPV4.iter().any(|(net, prefix)| {
                net.parse::<std::net::Ipv4Addr>()
                    .map(|n| ipv4_in_network(ip, u32::from(n), *prefix))
                    .unwrap_or(false)
            })
        }
        IpAddr::V6(v6) => {
            let ip = u128::from(v6);
            CLOUDFLARE_IPV6.iter().any(|(net, prefix)| {
                net.parse::<std::net::Ipv6Addr>()
                    .map(|n| ipv6_in_network(ip, u128::from(n), *prefix))
                    .unwrap_or(false)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloudflare_peer_matches_known_range() {
        assert!(is_cloudflare_peer("104.16.42.1".parse().unwrap()));
        assert!(is_cloudflare_peer("172.64.0.1".parse().unwrap()));
        assert!(is_cloudflare_peer(
            "2606:4700:3037::6815:1234".parse().unwrap()
        ));
    }

    #[test]
    fn non_cloudflare_peer_rejected() {
        assert!(!is_cloudflare_peer("8.8.8.8".parse().unwrap()));
        assert!(!is_cloudflare_peer("203.0.113.7".parse().unwrap()));
        assert!(!is_cloudflare_peer("2001:db8::1".parse().unwrap()));
    }
}
