//! Auth Gateway SSO 对接（OAuth2 授权码 + PKCE）
//!
//! 可选功能：`GATEWAY_URL` / `GATEWAY_CLIENT_ID` / `GATEWAY_CLIENT_SECRET`
//! / `GATEWAY_REDIRECT_URI` 四个环境变量全部配置时启用。
//!
//! 流程：
//! 1. `GET /auth/gateway/start`   → 生成 state+PKCE 暂存 → 302 到网关授权页
//! 2. 用户在网关登录并同意授权 → 302 回 `redirect_uri` 并携带 `code`
//! 3. `GET /auth/gateway/callback` → code 换 token → 拉 userinfo →
//!    按网关不可变 `sub` 关联/建号（邮箱仅在已验证时按邮箱绑定）→
//!    签发 Atmos token → 302 回前端落地页
//!    （带一次性 exchange_code，前端用它换真 token）

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::models::auth::AuthResponse;
use crate::state::AppState;
use crate::util::response::error_response;

/// 单条待授权请求的暂存记录
#[derive(Clone)]
struct PendingAuth {
    verifier: String,
    created_at: Instant,
}

/// 内存暂存区（state → PKCE verifier）。重启即失效，无需持久化：
/// 用户重新点一次登录按钮即可。
static PENDING: std::sync::OnceLock<Arc<Mutex<HashMap<String, PendingAuth>>>> =
    std::sync::OnceLock::new();

fn pending() -> &'static Arc<Mutex<HashMap<String, PendingAuth>>> {
    PENDING.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

const PENDING_TTL_SECS: u64 = 600;

/// 一次性 exchange code 暂存（exchange_code → Atmos token）。
/// 30 秒有效、用后即焚，用于把 token 安全交给前端。
type ExchangeMap = HashMap<String, (String, Instant)>;

static EXCHANGES: std::sync::OnceLock<Arc<Mutex<ExchangeMap>>> = std::sync::OnceLock::new();

fn exchanges() -> &'static Arc<Mutex<ExchangeMap>> {
    EXCHANGES.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

const EXCHANGE_TTL_SECS: u64 = 30;

/// 网关是否已启用（四个配置全部非空）
pub fn enabled(state: &AppState) -> bool {
    !state.config.gateway_url.is_empty()
        && !state.config.gateway_client_id.is_empty()
        && !state.config.gateway_client_secret.is_empty()
        && !state.config.gateway_redirect_uri.is_empty()
}

