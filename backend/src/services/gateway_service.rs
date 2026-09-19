//! Auth Gateway SSO 对接（OAuth2 授权码 + PKCE）
//!
//! 可选功能：`GATEWAY_URL` / `GATEWAY_CLIENT_ID` / `GATEWAY_CLIENT_SECRET`
//! / `GATEWAY_REDIRECT_URI` 四个环境变量全部配置时启用。
//!
//! 流程：
//! 1. `GET /auth/gateway/start`   → 生成 state+PKCE 暂存 → 302 到网关授权页
//! 2. 用户在网关登录并同意授权 → 302 回 `redirect_uri` 并携带 `code`
//! 3. `GET /auth/gateway/callback` → code 换 token → 拉 userinfo →
//!    按网关用户名自动建号/绑定 → 签发 Atmos token → 302 回前端落地页
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
    #[serde(default)]
    username: String,
    #[serde(default)]
    email: Option<String>,
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
    if ui.username.trim().is_empty() {
        return redirect_err(webapp_login, "userinfo_failed");
    }

    // 账号关联/建号 + 签发 Atmos token
    let gw_username = ui.username.trim().to_string();
    let display_email = ui.email.clone().filter(|e| e.contains('@'));

    let result = gateway_sign_in(&state, &gw_username, display_email.as_deref()).await;
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

/// 按网关用户名登录/建号，返回 Atmos token
async fn gateway_sign_in(
    state: &AppState,
    gw_username: &str,
    email: Option<&str>,
) -> Result<String, String> {
    // 先找现有账号（用户名大小写不敏感，与 login 行为一致）
    if let Some(user) = state
        .repos
        .user
        .find_by_username(gw_username)
        .await
        .map_err(|e| e.to_string())?
    {
        if !user.approved {
            return Err("user not approved".into());
        }
        // 绑定邮箱（若用户还没有）
        if let Some(mail) = email {
            if user.email.is_none() && !mail.is_empty() {
                let _ = state.repos.user.update_email(user.id, mail).await;
            }
        }
        // 单会话策略与密码登录一致：普通用户新登录踢旧会话
        if user.role < 3 {
            let _ = state.repos.user.revoke_tokens_by_user_id(user.id).await;
        }
        return state
            .repos
            .user
            .create_token(user.id)
            .await
            .map_err(|e| e.to_string());
    }

    // 新用户：自动建号（网关用户 = 已验证身份，免审批直接通过）
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

    let username = sanitize_username(gw_username);
    let new_id = state
        .repos
        .user
        .create_user(&username, &password_hash, 1)
        .await
        .map_err(|e| e.to_string())?;
    // create_user 里 role>=3 才 approved；这里手动放行网关用户
    let _ = state.repos.user.approve_user(new_id, true).await;
    if let Some(mail) = email {
        if !mail.is_empty() {
            let _ = state.repos.user.update_email(new_id, mail).await;
        }
    }
    state
        .repos
        .user
        .create_token(new_id)
        .await
        .map_err(|e| e.to_string())
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
