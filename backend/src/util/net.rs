use axum::extract::{ConnectInfo, Request};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::OnceLock;

use crate::util::cloudflare_ips::is_cloudflare_peer;

/// 应用启动时用 AppConfig.trusted_proxy 显式配置（优先于 TRUSTED_PROXY env）。
pub fn configure_trusted_proxy(enabled: bool) {
    let _ = TRUSTED_PROXY_CONF.set(Some(enabled));
}

/// 显式配置可信代理对端白名单：`Some(peers)` = 仅这些对端的代理头被信任；
/// `None` = 未显式配置（回退到 TRUSTED_PROXY_PEERS env 或全部信任）。
pub fn configure_trusted_proxy_peers(peers: Option<Vec<IpAddr>>) {
    let _ = TRUSTED_PROXY_PEERS_CONF.set(peers);
}

static TRUSTED_PROXY_CONF: OnceLock<Option<bool>> = OnceLock::new();
static TRUSTED_PROXY_PEERS_CONF: OnceLock<Option<Vec<IpAddr>>> = OnceLock::new();

#[inline]
fn trusted_proxy() -> bool {
    if let Some(configured) = TRUSTED_PROXY_CONF.get() {
        return configured.unwrap_or(false);
    }
    std::env::var("TRUSTED_PROXY")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

/// 可信代理对端白名单；`None` 表示未限制（任意对端可能被信任，旧行为）。
#[inline]
fn trusted_proxy_peers() -> Option<&'static [IpAddr]> {
    if let Some(configured) = TRUSTED_PROXY_PEERS_CONF.get() {
        return configured.as_deref();
    }
    let parsed: Vec<IpAddr> = std::env::var("TRUSTED_PROXY_PEERS")
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|part| IpAddr::from_str(part.trim()).ok())
                .collect()
        })
        .unwrap_or_default();
    if parsed.is_empty() {
        None
    } else {
        Some(Box::leak(parsed.into_boxed_slice()))
    }
}

/// 供 middleware 判断请求是否来自受信任代理。
#[inline]
pub fn trusted_proxy_is_enabled() -> bool {
    trusted_proxy()
}

#[inline]
pub fn client_ip(req: &Request) -> String {
    let trusted_proxy = trusted_proxy();

    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>();

    if trusted_proxy {
        // 仅当对端来自可信代理白名单时才信任代理头，否则直接伪造头可绕过限流/审计。
        let peer_ip = peer.map(|p| p.0.ip());
        let peer_allowed = match peer_ip {
            Some(ip) => trusted_proxy_peers()
                .map(|peers| peers.contains(&ip))
                .unwrap_or(true), // 未配置白名单 = 旧行为：任意对端信任
            None => false, // 无从判断对端 → 不采信任何代理头
        };
        if peer_allowed {
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
