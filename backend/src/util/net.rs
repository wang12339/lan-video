use axum::extract::{ConnectInfo, Request};
use std::net::{IpAddr, SocketAddr};
use std::sync::OnceLock;

use crate::util::cloudflare_ips::is_cloudflare_peer;

/// 应用启动时用 AppConfig.trusted_proxy 显式配置（优先于 TRUSTED_PROXY env）。
pub fn configure_trusted_proxy(enabled: bool) {
    let _ = TRUSTED_PROXY.set(enabled);
}

static TRUSTED_PROXY: OnceLock<bool> = OnceLock::new();

#[inline]
fn trusted_proxy() -> bool {
    *TRUSTED_PROXY.get_or_init(|| {
        std::env::var("TRUSTED_PROXY")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
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
