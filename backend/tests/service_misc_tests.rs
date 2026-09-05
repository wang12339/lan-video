#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! 搜索 / 推荐 / 播放 / 工具类的纯逻辑单元测试。
//!
//! 背景：50 智能体迭代改进 · 测试补齐环节。只补测试，不改源码。
//!
//! 运行：
//!   cargo check --tests
//!   cargo test --test service_misc_tests
//!
//! 涉及真实 PostgreSQL 的用例统一 `#[ignore]`（见文件尾部）：
//!   DATABASE_URL=... cargo test --test service_misc_tests -- --ignored
//!
//! 设计说明：
//! - 所有"不触库"测试使用指向 127.0.0.1:1（必然拒绝连接）的懒连接池。
//!   若被测短路/节流逻辑回归、开始触碰数据库，测试会立即失败，从而把
//!   "不访问 DB"的行为锁定为契约。
//! - `should_write`（播放节流）的 TTL 过期（ENTRY_TTL=120s）与
//!   MAX_TRACKED_WRITES 惰性清理依赖真实时钟上的 `Instant`，无法在
//!   不注入时钟的前提下测试过期分支，故只测窗口内节流行为。
//! - `retry_backoff`（指数退避）与 `normalize_query`、`strip_ts_headline_markers`
//!   均为模块私有函数，模块内已有 #[cfg(test)] 覆盖；本文件通过公开接口
//!   从外部验证可观测行为，避免重复复制私有逻辑。

use std::net::{Ipv6Addr, SocketAddr};
use std::time::Duration;

use atmos_video_backend::repositories::playback_repo::PlaybackRepository;
use atmos_video_backend::repositories::video_repo::VideoRepository;
use atmos_video_backend::services::playback_service::PlaybackService;
use atmos_video_backend::services::recommendation_service::RecommendationService;
use atmos_video_backend::services::search_service::SearchService;
use atmos_video_backend::services::task_queue::{TaskQueue, TaskStatus, TranscodeTask};
use atmos_video_backend::services::transcoder::Transcoder;
use atmos_video_backend::util::hashid::{decode_id, decode_id_or_numeric, encode_id};
use atmos_video_backend::util::net::client_ip;
use atmos_video_backend::util::response::{internal_error_log, ErrorResponse, SafeJson};
use axum::body::Body;
use axum::extract::{ConnectInfo, FromRequest};
use axum::http::{header, Request, StatusCode};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

/// 指向必然拒绝连接的端口的懒连接池：任何真实 SQL 执行都会立刻失败。
/// 用于把"短路 / 节流 / 幂等"路径锁在数据库访问之前。
/// acquire_timeout 故意设短：sqlx 对无法建立的连接会等到超时才报错，
/// 若不限制会让"死池"用例慢吞吞拖 30s，甚至跨过 10s 节流窗口。
fn dead_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_millis(500))
        .connect_lazy("postgres://127.0.0.1:1/atmos_dead_test")
        .expect("懒连接池构造不应发起连接")
}

// ══════════════════════════════════════════════════════════════════════
// 一、util::hashid — encode/decode 往返、非法输入、负数、回环校验
// ══════════════════════════════════════════════════════════════════════

#[test]
fn hashid_roundtrip_positive_ids() {
    for id in [0i64, 1, 42, 12_345, 1_000_000_000, i64::MAX] {
        let enc = encode_id(id);
        assert!(!enc.is_empty(), "encode 不应返回空串");
        assert_eq!(decode_id(&enc), Some(id), "roundtrip 失败: {id} -> {enc}");
    }
}

#[test]
fn hashid_encode_is_deterministic() {
    // 同一 id 重复编码必须一致（无随机性），否则 URL 不稳定
    assert_eq!(encode_id(42), encode_id(42));
}

