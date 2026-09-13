//! util::net — client_ip 的 TRUSTED_PROXY_PEERS 白名单行为。
//!
//! TRUSTED_PROXY / TRUSTED_PROXY_PEERS 由 `client_ip` 每次读取（未显式
//! configure 时），同一进程内并行测试会互相污染 env，因此本文件以单个
//! 串行测试函数覆盖全部场景（service_misc_tests.rs 只覆盖关闭路径）。

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use atmos_video_backend::util::net::client_ip;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use std::net::{Ipv6Addr, SocketAddr};

fn ip_req(peer: Option<SocketAddr>, cf: Option<&str>, xff: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder();
    if let Some(addr) = peer {
        builder = builder.extension(ConnectInfo(addr));
    }
    if let Some(v) = cf {
        builder = builder.header("cf-connecting-ip", v);
    }
    if let Some(v) = xff {
        builder = builder.header("x-forwarded-for", v);
    }
    builder.body(Body::empty()).unwrap()
}

#[test]
fn client_ip_trusted_proxy_peers_allowlist() {
    // ── 场景 1：TRUSTED_PROXY=1 + 白名单 → 白名单内对端采纳 cf-connecting-ip
    std::env::set_var("TRUSTED_PROXY", "1");
    std::env::set_var("TRUSTED_PROXY_PEERS", "127.0.0.1,::1,192.168.66.1");

    let req = ip_req(
        Some(SocketAddr::from(([192, 168, 66, 1], 53454))),
        Some("203.0.113.9"),
        None,
    );
    assert_eq!(client_ip(&req), "203.0.113.9");

    // ── 场景 2：白名单外对端伪造头必须被忽略 → 回退对端 IP
    let req = ip_req(
        Some(SocketAddr::from(([192, 168, 66, 99], 53454))),
        Some("203.0.113.9"),
        Some("6.6.6.6"),
    );
    assert_eq!(
        client_ip(&req),
        "192.168.66.99",
        "白名单外对端伪造 cf-connecting-ip / XFF 必须被忽略"
    );

    // ── 场景 3：IPv6 回环在名单内，XFF 也被采信
    let req = ip_req(
        Some(SocketAddr::from((
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1),
            12345,
        ))),
        None,
        Some("7.7.7.7"),
    );
    assert_eq!(client_ip(&req), "7.7.7.7");

    // ── 场景 4：无对端信息时不采信任何头（防语义歧义）
    let req = ip_req(None, Some("203.0.113.9"), None);
    assert_eq!(client_ip(&req), "unknown");

    // ── 场景 5：未配置白名单 = 旧行为，任意对端信任
    std::env::remove_var("TRUSTED_PROXY_PEERS");
    let req = ip_req(
        Some(SocketAddr::from(([8, 8, 8, 8], 12345))),
        Some("203.0.113.9"),
        None,
    );
    assert_eq!(client_ip(&req), "203.0.113.9");

    // ── 场景 6：TRUSTED_PROXY 关闭 → 非 CF 对端伪造头被忽略（回归）
    std::env::remove_var("TRUSTED_PROXY");
    let req = ip_req(
        Some(SocketAddr::from(([8, 8, 8, 8], 12345))),
        Some("203.0.113.9"),
        None,
    );
    assert_eq!(
        client_ip(&req),
        "8.8.8.8",
        "TRUSTED_PROXY 关闭时非 CF 对端 cf-connecting-ip 被忽略"
    );

    std::env::remove_var("TRUSTED_PROXY_PEERS");
}