fn random_token(len: usize) -> String {
    use rand::distributions::Alphanumeric;
    rand::rngs::OsRng
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn pkce_challenge(verifier: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

async fn gc_pending() {
    let mut map = pending().lock().await;
    map.retain(|_, v| v.created_at.elapsed() < Duration::from_secs(PENDING_TTL_SECS));
}

async fn gc_exchanges() {
    let mut map = exchanges().lock().await;
    map.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(EXCHANGE_TTL_SECS));
}

// ---------- 网关响应结构 ----------

#[derive(Deserialize)]
struct GatewayTokenResp {
    access_token: String,
    #[allow(dead_code)]
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct GatewayUserinfo {
    /// 身份提供方的不可变主体标识（OIDC `sub`）。账号关联的唯一权威键：
    /// 缺失（反序列化失败）或为空都拒绝登录。
    sub: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    email: Option<String>,
    /// 网关是否声明邮箱已验证；只有 `Some(true)` 才允许按邮箱绑定已有账号。
    #[serde(default)]
    email_verified: Option<bool>,
}

// ---------- handlers ----------

/// GET /auth/gateway/status — 前端判断是否显示网关登录按钮
pub async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json(serde_json::json!({ "enabled": enabled(&state) }))
}

/// GET /auth/gateway/start — 302 到网关授权页
pub async fn start(State(state): State<Arc<AppState>>) -> axum::response::Response {
    if !enabled(&state) {
        return error_response(StatusCode::NOT_FOUND, "Not Found").into_response();
    }
    let state_param = random_token(32);
    let verifier = random_token(64);
    let challenge = pkce_challenge(&verifier);

    // 截断超长 verifier：RFC 7636 限制 43..128 字符
    let verifier = verifier[..96.min(verifier.len())].to_string();

    gc_pending().await;
    pending().lock().await.insert(
        state_param.clone(),
        PendingAuth {
            verifier,
            created_at: Instant::now(),
        },
    );

    let authorize_url = format!(
        "{}/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&scope=profile+email&state={}&code_challenge={}&code_challenge_method=S256",
        state.config.gateway_url,
        urlencode(&state.config.gateway_client_id),
        urlencode(&state.config.gateway_redirect_uri),
        urlencode(&state_param),
        urlencode(&challenge),
    );
    axum::response::Redirect::to(&authorize_url).into_response()
}

/// GET /auth/gateway/callback?code=..&state=..（或 error=..）
pub async fn callback(
    State(state): State<Arc<AppState>>,
    axum::extract::RawQuery(raw): axum::extract::RawQuery,
) -> axum::response::Response {
    let webapp_login = "/webapp/";
    if !enabled(&state) {
        return redirect_err(webapp_login, "gateway_disabled");
    }
    let Some(query) = raw else {
        return redirect_err(webapp_login, "missing_params");
    };
    let params: HashMap<String, String> = query
        .split('&')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some((urldecode(k), urldecode(v)))
        })
        .collect();

    // 用户在网关点了"拒绝"或出错
    if let Some(err) = params.get("error") {
        tracing::warn!(error = %err, "gateway oauth denied/errored");
        return redirect_err(webapp_login, "gateway_denied");
    }
    let (Some(code), Some(state_param)) = (params.get("code"), params.get("state")) else {
        return redirect_err(webapp_login, "missing_params");
    };

    // 取出并删除 pending（一次性）
    let pending_entry = {
        let mut map = pending().lock().await;
        map.remove(state_param)
    };
    let Some(p) = pending_entry else {
        return redirect_err(webapp_login, "state_invalid_or_expired");
    };
    if p.created_at.elapsed() >= Duration::from_secs(PENDING_TTL_SECS) {
        return redirect_err(webapp_login, "state_invalid_or_expired");
    }

    // code 换 token
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(_) => return redirect_err(webapp_login, "internal_error"),
    };
    let internal_base = state.config.gateway_internal_url.trim().to_string();
    let token_resp = client
        .post(format!("{}/oauth/token", internal_base))
        .form(&[
            ("grant_type", "authorization_code".to_string()),
            ("client_id", state.config.gateway_client_id.clone()),
            ("client_secret", state.config.gateway_client_secret.clone()),
            ("code", code.clone()),
            ("redirect_uri", state.config.gateway_redirect_uri.clone()),
            ("code_verifier", p.verifier.clone()),
        ])
        .send()
        .await;
    let Ok(resp) = token_resp else {
        tracing::error!("gateway token request failed");
        return redirect_err(webapp_login, "gateway_unreachable");
    };
    if resp.status() != reqwest::StatusCode::OK {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        tracing::error!(status = %status, body = %body, "gateway token exchange failed");
        return redirect_err(webapp_login, "token_exchange_failed");
    }
    let Ok(tok) = resp.json::<GatewayTokenResp>().await else {
        return redirect_err(webapp_login, "token_exchange_failed");
    };

    // 拉用户信息
    let ui_resp = client
        .get(format!("{}/oauth/userinfo", internal_base))
        .bearer_auth(&tok.access_token)
        .send()
        .await;
    let Ok(resp) = ui_resp else {
        return redirect_err(webapp_login, "gateway_unreachable");
    };
    if resp.status() != reqwest::StatusCode::OK {
        tracing::error!(status = %resp.status(), "gateway userinfo failed");
        return redirect_err(webapp_login, "userinfo_failed");
    }
    let Ok(ui) = resp.json::<GatewayUserinfo>().await else {
        return redirect_err(webapp_login, "userinfo_failed");
    };
    // sub 是必填的不可变主体标识：缺失/为空一律拒绝，绝不用用户名兜底。
    let gw_sub = ui.sub.trim().to_string();
    if gw_sub.is_empty() {
        tracing::warn!("gateway userinfo missing sub");
        return redirect_err(webapp_login, "userinfo_failed");
    }

    // 账号关联/建号 + 签发 Atmos token
    let gw_username = ui.username.trim().to_string();
    let display_email = ui.email.clone().filter(|e| e.contains('@'));
    let email_verified = ui.email_verified == Some(true);

    let result = gateway_sign_in(
        &state,
        &gw_sub,
        &gw_username,
        display_email.as_deref(),
        email_verified,
    )
    .await;
    match result {
        Ok(atmos_token) => {
            // 生成一次性 exchange code，30 秒有效
            let exchange_code = random_token(48);
            gc_exchanges().await;
            exchanges()
                .lock()
                .await
                .insert(exchange_code.clone(), (atmos_token, Instant::now()));
            // 302 回前端落地页
            axum::response::Redirect::to(&format!("{webapp_login}?gw_code={exchange_code}"))
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "gateway sign-in failed");
            redirect_err(webapp_login, "sign_in_failed")
        }
    }
}

