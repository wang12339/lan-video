//! 路由注册与 OpenAPI 文档一致性测试
//!
//! 背景
//! ----
//! OpenAPI 文档在 `backend/src/openapi.rs` 中手写维护（`openapi::spec()`），
//! 路由在 `backend/src/app.rs` 中注册。两者独立演进，容易漂移：
//!
//! 1. 新增了路由但忘了更新 OpenAPI 文档（文档缺 operation）；
//! 2. OpenAPI 文档声明了 operation，但路由根本不存在（文档超前或误写）；
//! 3. 路径/方法写法不一致（如 transcode 状态接口的路径不同）。
//!
//! 本测试直接调用 `atmos_video_backend::openapi::spec()` 解析文档 JSON，
//! 与 app.rs 的路由注册清单做双向比对，把差异完整打印到失败消息中。
//! 不启动服务器、不连数据库、不占端口，可随时离线运行。
//!
//! 维护注意
//! --------
//! - `registered_routes()` 是 app.rs 的镜像清单，新增/删除/改动路由时必须同步；
//! - 若某个路由有意不写进文档，把它加进 `INTENTIONALLY_OMITTED` 并注明原因，
//!   而不是改断言逻辑（静态资源 / 内部页面除外，参见该清单）。
//!
//! 运行：`cargo test --test openapi_route_tests`

use std::collections::BTreeSet;

use atmos_video_backend::openapi;