#[test]
fn hashid_negative_ids_rejected_on_decode() {
    // encode 前做 `id as u64` 无符号转换：负数包装成超 i64 范围的大数，
    // decode 时 i64::try_from 失败 → None，而不是回绕成错误的正数。
    for id in [-1i64, -42, -9_223_372_036_854_775_000, i64::MIN] {
        let enc = encode_id(id);
        assert_eq!(
            decode_id(&enc),
            None,
            "负数 {id} 编码为 {enc}，不得解码回环"
        );
    }
}

#[test]
fn hashid_rejects_garbage_input() {
    for s in ["", "not-a-hash!", "######", "中文输入", " ", "!@#$%^&*"] {
        assert_eq!(decode_id(s), None, "垃圾输入 {s:?} 必须被拒绝");
    }
}

#[test]
fn hashid_rejects_non_canonical_encodings() {
    // 回环校验：能解码出数字但重编码不一致（附加/篡改字符、短串）→ None，
    // 防止歧义字符串 shadow 掉数字 id。
    let enc = encode_id(42);
    assert_eq!(
        decode_id(&format!("{enc}x")),
        None,
        "尾部附加字符必须被拒绝"
    );
    assert_eq!(
        decode_id(&format!("x{enc}")),
        None,
        "头部附加字符必须被拒绝"
    );
    assert_eq!(
        decode_id(&enc[..enc.len() - 1]),
        None,
        "截断串不得通过回环校验"
    );

    // 数字直写不是规范 hashid（harsh 重编码长度恒 >= 8）
    assert_eq!(decode_id("42"), None);
    assert_eq!(decode_id("1"), None);
    assert_eq!(decode_id("0"), None);
}

#[test]
fn hashid_or_numeric_fallback() {
    // 纯数字 → 数字解析
    assert_eq!(decode_id_or_numeric("42"), Some(42));
    assert_eq!(decode_id_or_numeric("-7"), Some(-7));
    assert_eq!(decode_id_or_numeric("0"), Some(0));
    assert_eq!(decode_id_or_numeric(&i64::MAX.to_string()), Some(i64::MAX));
    // hashid → 解码
    assert_eq!(decode_id_or_numeric(&encode_id(42)), Some(42));
    // 非法 → None
    assert_eq!(decode_id_or_numeric(""), None);
    assert_eq!(decode_id_or_numeric("garbage"), None);
    assert_eq!(decode_id_or_numeric("12abc"), None);
}

// ══════════════════════════════════════════════════════════════════════
// 二、util::response — 错误映射与脱敏
// ══════════════════════════════════════════════════════════════════════

#[test]
fn response_internal_error_log_redacts_details() {
    // 脱敏契约：外部只看到通用 500 文案，绝不回传内部错误原文
    let secret = "connection to db failed, password=SuperSecret leaked";
    let (status, body) = internal_error_log("video_service", &secret);
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body.0.error, "服务器内部错误");
    assert!(
        !body.0.error.contains("SuperSecret") && !body.0.error.contains("password"),
        "错误详情不得泄漏给客户端"
    );
}