/// POST /auth/gateway/exchange { code } — 前端用一次性 code 换真 token
pub async fn exchange(
    State(_state): State<Arc<AppState>>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> axum::response::Response {
    let code = body
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if code.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(AuthResponse {
                ok: false,
                token: None,
                error: Some("缺少 code".into()),
            }),
        )
            .into_response();
    }
    let entry = {
        let mut map = exchanges().lock().await;
        map.remove(&code)
    };
    let Some((token, t)) = entry else {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(AuthResponse {
                ok: false,
                token: None,
                error: Some("code 无效或已过期".into()),
            }),
        )
            .into_response();
    };
    if t.elapsed() >= Duration::from_secs(EXCHANGE_TTL_SECS) {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(AuthResponse {
                ok: false,
                token: None,
                error: Some("code 已过期".into()),
            }),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        axum::Json(AuthResponse {
            ok: true,
            token: Some(token),
            error: None,
        }),
    )
        .into_response()
}

/// 判断 sqlx 错误是否为 PostgreSQL 唯一约束冲突（SQLSTATE 23505）。
fn is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(|db| db.code())
        .map(|code| code.as_ref() == "23505")
        .unwrap_or(false)
}

/// 校验 approved、执行单会话踢旧并签发 token（所有关联路径共用）。
async fn issue_token_for_existing(
    state: &AppState,
    user: &crate::repositories::user_repo::UserRow,
) -> Result<String, String> {
    if !user.approved {
        return Err("user not approved".into());
    }
    // 单会话策略与密码登录一致：普通用户新登录踢旧会话（管理员允许多端）
    if user.role < 3 {
        if let Err(e) = state.repos.user.revoke_tokens_by_user_id(user.id).await {
            tracing::warn!("gateway single-session revoke failed: {}", e);
        }
    }
    state
        .repos
        .user
        .create_token(user.id)
        .await
        .map_err(|e| e.to_string())
}