/// OpenAPI 3.1 规范中的 HTTP 方法（路径条目下的合法键）。
const HTTP_METHODS: &[&str] = &[
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// 已注册但有意不写进 OpenAPI 文档的路径。
/// `method == "*"` 表示任意 HTTP 方法（nest_service 挂载的静态服务）。
struct IntentionallyOmitted {
    method: &'static str,
    path_prefix: &'static str,
    reason: &'static str,
}

const INTENTIONALLY_OMITTED: &[IntentionallyOmitted] = &[
    IntentionallyOmitted {
        method: "GET",
        path_prefix: "/",
        reason: "根路径 301 重定向到 /webapp/，非 API",
    },
    IntentionallyOmitted {
        method: "*",
        path_prefix: "/webapp",
        reason: "SPA 静态资源（ServeDir 提供，非 REST API）",
    },
    IntentionallyOmitted {
        method: "*",
        path_prefix: "/media",
        reason: "媒体文件流（ServeDir + media_auth，非 REST API）",
    },
    IntentionallyOmitted {
        method: "GET",
        path_prefix: "/docs",
        reason: "OpenAPI 文档页面自身（/docs、/docs/openapi.json）",
    },
];

/// app.rs 路由注册清单（镜像，须随 app.rs 同步维护）。
/// 分组与 app.rs 中的 Router::new() 区块一一对应。
fn registered_routes() -> Vec<(&'static str, &'static str)> {
    let mut routes: Vec<(&'static str, &'static str)> = Vec::new();

    // 公开路由（无鉴权，app.rs `public_routes`）
    for (m, p) in [
        ("GET", "/health"),
        ("POST", "/auth/register"),
        ("POST", "/auth/login"),
        ("POST", "/auth/forgot-password"),
        ("GET", "/auth/reset-password"),
        ("POST", "/auth/reset-password"),
        ("GET", "/auth/verify-email"),
        ("POST", "/auth/verify-email"),
        ("GET", "/tags"),
        ("GET", "/tags/popular"),
        ("GET", "/tags/{id}"),
        ("GET", "/recommendations/trending"),
        ("GET", "/recommendations/recent"),
        ("GET", "/recommendations/similar/{video_id}"),
        ("GET", "/share/{token}"),
    ] {
        routes.push((m, p));
    }

    // bearer 鉴权路由（app.rs `auth_routes`）
    for (m, p) in [
        ("GET", "/auth/user"),
        ("GET", "/auth/user/profile"),
        ("PUT", "/auth/user/email"),
        ("POST", "/auth/user/avatar"),
        ("GET", "/auth/user/shares"),
        ("DELETE", "/auth/user/shares/{share_id}"),
        ("POST", "/auth/logout"),
        ("POST", "/auth/send-verification-email"),
        ("POST", "/admin/track"),
        ("GET", "/recommendations"),
        ("GET", "/playlists"),
        ("POST", "/playlists"),
        ("GET", "/playlists/{id}"),
        ("PUT", "/playlists/{id}"),
        ("DELETE", "/playlists/{id}"),
        ("GET", "/playlists/{id}/videos"),
        ("POST", "/playlists/{id}/videos"),
        ("DELETE", "/playlists/{id}/videos/{video_id}"),
        ("GET", "/videos/{id}/comments"),
        ("POST", "/videos/{id}/comments"),
        ("GET", "/comments/{id}/replies"),
        ("DELETE", "/comments/{id}"),
        ("POST", "/videos/{id}/share"),
        ("DELETE", "/videos/{id}/share/{share_id}"),
    ] {
        routes.push((m, p));
    }

    // 视频列表/详情路由（app.rs `video_routes`）
    for (m, p) in [
        ("GET", "/videos"),
        ("GET", "/videos/favorites"),
        ("GET", "/videos/search"),
        ("GET", "/videos/search/suggest"),
        ("GET", "/videos/{id}"),
        ("GET", "/videos/{id}/variants"),
        ("POST", "/videos/{id}/view"),
        ("POST", "/videos/{id}/like"),
        ("GET", "/videos/{id}/like"),
        ("POST", "/videos/{id}/favorite"),
        ("GET", "/videos/{id}/favorite"),
        ("GET", "/videos/{id}/tags"),
        ("POST", "/videos/{id}/tags"),
        ("DELETE", "/videos/{id}/tags"),
        ("DELETE", "/videos/{id}/tags/{tag_id}"),
    ] {
        routes.push((m, p));
    }

    // 播放历史与播放会话（app.rs `playback_routes`）
    for (m, p) in [
        ("GET", "/playback/history/{video_id}"),
        ("GET", "/playback/history"),
        ("POST", "/playback/history"),
        ("POST", "/playback/session/start"),
        ("POST", "/playback/session/heartbeat"),
        ("POST", "/playback/session/stop"),
    ] {
        routes.push((m, p));
    }

    // 上传路由（app.rs `upload_route`）
    for (m, p) in [
        ("POST", "/admin/videos/upload"),
        ("POST", "/admin/videos/upload-resume"),
        ("GET", "/admin/videos/upload-status"),
    ] {
        routes.push((m, p));
    }

    // 管理路由（app.rs `admin_routes`）
    for (m, p) in [
        ("GET", "/admin/users"),
        ("DELETE", "/admin/users/{id}"),
        ("POST", "/admin/videos/external"),
        ("POST", "/admin/videos/check-hashes"),
        ("POST", "/admin/videos/check-files"),
        ("POST", "/admin/videos/scan"),
        ("POST", "/admin/videos/backfill-thumbnails"),
        ("DELETE", "/admin/videos/batch"),
        ("PUT", "/admin/videos/{id}"),
        ("DELETE", "/admin/videos/{id}"),
        ("POST", "/admin/videos/{id}/cover"),
        ("PUT", "/admin/videos/batch-category"),
        ("POST", "/admin/videos/{id}/transcode"),
        ("GET", "/admin/videos/{id}/transcode/status"),
        ("DELETE", "/admin/videos/{id}/transcode/{resolution}"),
        ("POST", "/admin/videos/{id}/transcode/cancel"),
        ("POST", "/admin/tags"),
        ("PUT", "/admin/tags/{id}"),
        ("DELETE", "/admin/tags/{id}"),
        ("GET", "/admin/stats"),
        ("PUT", "/admin/users/{id}/password"),
        ("PUT", "/admin/users/{id}/admin"),
        ("PUT", "/admin/users/{id}/approve"),
        ("POST", "/admin/users/{id}/kick"),
        ("GET", "/admin/config/registration"),
        ("PUT", "/admin/config/registration"),
        ("GET", "/admin/system"),
        ("GET", "/admin/logs"),
        ("DELETE", "/admin/logs"),
    ] {
        routes.push((m, p));
    }

    // 内部监控路由（app.rs `internal_routes`）
    for (m, p) in [
        ("GET", "/server/info"),
        ("GET", "/metrics"),
        ("GET", "/metrics/prometheus"),
    ] {
        routes.push((m, p));
    }

    // OpenAPI 文档路由（app.rs `docs_routes`，见 INTENTIONALLY_OMITTED）
    for (m, p) in [("GET", "/docs/openapi.json"), ("GET", "/docs")] {
        routes.push((m, p));
    }

    // 根路径重定向与 nest_service 静态挂载
    routes.push(("GET", "/"));
    routes.push(("*", "/webapp"));
    routes.push(("*", "/media"));

    routes
}

/// 从 `openapi::spec()` 解析文档中声明的 (METHOD, path) 集合。
fn openapi_routes() -> Vec<(String, String)> {
    let spec = openapi::spec();
    let paths = spec
        .get("paths")
        .and_then(|p| p.as_object())
        .expect("OpenAPI spec 必须包含 paths 对象");
    let mut routes = Vec::new();
    for (path, item) in paths {
        let item = item
            .as_object()
            .unwrap_or_else(|| panic!("path {path} 必须是对象"));
        for (method, _op) in item {
            if HTTP_METHODS.contains(&method.as_str()) {
                routes.push((method.to_uppercase(), path.clone()));
            }
        }
    }
    routes
}

/// 判断 (method, path) 是否命中"有意不写进文档"清单。
fn is_intentionally_omitted(method: &str, path: &str) -> bool {
    INTENTIONALLY_OMITTED.iter().any(|e| {
        let method_ok = e.method == "*" || e.method == method;
        if !method_ok {
            return false;
        }
        if e.path_prefix == "/" {
            path == "/"
        } else {
            let prefix = format!("{}/", e.path_prefix);
            path == e.path_prefix || path.starts_with(&prefix)
        }
    })
}

struct DiffReport {
    /// 已注册路由但 OpenAPI 文档中没有
    missing: Vec<String>,
    /// OpenAPI 文档声明但未注册路由
    extra: Vec<String>,
    /// 排除清单条目已被文档覆盖（排除已无必要）
    stale: Vec<String>,
    /// 已注册且已文档化的数量
    ok_count: usize,
    /// 被有意排除的数量
    omitted_count: usize,
}

fn compare() -> DiffReport {
    let registered = registered_routes();
    let documented: BTreeSet<(String, String)> = openapi_routes().into_iter().collect();
    let registered_set: BTreeSet<(String, String)> = registered
        .iter()
        .map(|(m, p)| ((*m).to_string(), (*p).to_string()))
        .collect();

    let mut missing = Vec::new();
    let mut extra = Vec::new();
    let mut stale = Vec::new();
    let mut ok_count = 0;
    let mut omitted_count = 0;

    for (m, p) in &registered {
        if is_intentionally_omitted(m, p) {
            omitted_count += 1;
            if documented.contains(&((*m).to_string(), (*p).to_string())) {
                stale.push(format!("{m:6} {p}"));
            }
            continue;
        }
        let key = ((*m).to_string(), (*p).to_string());
        if documented.contains(&key) {
            ok_count += 1;
        } else {
            missing.push(format!("{m:6} {p}"));
        }
    }

    for (m, p) in &documented {
        if !registered_set.contains(&(m.clone(), p.clone())) {
            extra.push(format!("{m:6} {p}"));
        }
    }

    DiffReport {
        missing,
        extra,
        stale,
        ok_count,
        omitted_count,
    }
}

fn build_report(r: &DiffReport) -> String {
    let mut out = String::new();
    out.push_str("================================================================\n");
    out.push_str(" 路由 (backend/src/app.rs) vs OpenAPI 文档 (backend/src/openapi.rs)\n");
    out.push_str("================================================================\n");
    out.push_str(&format!(
        " 一致: {} 项；有意排除: {} 项；缺失: {} 项；多余: {} 项；过时排除: {} 项\n\n",
        r.ok_count,
        r.omitted_count,
        r.missing.len(),
        r.extra.len(),
        r.stale.len()
    ));

    if !r.missing.is_empty() {
        out.push_str(&format!(
            " [缺失] 已注册路由但 OpenAPI 文档中没有（{} 项）:\n",
            r.missing.len()
        ));
        for line in &r.missing {
            out.push_str(&format!("   {line}\n"));
        }
        out.push('\n');
    }

    if !r.extra.is_empty() {
        out.push_str(&format!(
            " [多余] OpenAPI 文档声明但未注册路由（{} 项）:\n",
            r.extra.len()
        ));
        for line in &r.extra {
            out.push_str(&format!("   {line}\n"));
        }
        out.push('\n');
    }

    if !r.stale.is_empty() {
        out.push_str(&format!(
            " [过时] 排除清单条目已被文档覆盖（{} 项）:\n",
            r.stale.len()
        ));
        for line in &r.stale {
            out.push_str(&format!("   {line}\n"));
        }
        out.push('\n');
    }

    if !INTENTIONALLY_OMITTED.is_empty() {
        out.push_str(&format!(
            " [排除] 有意不写进文档的已注册路径（{} 项）:\n",
            INTENTIONALLY_OMITTED.len()
        ));
        for e in INTENTIONALLY_OMITTED {
            out.push_str(&format!(
                "   {:<6} {:<16} — {}\n",
                e.method, e.path_prefix, e.reason
            ));
        }
        out.push('\n');
    }

    if r.missing.is_empty() && r.extra.is_empty() && r.stale.is_empty() {
        out.push_str(" ✓ 文档与路由完全一致。\n");
    } else {
        out.push_str(
            " ✗ 文档与路由存在差异，请同步 backend/src/openapi.rs 或 backend/src/app.rs。\n",
        );
    }
    out
}

/// 基础健全性：spec() 可解析且是合法的 OpenAPI 结构。
#[test]
fn openapi_spec_is_parseable() {
    let spec = openapi::spec();
    assert_eq!(spec["openapi"], "3.1.0");
    assert!(
        spec.get("paths").and_then(|p| p.as_object()).is_some(),
        "spec 必须包含 paths 对象"
    );
}

/// 路由 ↔ OpenAPI 文档双向一致性。
/// 失败时打印完整差异报告（这正是本测试的价值）。
#[test]
fn routes_match_openapi_docs() {
    let r = compare();
    let report = build_report(&r);
    assert!(
        r.missing.is_empty() && r.extra.is_empty() && r.stale.is_empty(),
        "{report}"
    );
}