#[test]
fn response_error_envelope_serializes_as_expected() {
    let json = serde_json::to_string(&ErrorResponse {
        error: "坏了".to_string(),
    })
    .unwrap();
    assert_eq!(json, r#"{"error":"坏了"}"#);
}

// ── SafeJson：严格 Content-Type 检查 + 反序列化错误脱敏 ──

fn json_req(content_type: &str, body: &str) -> Request<Body> {
    Request::builder()
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn safe_json_rejects_non_application_json_content_types() {
    // 安全属性：`application/ld+json` 等 `+json` 变体必须被拒绝，
    // 避免反代/网关内容协商被迷惑。
    for ct in [
        "text/plain",
        "application/ld+json",
        "application/hal+json",
        "application/json2",
        "application/xml",
        "text/json",
    ] {
        let req = json_req(ct, r#"{"a":1}"#);
        let err = match SafeJson::<serde_json::Value>::from_request(req, &()).await {
            Err(e) => e,
            Ok(_) => panic!("content-type {ct:?} 必须被拒绝"),
        };
        assert_eq!(
            err.0,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "content-type {ct:?} 必须被拒绝"
        );
        assert!(err.1 .0.error.contains("unsupported media type"));
    }
}

#[tokio::test]
async fn safe_json_missing_content_type_rejected() {
    let req = Request::builder().body(Body::from(r#"{"a":1}"#)).unwrap();
    let err = match SafeJson::<serde_json::Value>::from_request(req, &()).await {
        Err(e) => e,
        Ok(_) => panic!("缺少 Content-Type 必须被拒绝"),
    };
    assert_eq!(err.0, StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn safe_json_accepts_application_json_variants() {
    for ct in [
        "application/json",
        "Application/JSON",
        "application/json; charset=utf-8",
    ] {
        let req = json_req(ct, r#"{"a":1,"b":"x"}"#);
        let val = SafeJson::<serde_json::Value>::from_request(req, &())
            .await
            .unwrap_or_else(|e| panic!("{ct:?} 应被接受: {:?}", e.1 .0.error));
        assert_eq!(val.0["a"], 1);
    }
}

#[tokio::test]
async fn safe_json_malformed_body_sanitized_400() {
    let req = json_req("application/json", r#"{"broken": "#);
    let err = match SafeJson::<serde_json::Value>::from_request(req, &()).await {
        Err(e) => e,
        Ok(_) => panic!("畸形 JSON 必须被拒绝"),
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    // 脱敏：统一文案，不泄漏 serde 内部错误（如 EOF / line-column 详情）
    assert_eq!(err.1 .0.error, "invalid request body");
    assert!(!err.1 .0.error.contains("EOF"));
}

// ══════════════════════════════════════════════════════════════════════
// 三、util::net — client_ip（is_cloudflare_peer 已有模块内单测覆盖，此处
//     只补 client_ip 的公开行为；TRUSTED_PROXY 分支依赖全局环境变量，
//     在并行测试中会互相污染，故只覆盖默认关闭的路径）
// ══════════════════════════════════════════════════════════════════════

fn ip_req(peer: Option<SocketAddr>, cf_ip: Option<&str>, xff: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder();
    if let Some(addr) = peer {
        builder = builder.extension(ConnectInfo(addr));
    }
    if let Some(v) = cf_ip {
        builder = builder.header("cf-connecting-ip", v);
    }
    if let Some(v) = xff {
        builder = builder.header("x-forwarded-for", v);
    }
    builder.body(Body::empty()).unwrap()
}

fn v4(a: [u8; 4]) -> SocketAddr {
    SocketAddr::from((a, 12345))
}

#[test]
fn client_ip_rejects_spoofed_headers_from_non_cloudflare_peer() {
    // 安全属性：直连源站的非 Cloudflare 对端携带 cf-connecting-ip / XFF
    // 必须被忽略，否则可伪造限流 IP。
    let req = ip_req(Some(v4([8, 8, 8, 8])), Some("203.0.113.9"), None);
    assert_eq!(client_ip(&req), "8.8.8.8");

    let req = ip_req(Some(v4([203, 0, 113, 7])), Some("1.1.1.1"), Some("6.6.6.6"));
    assert_eq!(
        client_ip(&req),
        "203.0.113.7",
        "TRUSTED_PROXY 关闭时 XFF 也必须被忽略"
    );
}

#[test]
fn client_ip_trusts_cf_header_from_cloudflare_peer() {
    // Cloudflare IPv4 网段内对端 → 允许 cf-connecting-ip
    let req = ip_req(Some(v4([104, 16, 42, 1])), Some("203.0.113.9"), None);
    assert_eq!(client_ip(&req), "203.0.113.9");
    // Cloudflare IPv6 网段
    let v6_peer: SocketAddr =
        SocketAddr::from((Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 1), 443));
    let req = ip_req(Some(v6_peer), Some("203.0.113.10"), None);
    assert_eq!(client_ip(&req), "203.0.113.10");
}

#[test]
fn client_ip_cloudflare_peer_without_or_invalid_header_falls_back() {
    let req = ip_req(Some(v4([104, 16, 42, 1])), None, None);
    assert_eq!(client_ip(&req), "104.16.42.1");
    let req = ip_req(Some(v4([104, 16, 42, 1])), Some("not-an-ip"), None);
    assert_eq!(client_ip(&req), "104.16.42.1");
}

#[test]
fn client_ip_without_peer_returns_unknown() {
    let req = ip_req(None, Some("203.0.113.9"), None);
    assert_eq!(client_ip(&req), "unknown");
}

// ══════════════════════════════════════════════════════════════════════
// 四、services::search_service — 空查询短路（normalize 后为空必须不触库）
// ══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn search_empty_query_short_circuits_before_db() {
    let svc = SearchService::new(VideoRepository::new(dead_pool()));
    // 死池 + 空查询：若短路逻辑回归、开始查库，这里会返回 Err
    for q in ["", "   ", "\t\n  ", "   \u{3000}  "] {
        let (results, total) = svc
            .full_text_search(1, q, 1, 10)
            .await
            .expect("空查询必须短路返回 Ok，不触碰数据库");
        assert!(results.is_empty());
        assert_eq!(total, 0);
    }
}

#[tokio::test]
async fn search_suggest_empty_query_short_circuits_before_db() {
    let svc = SearchService::new(VideoRepository::new(dead_pool()));
    assert!(svc.search_suggest(1, "   ", 5).await.unwrap().is_empty());
    assert!(svc.search_suggest(1, "", 5).await.unwrap().is_empty());
}

#[tokio::test]
async fn search_nonempty_query_reaches_db() {
    // 反证：非空查询必须真的走到数据库（死池 → Err）。
    // 证明上面的短路只发生在 normalize 后为空时，防止"假短路"掩盖查询。
    let svc = SearchService::new(VideoRepository::new(dead_pool()));
    let err = svc.full_text_search(1, "hello", 1, 10).await.unwrap_err();
    assert!(format!("{}", err).contains("搜索失败"), "got: {err}");
    let err = svc.search_suggest(1, "hello", 5).await.unwrap_err();
    assert!(format!("{}", err).contains("搜索建议失败"), "got: {err}");
}

// ══════════════════════════════════════════════════════════════════════
// 五、services::task_queue — 队列纯逻辑（去重 / 状态过滤 / FIFO / 清空）
//     注意：add_task 对 Pending 任务会"尽力"持久化到 DB，用死池时持久化
//     失败仅记日志、不入队副作用，正好验证"内存队列不依赖 DB"的契约。
// ══════════════════════════════════════════════════════════════════════

fn make_task(id: i64, status: TaskStatus) -> TranscodeTask {
    TranscodeTask {
        id,
        video_id: id,
        input_path: "/tmp/atmos-test-nonexistent.mp4".to_string(),
        resolutions: vec!["720p".to_string()],
        status,
        created_at: chrono::Utc::now().naive_utc(),
    }
}

fn test_queue() -> TaskQueue {
    TaskQueue::new(
        Transcoder::new(
            std::path::Path::new("/tmp/atmos-test-variants"),
            Default::default(),
        ),
        dead_pool(),
        std::path::PathBuf::from("/tmp/atmos-test-media"),
    )
}

#[tokio::test]
async fn task_queue_rejects_non_pending_status() {
    let q = test_queue();
    for status in [
        TaskStatus::Processing,
        TaskStatus::Completed,
        TaskStatus::Failed,
    ] {
        q.add_task(make_task(1, status)).await;
    }
    assert_eq!(q.get_queue_size().await, 0, "非 Pending 任务一律拒绝入队");
    assert!(q.process_next().await.is_none());
}

#[tokio::test]
async fn task_queue_dedupes_by_id() {
    let q = test_queue();
    q.add_task(make_task(7, TaskStatus::Pending)).await;
    q.add_task(make_task(7, TaskStatus::Pending)).await;
    assert_eq!(q.get_queue_size().await, 1, "同 id 任务只允许入队一次");
}

#[tokio::test]
async fn task_queue_fifo_order() {
    let q = test_queue();
    for id in [1, 2, 3] {
        q.add_task(make_task(id, TaskStatus::Pending)).await;
    }
    let mut popped = Vec::new();
    while let Some(t) = q.process_next().await {
        popped.push(t.id);
    }
    assert_eq!(popped, vec![1, 2, 3]);
}

#[tokio::test]
async fn task_queue_clear_empties_queue() {
    let q = test_queue();
    q.add_task(make_task(1, TaskStatus::Pending)).await;
    q.add_task(make_task(2, TaskStatus::Pending)).await;
    q.clear_queue().await;
    assert_eq!(q.get_queue_size().await, 0);
    assert!(q.process_next().await.is_none());
}

#[tokio::test]
async fn task_queue_add_works_with_db_down() {
    // 契约：DB 不可用时（死池），入队仍成功、队列可正常弹出。
    // 持久化是"尽力而为"，失败只记日志。
    let q = test_queue();
    q.add_task(make_task(5, TaskStatus::Pending)).await;
    assert_eq!(q.get_queue_size().await, 1);
    let t = q.process_next().await.expect("DB 不可用也必须能取出任务");
    assert_eq!(t.id, 5);
}

// ══════════════════════════════════════════════════════════════════════
// 六、services::playback_service — update_playback 写库节流
//     用死池区分"节流吞掉"(Ok，未触库) 与"真实写库"(Err，触库)。
// ══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn playback_write_throttled_within_window() {
    let svc = PlaybackService::new(PlaybackRepository::new(dead_pool()));
    // 首次写入：不受节流，真实触库 → 死池报错
    let first = svc.update_playback(1, "alice", 1, 500, 1000).await;
    assert!(first.is_err(), "首次写入必须放行并触库（期望 DB 连接错误）");
    // 10s 窗口内重复上报：被节流吞掉，返回 Ok 且不触库
    assert!(svc.update_playback(1, "alice", 1, 600, 1000).await.is_ok());
    assert!(svc.update_playback(1, "alice", 1, 700, 1000).await.is_ok());
    // 不同视频、不同用户：各自独立 key，仍会触库（Err）
    assert!(
        svc.update_playback(1, "alice", 2, 100, 1000).await.is_err(),
        "不同 video_id 必须不受前一条节流影响"
    );
    assert!(
        svc.update_playback(1, "bob", 1, 100, 1000).await.is_err(),
        "不同用户必须不受他人节流影响"
    );
}

#[tokio::test]
async fn playback_throttle_key_is_username_and_video() {
    let svc = PlaybackService::new(PlaybackRepository::new(dead_pool()));
    assert!(svc.update_playback(1, "u1", 10, 1, 1000).await.is_err());
    // 同用户同视频：命中节流
    assert!(svc.update_playback(1, "u1", 10, 2, 1000).await.is_ok());
    // 同用户不同视频：新 key，放行触库
    assert!(svc.update_playback(1, "u1", 11, 1, 1000).await.is_err());
    // 不同用户同视频：新 key，放行触库
    assert!(svc.update_playback(1, "u2", 10, 1, 1000).await.is_err());
}

// ══════════════════════════════════════════════════════════════════════
// 七、需要真实 PostgreSQL 的用例（#[ignore]）
//
// 标注原因：以下用例依赖真实数据与触发器等 DB 行为（SQL 评分、tsvector
// 匹配、transcoding_jobs 持久化），纯逻辑部分已在上文覆盖。CI 与本地无
// DB 时跳过；运行方式：
//   DATABASE_URL=postgres://kuaile@localhost:5432/atmos_video \
//   cargo test --test service_misc_tests -- --ignored --nocapture
// ══════════════════════════════════════════════════════════════════════

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn db_pool() -> Option<PgPool> {
    let url = database_url()?;
    Some(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .expect("无法连接测试数据库"),
    )
}

/// 插入一条测试视频并返回 id（仅用于 #[ignore] 的 DB 用例）。
async fn insert_test_video(pool: &PgPool, title: &str, category: &str, views: i64) -> i64 {
    sqlx::query(
        "INSERT INTO videos (title, description, source_type, stream_url, category, views) \
         VALUES ($1, '', 'local', '/tmp/atmos-test-nonexistent.mp4', $2, $3) RETURNING id",
    )
    .bind(title)
    .bind(category)
    .bind(views)
    .fetch_one(pool)
    .await
    .expect("插入测试视频失败")
    .get("id")
}

/// 统计某视频仍为 pending 的转码任务行数（仅用于 #[ignore] 的 DB 用例）。
async fn count_pending_jobs(pool: &PgPool, video_id: i64) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM transcoding_jobs \
         WHERE video_id = $1 AND status = 'pending'",
    )
    .bind(video_id)
    .fetch_one(pool)
    .await
    .expect("统计转码任务失败")
}

/// 全文字搜索：匹配、headline 脱敏、分页防御（极端 page/size 不报错）。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL（DATABASE_URL）"]
async fn search_full_text_and_pagination_defense_with_real_db() {
    let Some(pool) = db_pool().await else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let svc = SearchService::new(VideoRepository::new(pool.clone()));

    let title = format!("SvcMisc AlphaSearch Video {}", std::process::id());
    let id: i64 = sqlx::query(
        "INSERT INTO videos (title, description, source_type, stream_url, category) \
         VALUES ($1, 'desc', 'local', '/tmp/atmos-test-nonexistent.mp4', 'general') RETURNING id",
    )
    .bind(&title)
    .fetch_one(&pool)
    .await
    .expect("插入测试视频失败（检查 search_vector 触发器是否可用）")
    .get("id");

    // 与 search_service 相同的 'simple' 词典重建向量，屏蔽词典差异
    sqlx::query("UPDATE videos SET search_vector = to_tsvector('simple', title) WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    // 注意：handler 的分页约定是 page 从 0 开始（offset = page * size）
    let (results, total) = svc
        .full_text_search(1, "AlphaSearch", 0, 10)
        .await
        .expect("搜索不应失败");
    assert!(total >= 1, "应至少命中刚插入的视频");
    let hit = results
        .iter()
        .find(|r| r.video_id == id)
        .expect("搜索结果应包含测试视频");
    assert_eq!(hit.title, title);
    assert!(hit.rank > 0.0, "完全匹配的 rank 应为正数: {}", hit.rank);
    if let Some(h) = &hit.headline {
        assert!(
            !h.contains("<mark>") && !h.contains("</mark>"),
            "headline 必须剥离 <mark> 标记（XSS-001），got: {h}"
        );
    }

    // 分页防御：负数 / 巨大 page、size=0 / 巨大 size 均不得触发 PostgreSQL 错误
    let (_, _) = svc
        .full_text_search(1, "AlphaSearch", -1_000_000, -10)
        .await
        .expect("负 page/size 必须被 clamp，不得报错");
    let (_, _) = svc
        .full_text_search(1, "AlphaSearch", 1, 10)
        .await
        .expect("page=1（offset=10）必须被接受，不得报错");
    let (oversized, total_beyond) = svc
        .full_text_search(1, "AlphaSearch", 1_000_000, 100)
        .await
        .expect("超大 page/size 必须被 clamp，不得报错");
    assert!(oversized.is_empty(), "offset 越界应返回空结果");
    assert_eq!(total_beyond, 0);

    sqlx::query("DELETE FROM videos WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
}

/// 搜索建议：去重 + 60s TTL 缓存的稳定性（同查询结果一致）。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL（DATABASE_URL）"]
async fn search_suggest_real_db_and_cache_consistency() {
    let Some(pool) = db_pool().await else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let svc = SearchService::new(VideoRepository::new(pool.clone()));

    let title = format!("SvcMisc SuggestAlpha {}", std::process::id());
    let id: i64 = sqlx::query(
        "INSERT INTO videos (title, description, source_type, stream_url, category) \
         VALUES ($1, '', 'local', '/tmp/atmos-test-nonexistent.mp4', 'general') RETURNING id",
    )
    .bind(&title)
    .fetch_one(&pool)
    .await
    .expect("插入测试视频失败")
    .get("id");
    sqlx::query("UPDATE videos SET search_vector = to_tsvector('simple', title) WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    // 前缀命中（tsvector 匹配分支）
    let s1 = svc
        .search_suggest(1, "suggestalpha", 10)
        .await
        .expect("suggest 失败");
    assert!(
        s1.iter().any(|t| t == &title),
        "建议应包含前缀/向量匹配的标题: {s1:?}"
    );
    // 连续第二次调用（命中缓存）：结果必须与首次一致
    let s2 = svc.search_suggest(1, "suggestalpha", 10).await.unwrap();
    assert_eq!(s1, s2, "缓存命中的结果必须与首次一致");
    // 不同 limit → 不同 cache key，行为正确即可
    let s3 = svc.search_suggest(1, "suggestalpha", 1).await.unwrap();
    assert_eq!(s3.len(), 1);

    sqlx::query("DELETE FROM videos WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
}

/// 推荐评分：验证 Rust 侧 score/reason 与 SQL ORDER BY 的一致性，
/// 以及"已看排除 / exclude / 冷启动回退 trending"。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL（DATABASE_URL）"]
async fn recommendation_scoring_and_fallbacks_with_real_db() {
    let Some(pool) = db_pool().await else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let svc = RecommendationService::new(VideoRepository::new(pool.clone()));

    let suffix = std::process::id().to_string();
    let user = format!("svcmisc_{suffix}");
    let fresh_user = format!("svcmisc_fresh_{suffix}");
    let cat = format!("svcmisc_cat_{suffix}");
    let _ = sqlx::query("DELETE FROM playback_history WHERE username = $1 OR username = $2")
        .bind(&user)
        .bind(&fresh_user)
        .execute(&pool)
        .await;

    // v1: 首选分类(cat) 低播放 → 2.0 * 1.0 = 2.0，"基于你的观看偏好"
    // v2: 首选分类(cat) 高播放(>1000) → 2.0 * 1.5 = 3.0
    // v3: 非首选(general) 中播放(>100) → 1.0 * 1.2 = 1.2，"热门推荐"
    // v4: 用户已看 → 必须被排除
    // v5: exclude_video_id → 必须被排除
    let v1: i64 = insert_test_video(&pool, &format!("svcmisc rec A {suffix}"), &cat, 50).await;
    let v2: i64 = insert_test_video(&pool, &format!("svcmisc rec B {suffix}"), &cat, 5_000).await;
    let v3: i64 =
        insert_test_video(&pool, &format!("svcmisc rec C {suffix}"), "general", 500).await;
    let v4: i64 = insert_test_video(&pool, &format!("svcmisc rec D {suffix}"), &cat, 10).await;
    let v5: i64 =
        insert_test_video(&pool, &format!("svcmisc rec E {suffix}"), "general", 100).await;

    // 观看历史：user 看过 v4（产生首选分类 cat）
    for (u, vid) in [(&user, v4)] {
        sqlx::query(
            "INSERT INTO playback_history (username, video_id, position_ms, duration_ms) \
             VALUES ($1, $2, 0, 1000)",
        )
        .bind(u)
        .bind(vid)
        .execute(&pool)
        .await
        .unwrap();
    }

    // ── user：有首选分类，正常推荐 ──
    let recs = svc
        .get_recommendations(1, &user, v5, 10)
        .await
        .expect("get_recommendations 失败");
    let ids: Vec<i64> = recs.iter().map(|r| r.id).collect();
    assert!(!ids.contains(&v4), "已看视频必须被排除: {ids:?}");
    assert!(!ids.contains(&v5), "exclude_video_id 必须被排除: {ids:?}");
    let score_of = |id: i64| {
        recs.iter()
            .find(|r| r.id == id)
            .map(|r| r.score)
            .expect("缺少推荐")
    };
    assert_eq!(score_of(v1), 2.0, "首选分类低播放: 2.0*1.0");
    assert_eq!(score_of(v2), 3.0, "首选分类高播放: 2.0*1.5");
    assert_eq!(score_of(v3), 1.2, "非首选中播放: 1.0*1.2");
    let reason_of = |id: i64| recs.iter().find(|r| r.id == id).unwrap().reason;
    assert_eq!(reason_of(v1), "基于你的观看偏好");
    assert_eq!(reason_of(v2), "基于你的观看偏好");
    assert_eq!(reason_of(v3), "热门推荐");

    // ── 冷启动回退：无任何观看历史 → 无首选分类 → 直接回退 trending ──
    // 注：`rows.is_empty()` 分支要求用户把全库视频都看过才能触发，
    // 在共享的已填充数据库里不可行，这里验证等价的无历史分支。
    let fallback = svc
        .get_recommendations(1, &fresh_user, v5, 10)
        .await
        .expect("回退 trending 失败");
    assert!(!fallback.is_empty(), "无历史用户必须回退到 trending");
    assert!(
        fallback.iter().all(|r| r.reason == "热门推荐"),
        "回退结果理由应为热门推荐: {:?}",
        fallback.iter().map(|r| &r.reason).collect::<Vec<_>>()
    );

    // ── get_similar_videos / get_trending_videos / get_recent_videos ──
    let similar = svc
        .get_similar_videos(1, v1, 10)
        .await
        .expect("get_similar_videos 失败");
    assert!(
        similar
            .iter()
            .all(|r| r.category.as_deref() == Some(cat.as_str())),
        "相似视频必须限定同分类"
    );
    assert!(similar.iter().all(|r| r.id != v1));
    let trending = svc
        .get_trending_videos(1, 0, 10)
        .await
        .expect("trending 失败");
    assert!(!trending.0.is_empty());
    let recent = svc.get_recent_videos(1, 0, 10).await.expect("recent 失败");
    assert!(!recent.0.is_empty());

    // 清理
    sqlx::query("DELETE FROM videos WHERE id = ANY($1)")
        .bind(vec![v1, v2, v3, v4, v5])
        .execute(&pool)
        .await
        .unwrap();
    let _ = sqlx::query("DELETE FROM playback_history WHERE username = $1 OR username = $2")
        .bind(&user)
        .bind(&fresh_user)
        .execute(&pool)
        .await;
}

/// 任务队列：Pending 任务持久化到 transcoding_jobs，且幂等（UNIQUE 约束）。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL（DATABASE_URL）"]
async fn task_queue_persists_pending_jobs_idempotently_with_real_db() {
    let Some(pool) = db_pool().await else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let q = TaskQueue::new(
        Transcoder::new(
            std::path::Path::new("/tmp/atmos-test-variants"),
            Default::default(),
        ),
        pool.clone(),
        std::path::PathBuf::from("/tmp/atmos-test-media"),
    );

    let video_id: i64 = sqlx::query(
        "INSERT INTO videos (title, description, source_type, stream_url, category) \
         VALUES ('svcmisc taskqueue', '', 'local', '/tmp/atmos-test-nonexistent.mp4', 'general') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .get("id");

    let task = TranscodeTask {
        id: 1,
        video_id,
        input_path: "/tmp/atmos-test-nonexistent.mp4".to_string(),
        resolutions: vec!["720p".to_string(), "1080p".to_string()],
        status: TaskStatus::Pending,
        created_at: chrono::Utc::now().naive_utc(),
    };
    q.add_task(task.clone()).await;
    q.add_task(task).await; // 重复入队：内存去重，持久化幂等

    // 持久化是异步 best-effort，轮询等待
    let mut rows = 0;
    for _ in 0..20 {
        rows = count_pending_jobs(&pool, video_id).await;
        if rows == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        rows, 2,
        "2 个分辨率应持久化为 2 行 pending 任务（重复入队不翻倍）"
    );

    sqlx::query("DELETE FROM transcoding_jobs WHERE video_id = $1")
        .bind(video_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM videos WHERE id = $1")
        .bind(video_id)
        .execute(&pool)
        .await
        .unwrap();
}