/// 按网关不可变 `sub` 登录/建号，返回 Atmos token。
///
/// 关联顺序（防账号接管）：
/// a. `sub` 已绑定 → 直接命中该本地账号；
/// b. 否则仅当网关声明 `email_verified = true` 时，按邮箱命中已有账号并
///    绑定 `sub`；
/// c. 否则新建账号：用户名被占用时追加 `_2`/`_3`… 直到可用，`sub` 绑定
///    成功后才放行，`email` 仅在未被占用时写入。
///
/// 明确不再「按用户名自动登入本地同名账号」，也不把邮箱写到任意已有账号上。
async fn gateway_sign_in(
    state: &AppState,
    gw_sub: &str,
    gw_username: &str,
    email: Option<&str>,
    email_verified: bool,
) -> Result<String, String> {
    // a. 不可变 sub 命中：sub 是唯一权威键，不再按用户名查。
    if let Some(user) = state
        .repos
        .user
        .find_by_gateway_sub(gw_sub)
        .await
        .map_err(|e| e.to_string())?
    {
        return issue_token_for_existing(state, &user).await;
    }

    // b. 网关声明邮箱已验证时，才允许按邮箱绑定已有账号。
    if email_verified {
        if let Some(mail) = email.filter(|m| !m.is_empty()) {
            if let Some(user) = state
                .repos
                .user
                .find_by_email(mail)
                .await
                .map_err(|e| e.to_string())?
            {
                return link_gateway_sub_and_sign_in(state, &user, gw_sub).await;
            }
        }
    }

    // c. 新用户：网关用户 = 已验证身份，免审批直接通过。
    // 密码字段存随机 Argon2 哈希 —— 用户无法用密码登录此账号，
    // 只能通过网关 SSO 进入（密码找回等流程自然失效）。
    let random_password = random_token(32);
    let password_hash = tokio::task::spawn_blocking(move || {
        use argon2::password_hash::{PasswordHasher, SaltString};
        use argon2::Argon2;
        let salt = SaltString::generate(&mut rand::rngs::OsRng);
        Argon2::default()
            .hash_password(random_password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .unwrap_or_default()
    })
    .await
    .map_err(|e| e.to_string())?;
    if password_hash.is_empty() {
        return Err("argon2 hash failed".into());
    }

    let base = sanitize_username(gw_username);
    let new_id = create_user_with_unique_username(state, &base, &password_hash).await?;

    // 先绑定 sub：绑定失败（并发下被其他请求抢先绑定）时，刚建的空账号
    // 没有任何会话/token，直接清理，避免留下无法登录的孤儿行。
    match state.repos.user.link_gateway_sub(new_id, gw_sub).await {
        Ok(true) => {}
        Ok(false) => {
            let _ = state.repos.user.delete_user(new_id).await;
            return sign_in_sub_owner(state, gw_sub).await;
        }
        Err(e) if is_unique_violation(&e) => {
            let _ = state.repos.user.delete_user(new_id).await;
            return sign_in_sub_owner(state, gw_sub).await;
        }
        Err(e) => {
            let _ = state.repos.user.delete_user(new_id).await;
            return Err(e.to_string());
        }
    }

    // create_user 里 role>=3 才 approved；这里手动放行网关用户
    if let Err(e) = state.repos.user.approve_user(new_id, true).await {
        tracing::warn!("gateway approve new user failed: {}", e);
    }

    // email 仅在未被其他账号占用时写入（网关已验证的邮箱此时通常可用，
    // 但并发/历史数据下仍可能撞车，撞车则放弃写入而不是覆盖他人邮箱）。
    if let Some(mail) = email.filter(|m| !m.is_empty()) {
        match state.repos.user.find_by_email(mail).await {
            Ok(None) => {
                if let Err(e) = state.repos.user.update_email(new_id, mail).await {
                    tracing::warn!("gateway set email for new user failed: {}", e);
                }
            }
            Ok(Some(_)) => tracing::warn!("gateway email already in use, not setting for new user"),
            Err(e) => tracing::warn!("gateway email lookup failed: {}", e),
        }
    }

    state
        .repos
        .user
        .create_token(new_id)
        .await
        .map_err(|e| e.to_string())
}

/// 把 `sub` 绑定到已有账号（路径 b）；冲突（23505 或已被绑到别的 sub）
/// 时按 sub 重新查询，命中则登录 sub 的归属账号，否则报错。
async fn link_gateway_sub_and_sign_in(
    state: &AppState,
    user: &crate::repositories::user_repo::UserRow,
    gw_sub: &str,
) -> Result<String, String> {
    match state.repos.user.link_gateway_sub(user.id, gw_sub).await {
        Ok(true) => issue_token_for_existing(state, user).await,
        Ok(false) => sign_in_sub_owner(state, gw_sub).await,
        Err(e) if is_unique_violation(&e) => sign_in_sub_owner(state, gw_sub).await,
        Err(e) => Err(e.to_string()),
    }
}

/// 按 sub 查找归属账号并签发 token（并发绑定冲突后的收敛路径）。
async fn sign_in_sub_owner(state: &AppState, gw_sub: &str) -> Result<String, String> {
    match state.repos.user.find_by_gateway_sub(gw_sub).await {
        Ok(Some(user)) => issue_token_for_existing(state, &user).await,
        Ok(None) => Err("gateway sub conflict".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// 创建用户；用户名被占用时追加 `_2`/`_3`… 直到可用（预查 + 唯一索引
/// 23505 竞态兜底）。避免网关用户名与本地既有账号同名时登入他人账号。
async fn create_user_with_unique_username(
    state: &AppState,
    base: &str,
    password_hash: &str,
) -> Result<i64, String> {
    const MAX_SUFFIX: u32 = 1000;
    let mut suffix: u32 = 1;
    loop {
        let candidate = if suffix == 1 {
            base.to_string()
        } else {
            format!("{}_{}", base, suffix)
        };
        match state.repos.user.find_by_username(&candidate).await {
            Ok(Some(_)) => {
                suffix += 1;
                if suffix > MAX_SUFFIX {
                    return Err("username exhausted".into());
                }
                continue;
            }
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
        match state
            .repos
            .user
            .create_user(&candidate, password_hash, 1)
            .await
        {
            Ok(id) => return Ok(id),
            Err(e) if is_unique_violation(&e) => {
                suffix += 1;
                if suffix > MAX_SUFFIX {
                    return Err("username exhausted".into());
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// 用户名清洗：只保留字母数字/_-.，长度限制
fn sanitize_username(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
        .collect::<String>()
        .to_lowercase();
    if cleaned.len() >= 2 {
        cleaned.chars().take(64).collect()
    } else {
        format!("gw_{}", random_token(10).to_lowercase())
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn redirect_err(webapp_login: &str, code: &str) -> axum::response::Response {
    axum::response::Redirect::to(&format!("{webapp_login}?gw_error={}", urlencode(code)))
        .into_response()
}
