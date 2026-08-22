use crate::config::AppConfig;
use crate::middleware::rate_limit::RateLimiter;
use crate::models::auth::{AuthRequest, AuthResponse, UserInfoResponse, UserProfileResponse};
use crate::repositories::user_repo::UserRepository;
use crate::services::email_service::EmailService;
use crate::services::playback_service::PlaybackService;
use crate::util::db_error;
use crate::util::error::ServiceError;
use crate::util::password;
use crate::util::password::{MAX_PASSWORD_LEN, MIN_PASSWORD_LEN};

/// Token cookie lifetime (7 days in seconds)
pub const COOKIE_MAX_AGE: i64 = 604800;

/// IP rate limit: 30 attempts per minute per IP.
/// Combined with per-username limit for defense in depth.
const IP_RATE_LIMIT_MAX_ATTEMPTS: u32 = 30;
const IP_RATE_LIMIT_WINDOW_SECS: u64 = 60;
const IP_RATE_LIMIT_BLOCK_SECS: u64 = 0;

#[derive(Clone)]
pub struct AuthService {
    user_repo: UserRepository,
    playback_service: PlaybackService,
    rate_limiter: RateLimiter,
    ip_rate_limiter: RateLimiter,
    config: AppConfig,
}

impl AuthService {
    /// 创建新的 `AuthService` 实例
    ///
    /// # 参数
    /// - `user_repo`: 用户数据访问层
    /// - `playback_service`: 播放历史服务
    /// - `rate_limiter`: 用户名级速率限制器
    /// - `ip_rate_limiter`: IP 级速率限制器
    /// - `config`: 应用配置
    pub fn new(
        user_repo: UserRepository,
        playback_service: PlaybackService,
        rate_limiter: RateLimiter,
        ip_rate_limiter: RateLimiter,
        config: AppConfig,
    ) -> Self {
        Self {
            user_repo,
            playback_service,
            rate_limiter,
            ip_rate_limiter,
            config,
        }
    }

    /// 用户注册
    ///
    /// # 参数
    /// - `req`: 认证请求，包含用户名和密码
    /// - `client_ip`: 客户端 IP 地址，用于速率限制和日志记录
    /// - `tenant_id`: 租户 ID，用于多租户隔离
    ///
    /// # 返回
    /// - `Ok(AuthResponse)`: 注册成功
    ///   - `token`: 管理员用户自动获得 token，普通用户需等待审批
    ///   - `error`: 注册失败时的错误信息
    ///
    /// # 错误
    /// - 注册功能关闭时返回友好提示
    /// - 用户名或密码为空时返回提示
    /// - 用户名长度不在 2-64 范围内返回提示
    /// - 用户名包含控制字符时返回提示
    /// - 密码长度不在 6-128 范围内返回提示
    /// - 密码强度不足时返回提示
    /// - 用户名已存在时返回提示（同时执行 dummy hash 防止时序攻击）
    /// - 超出速率限制时返回 `ServiceError::RateLimited`
    ///
    /// # 安全
    /// - 密码使用 Argon2id 哈希，通过 `spawn_blocking` 在阻塞线程池执行
    /// - 用户名大小写不敏感（存储前转小写）
    /// - 用户名包含控制字符（换行符等）会被拒绝，防止日志注入
    /// - 即使用户不存在也会执行 dummy argon2 验证，防止用户名枚举时序攻击
    /// - 并发注册同一用户名时处理唯一约束冲突，返回友好提示
    /// - 注册成功后重置该用户名的速率限制计数器
    pub async fn register(
        &self,
        req: &AuthRequest,
        client_ip: &str,
        tenant_id: i64,
    ) -> Result<AuthResponse, ServiceError> {
        if !self.config.registration_enabled() {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: registration disabled");
            return Ok(auth_err("注册功能已关闭"));
        }

        self.check_rate_limits(&req.username, client_ip, "register")
            .await?;

        let username = req.username.trim();
        // Do NOT trim the password: it is hashed exactly as the user typed it,
        // and login verifies the raw input — trimming here would make
        // passwords with leading/trailing whitespace impossible to log in
        // with.
        let password = req.password.as_str();

        if username.is_empty() || password.is_empty() {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: empty username or password");
            return Ok(auth_err("用户名和密码不能为空"));
        }

        if username.len() < 2 || username.len() > 64 {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: invalid username length");
            return Ok(auth_err("用户名长度需在 2-64 个字符之间"));
        }

        // SECURITY: reject control characters (newlines, etc.) so user-supplied
        // usernames cannot forge log lines or break out of HTML in the admin
        // UI / notification emails.
        if username.chars().any(char::is_control) {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: username contains control characters");
            return Ok(auth_err("用户名包含非法字符"));
        }

        if password.chars().count() < MIN_PASSWORD_LEN
            || password.chars().count() > MAX_PASSWORD_LEN
        {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: invalid password length");
            return Ok(auth_err(format!(
                "密码长度需在 {}-{} 个字符之间",
                MIN_PASSWORD_LEN, MAX_PASSWORD_LEN
            )));
        }

        if !is_password_strong_enough(password) {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: weak password (common or trivial)");
            return Ok(auth_err("密码过于简单，请使用更复杂的密码".to_string()));
        }

        // SECURITY (A07-09 / H-09): foot-gun guard. The first-ever registered
        // user previously became admin automatically if REGISTRATION_ENABLED
        // was true on an empty database. We now require an explicit env var
        // (ALLOW_FIRST_USER_ADMIN=true) to opt in to that behaviour. Without
        // it, the first user is a regular viewer that needs admin approval.
        let count = self.user_repo.count_users(tenant_id).await?;
        let is_first_user = count == 0;
        let first_user_admin = std::env::var("ALLOW_FIRST_USER_ADMIN")
            .ok()
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        let role: i16 = if is_first_user && first_user_admin {
            3
        } else {
            1
        };

        let user_exists = self
            .user_repo
            .find_by_username(tenant_id, username)
            .await
            .map(|u| u.is_some())
            .unwrap_or(false);
        if user_exists {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: username already exists");
            // Always hash the password to prevent timing side-channel (username enumeration)
            let _ = hash_in_blocking(password).await;
            return Ok(auth_err("用户名已存在"));
        }

        let hash = hash_in_blocking(password).await?;

        // SECURITY: a concurrent registration with the same username hits a
        // unique constraint race. Map it to the same friendly error instead
        // of leaking a 500.
        let user_id = match self
            .user_repo
            .create_user(tenant_id, username, &hash, role)
            .await
        {
            Ok(id) => id,
            Err(ref e) if db_error::is_unique_violation(e) => {
                tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: username taken (unique violation race)");
                return Ok(auth_err("用户名已存在"));
            }
            Err(e) => return Err(e.into()),
        };

        let reset_key = format!("auth:{}", req.username.trim().to_lowercase());
        self.rate_limiter.reset(&reset_key).await;

        tracing::info!(
            username = %sanitize_for_log(username),
            role = %role,
            "user registered"
        );

        if role >= 3 {
            // First user (admin) — auto-approved, return token
            let token = self.user_repo.create_token(user_id).await?;
            Ok(AuthResponse {
                ok: true,
                token: Some(token),
                error: None,
            })
        } else {
            // Regular user — needs admin approval
            Ok(AuthResponse {
                ok: true,
                token: None,
                error: Some("注册成功，请等待管理员审批后再登录".into()),
            })
        }
    }

    /// 用户登录
    ///
    /// # 参数
    /// - `req`: 认证请求，包含用户名和密码
    /// - `client_ip`: 客户端 IP 地址，用于速率限制和日志记录
    /// - `tenant_id`: 租户 ID，用于多租户隔离
    ///
    /// # 返回
    /// - `Ok(AuthResponse)`: 登录成功，返回 token
    ///   - `token`: 256 位 Alphanumeric 字符串，7 天有效期
    ///   - `error`: 登录失败时的错误信息
    ///
    /// # 错误
    /// - 用户名或密码错误时返回通用错误提示（不区分用户不存在和密码错误）
    /// - 用户未通过管理员审批时返回错误提示
    /// - 用户已在其他设备登录时返回错误提示（管理员豁免）
    /// - 超出速率限制时返回 `ServiceError::RateLimited`
    ///
    /// # 安全
    /// - 密码使用 Argon2id 验证，通过 `spawn_blocking` 在阻塞线程池执行
    /// - 用户名大小写不敏感
    /// - 用户不存在时执行 dummy argon2 验证，防止用户名枚举时序攻击（~50-100ms 差异）
    /// - 错误提示统一为"用户名或密码错误"，不泄露用户是否存在
    /// - 登录成功后重置该用户名的速率限制计数器
    /// - 普通用户同一时间只能在一个设备登录（管理员可多设备）
    pub async fn login(
        &self,
        req: &AuthRequest,
        client_ip: &str,
        tenant_id: i64,
    ) -> Result<AuthResponse, ServiceError> {
        self.check_rate_limits(&req.username, client_ip, "login")
            .await?;

        // SECURITY (A07-01 / AF-001): close the username-enumeration timing
        // oracle. We previously returned immediately for unknown users
        // *before* running argon2 verify, allowing an attacker to enumerate
        // valid usernames by measuring response time (~50-100 ms gap on LAN).
        // We now always run a dummy argon2 verify when the user is missing,
        // equalising the timing of the "user not found" and "wrong password"
        // branches.
        let user_opt = self
            .user_repo
            .find_by_username(tenant_id, req.username.trim())
            .await?;
        let user_hash: &str = match user_opt.as_ref() {
            Some(u) => u.password_hash.as_str(),
            None => DUMMY_ARGON2_HASH,
        };
        let password_ok = verify_in_blocking(&req.password, user_hash).await;

        let user = match user_opt {
            Some(u) if password_ok => u,
            _ => {
                // Generic error: do not reveal whether the user exists
                tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "failed login");
                return Ok(auth_err("用户名或密码错误"));
            }
        };

        if !user.approved {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "failed login: not approved");
            return Ok(auth_err("用户名或密码错误"));
        }

        // Reject login if user already has an active session (admins exempt)
        if user.role < 3 && self.user_repo.has_active_tokens(user.id).await? {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "failed login: already logged in elsewhere");
            return Ok(auth_err("该用户已在其他设备登录，请先退出后再试"));
        }

        let token = self.user_repo.create_token(user.id).await?;

        let reset_key = format!("auth:{}", req.username.trim().to_lowercase());
        self.rate_limiter.reset(&reset_key).await;

        tracing::info!(username = %sanitize_for_log(&req.username), "user logged in");

        Ok(AuthResponse {
            ok: true,
            token: Some(token),
            error: None,
        })
    }

    /// 用户登出
    ///
    /// # 参数
    /// - `username`: 可选的用户名，用于日志记录。如果未提供，会尝试从 token 解析
    /// - `token`: 可选的认证 token，用于删除会话
    ///
    /// # 返回
    /// - 无返回值（`()`），登出操作始终视为成功
    ///
    /// # 行为
    /// - 如果提供了 token，会从数据库中删除该 token
    /// - 如果未提供 username 但提供了 token，会尝试通过 token 查询用户名
    /// - 记录登出事件到日志（用户名使用 `<unknown>` 如果无法解析）
    pub async fn logout(&self, username: Option<&str>, token: Option<&str>) {
        let resolved_username = match username {
            Some(u) => Some(u.to_string()),
            None => {
                // Resolve username from the token so the handler doesn't need
                // to do the lookup itself.
                if let Some(t) = token {
                    self.user_repo
                        .find_user_by_token(t)
                        .await
                        .ok()
                        .flatten()
                        .map(|u| u.username)
                } else {
                    None
                }
            }
        };

        if let Some(t) = token {
            let _ = self.user_repo.delete_token(t).await;
        }
        tracing::info!(
            username = %sanitize_for_log(resolved_username.as_deref().unwrap_or("<unknown>")),
            "user logged out"
        );
    }

    /// 重置密码
    ///
    /// # 参数
    /// - `token`: 密码重置 token，由 `send_password_reset_email` 生成
    /// - `password`: 新密码
    ///
    /// # 返回
    /// - `Ok(ResetPasswordResult::Ok)`: 密码重置成功
    /// - `Ok(ResetPasswordResult::InvalidToken)`: token 无效或已过期
    /// - `Ok(ResetPasswordResult::Error(String))`: 密码验证失败（如长度不足或强度不够）
    /// - `Err(ServiceError)`: 服务器内部错误
    ///
    /// # 错误
    /// - 密码长度不在 6-128 范围内返回 `ResetPasswordResult::Error`
    /// - 密码强度不足返回 `ResetPasswordResult::Error`
    /// - token 无效或已过期返回 `ResetPasswordResult::InvalidToken`
    /// - 用户在 token 验证和密码更新之间被删除返回 `ResetPasswordResult::InvalidToken`
    ///
    /// # 安全
    /// - 新密码使用 Argon2id 哈希，通过 `spawn_blocking` 在阻塞线程池执行
    /// - 密码重置后自动撤销该用户的所有现有会话（token）
    /// - 密码验证逻辑与注册一致（不 trim 密码）
    pub async fn reset_password(
        &self,
        token: &str,
        password: &str,
    ) -> Result<ResetPasswordResult, ServiceError> {
        // Validate the raw password, not a trimmed copy — register no longer
        // trims either, so both paths agree on what is hashed.
        let pw_len = password.chars().count();
        if !(MIN_PASSWORD_LEN..=MAX_PASSWORD_LEN).contains(&pw_len) {
            return Ok(ResetPasswordResult::Error(format!(
                "密码长度需在 {}-{} 个字符之间",
                MIN_PASSWORD_LEN, MAX_PASSWORD_LEN
            )));
        }

        if !is_password_strong_enough(password) {
            return Ok(ResetPasswordResult::Error(
                "密码过于简单，请使用包含大小写字母、数字、特殊字符中至少三种的密码".into(),
            ));
        }

        let user_id = match self.user_repo.find_valid_reset_token(token).await {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(ResetPasswordResult::InvalidToken),
            Err(e) => return Err(e.into()),
        };

        let hash = hash_in_blocking(password).await?;

        let updated = self
            .user_repo
            .update_password_hash(user_id, &hash)
            .await?;

        if !updated {
            // The user was deleted between token validation and the update.
            return Ok(ResetPasswordResult::InvalidToken);
        }

        if let Err(e) = self.user_repo.revoke_tokens_by_user_id(user_id).await {
            tracing::error!("revoke_tokens_by_user_id after password reset: {}", e);
        }

        Ok(ResetPasswordResult::Ok)
    }

    /// 更新用户邮箱地址
    ///
    /// # 参数
    /// - `user_id`: 用户 ID
    /// - `email`: 新邮箱地址
    ///
    /// # 返回
    /// - `Ok(())`: 更新成功
    /// - `Err(ServiceError::Validation)`: 邮箱格式无效或已被其他账号绑定
    /// - `Err(ServiceError)`: 数据库错误
    ///
    /// # 错误
    /// - 邮箱格式无效时返回验证错误
    /// - 邮箱已被其他账号绑定时返回唯一约束冲突错误
    /// - 其他数据库错误时返回内部错误
    ///
    /// # 安全
    /// - 邮箱地址会 trim 并转为小写后存储
    /// - 验证邮箱格式（长度、@ 符号、域名格式等）
    /// - 检查唯一约束，防止重复绑定
    pub async fn update_email(
        &self,
        user_id: i64,
        email: &str,
    ) -> Result<(), ServiceError> {
        let email = email.trim().to_lowercase();

        if !is_valid_email(&email) {
            return Err(ServiceError::Validation("请输入有效的邮箱地址".into()));
        }

        self.user_repo
            .update_email(user_id, &email)
            .await
            .map_err(|e| {
                if let sqlx::Error::Database(ref db_err) = e {
                    if db_err.constraint() == Some("idx_users_email_unique") {
                        return ServiceError::Validation("该邮箱已被其他账号绑定".into());
                    }
                }
                tracing::error!("update_email: {}", e);
                ServiceError::from(e)
            })?;

        Ok(())
    }

    /// 发送邮箱验证邮件
    ///
    /// # 参数
    /// - `user_id`: 用户 ID
    /// - `username`: 用户名，用于邮件内容
    /// - `email_service`: 邮件服务实例，用于发送验证邮件
    ///
    /// # 返回
    /// - `Ok(SendVerificationEmailResult)`: 发送结果
    ///   - `ok`: 是否成功发送
    ///   - `message`: 结果消息
    ///
    /// # 错误
    /// - 超出速率限制（每 5 分钟最多 2 次）时返回失败提示
    /// - SMTP 未配置时返回提示信息
    ///
    /// # 行为
    /// - 生成验证 token 并存储到数据库
    /// - 构造验证链接（包含 token 和公共 URL）
    /// - 通过邮件服务发送验证邮件
    /// - SMTP 未配置时直接标记邮箱为已验证（开发/测试模式）
    ///
    /// # 安全
    /// - 验证 token 有有效期（通常 24 小时）
    /// - 用户级速率限制防止滥用
    /// - 验证链接包含一次性 token，使用后失效
    pub async fn send_verification_email(
        &self,
        user_id: i64,
        username: &str,
        email_service: &EmailService,
    ) -> Result<SendVerificationEmailResult, ServiceError> {
        // 用户级速率限制：每 5 分钟最多 2 次请求
        let key = format!("verify_email:user:{}", user_id);
        if self
            .rate_limiter
            .check_with(&key, 2, 300, 600)
            .await
            .is_err()
        {
            return Ok(SendVerificationEmailResult {
                ok: false,
                message: "请求过于频繁，请稍后再试。".into(),
            });
        }

        let email = self.user_repo.get_email(user_id).await.ok().flatten();

        if email_service.is_configured() {
            if let Some(ref email) = email {
                if let Ok(token) = self
                    .user_repo
                    .create_email_verification_token(user_id)
                    .await
                {
                    let verify_url = format!(
                        "{}/auth/verify-email?token={}",
                        self.config.public_url.trim_end_matches('/'),
                        token
                    );
                    email_service
                        .send_email_verification(email, username, &verify_url)
                        .await;
                }
            }

            Ok(SendVerificationEmailResult {
                ok: true,
                message: "验证邮件已发送。如果您的邮箱没有收到，请稍后再试。".into(),
            })
        } else {
            // SMTP 未配置时直接标记已验证（开发/测试模式）
            if email.is_some() {
                let _ = self.user_repo.verify_email(user_id).await;
            }

            Ok(SendVerificationEmailResult {
                ok: true,
                message: "验证邮件功能未配置。请联系管理员。".into(),
            })
        }
    }

    /// 获取用户基本信息
    ///
    /// # 参数
    /// - `username`: 用户名
    /// - `is_admin`: 是否为管理员（用于返回给客户端）
    /// - `tenant_id`: 租户 ID，用于多租户隔离
    ///
    /// # 返回
    /// - `Ok(UserInfoResponse)`: 用户信息响应
    ///   - `id`: 用户 ID（如果用户不存在则为 0）
    ///   - `username`: 用户名
    ///   - `is_admin`: 是否为管理员
    ///   - `created_at`: 注册时间（格式：YYYY-MM-DD HH:MM:SS）
    ///   - `email`: 邮箱地址（可能为 None）
    ///   - `email_verified`: 邮箱是否已验证
    /// - `Err(ServiceError)`: 数据库错误
    ///
    /// # 行为
    /// - 查询用户信息，如果用户不存在返回默认值（id=0，空字符串等）
    /// - 格式化注册时间为指定格式
    pub async fn user_info(
        &self,
        username: &str,
        is_admin: bool,
        tenant_id: i64,
    ) -> Result<UserInfoResponse, ServiceError> {
        let user = self
            .user_repo
            .find_by_username(tenant_id, username)
            .await
            .ok()
            .flatten();

        Ok(UserInfoResponse {
            id: user.as_ref().map(|u| u.id).unwrap_or(0),
            username: username.to_string(),
            is_admin,
            created_at: user
                .as_ref()
                .map(|u| u.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default(),
            email: user.as_ref().and_then(|u| u.email.clone()),
            email_verified: user.as_ref().map(|u| u.email_verified).unwrap_or(false),
        })
    }

    /// 获取用户个人资料
    ///
    /// # 参数
    /// - `username`: 用户名
    /// - `is_admin`: 是否为管理员（用于返回给客户端）
    /// - `tenant_id`: 租户 ID，用于多租户隔离
    ///
    /// # 返回
    /// - `Ok(UserProfileResponse)`: 用户个人资料响应
    ///   - `username`: 用户名
    ///   - `is_admin`: 是否为管理员
    ///   - `created_at`: 注册时间（格式：YYYY-MM-DD HH:MM:SS）
    ///   - `total_videos_watched`: 观看视频总数
    ///   - `total_watch_time_ms`: 总观看时长（毫秒）
    ///   - `recent_history`: 最近观看历史（最多 10 条）
    /// - `Err(ServiceError)`: 数据库错误
    ///
    /// # 行为
    /// - 查询用户注册时间
    /// - 通过 `PlaybackService` 获取播放统计数据
    /// - 如果播放数据查询失败，返回默认值（0, 0, 空列表）
    pub async fn user_profile(
        &self,
        username: &str,
        is_admin: bool,
        tenant_id: i64,
    ) -> Result<UserProfileResponse, ServiceError> {
        let created_at = self
            .user_repo
            .find_by_username(tenant_id, username)
            .await
            .ok()
            .flatten()
            .map(|u| u.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();

        let (total_watched, total_time, recent) = self
            .playback_service
            .get_user_profile_data(username)
            .await
            .unwrap_or((0, 0, vec![]));

        Ok(UserProfileResponse {
            username: username.to_string(),
            is_admin,
            created_at,
            total_videos_watched: total_watched,
            total_watch_time_ms: total_time,
            recent_history: recent,
        })
    }

    /// 获取 Cookie Secure 标志
    ///
    /// # 返回
    /// - `true`: Cookie 仅在 HTTPS 连接下传输
    /// - `false`: Cookie 在 HTTP 和 HTTPS 连接下均可传输
    ///
    /// # 用途
    /// - 用于设置认证 Cookie 的 Secure 属性
    /// - 生产环境应为 `true`，开发环境可设为 `false`
    pub fn cookie_secure(&self) -> bool {
        self.config.cookie_secure
    }

    /// Shared rate limit check: IP-level + per-username.
    /// SECURITY: per-username keying enables an account-DoS where an attacker
    /// who knows a username can lock it out for 10 minutes. The per-username
    /// rate limit uses a short window (3 attempts per minute) and short block
    /// (300s) to limit the impact. IP-level rate limiting provides the primary
    /// brute-force defense.
    async fn check_rate_limits(
        &self,
        username: &str,
        client_ip: &str,
        action: &str,
    ) -> Result<(), ServiceError> {
        let ip_key = format!("auth:ip:{}", client_ip);
        if self
            .ip_rate_limiter
            .check_with(
                &ip_key,
                IP_RATE_LIMIT_MAX_ATTEMPTS,
                IP_RATE_LIMIT_WINDOW_SECS,
                IP_RATE_LIMIT_BLOCK_SECS,
            )
            .await
            .is_err()
        {
            tracing::warn!(username = %sanitize_for_log(username), ip = %sanitize_for_log(client_ip), "{} rejected: IP rate limited", action);
            return Err(ServiceError::RateLimited);
        }

        let key = format!("auth:{}", username.trim().to_lowercase());
        if self.rate_limiter.check(&key).await.is_err() {
            tracing::warn!(username = %sanitize_for_log(username), ip = %sanitize_for_log(client_ip), "{} rejected: username rate limited", action);
            return Err(ServiceError::RateLimited);
        }
        Ok(())
    }
}

/// 密码重置结果枚举
///
/// 用于表示 `AuthService::reset_password` 方法的执行结果
#[derive(Debug)]
pub enum ResetPasswordResult {
    /// 密码重置成功
    Ok,
    /// 重置 token 无效或已过期
    InvalidToken,
    /// 验证错误（如密码强度不足）
    Error(String),
}

/// 发送验证邮件结果
///
/// 用于表示 `AuthService::send_verification_email` 方法的执行结果
#[derive(Debug, serde::Serialize)]
pub struct SendVerificationEmailResult {
    pub ok: bool,
    pub message: String,
}

/// 清理用户输入中的控制字符，防止日志注入攻击
///
/// # 参数
/// - `s`: 原始字符串
///
/// # 返回
/// - 清理后的字符串，控制字符被替换为 `?`
///
/// # 安全
/// - 防止攻击者通过用户名等字段注入换行符伪造日志记录
/// - 控制字符包括：换行符、回车符、制表符等
fn sanitize_for_log(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// 在阻塞线程池上执行 Argon2id 哈希
///
/// # 参数
/// - `password`: 明文密码
///
/// # 返回
/// - `Ok(String)`: 哈希后的密码字符串
/// - `Err(ServiceError)`: 哈希失败或任务执行失败
///
/// # 性能
/// - Argon2id 默认参数下每次调用消耗 ~50-100ms CPU
/// - 使用 `tokio::task::spawn_blocking` 避免阻塞异步运行时
/// - 防止大量登录/注册请求耗尽异步工作线程
async fn hash_in_blocking(password: &str) -> Result<String, ServiceError> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || password::hash(&password))
        .await
        .map_err(|e| ServiceError::internal(format!("password hashing task failed: {}", e)))?
        .map_err(ServiceError::from)
}

/// 在阻塞线程池上执行 Argon2id 验证
///
/// # 参数
/// - `password`: 明文密码
/// - `hash`: 存储的哈希值
///
/// # 返回
/// - `true`: 密码验证成功
/// - `false`: 密码验证失败或发生错误
///
/// # 安全
/// - 任何错误（如哈希格式错误、任务执行失败）都返回 `false`
/// - 调用方将所有失败视为"凭据被拒绝"
/// - 防止通过错误信息泄露系统内部状态
async fn verify_in_blocking(password: &str, hash: &str) -> bool {
    let password = password.to_owned();
    let hash = hash.to_owned();
    tokio::task::spawn_blocking(move || password::verify(&password, &hash))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false)
}

/// 预计算的 Argon2id 哈希，用于防止用户名枚举时序攻击
///
/// # 用途
/// - 当用户不存在时，使用此哈希执行 dummy argon2 验证
/// - 使"用户不存在"和"密码错误"两个分支的执行时间一致
/// - 防止攻击者通过响应时间差异枚举有效用户名
///
/// # 安全
/// - 由 `argon2::hash_encoded` 生成的随机字符串哈希
/// - 如果泄露，可以轻松重新生成
const DUMMY_ARGON2_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$ZHVtbXlzYWx0Zm9yYXRtb3M$Ix2m8ZxRg2E2XgO6nQ8T0q3yJ4cZUEZ5K6yJxY7GQhA";

/// 检查密码强度是否足够
///
/// # 参数
/// - `pw`: 明文密码
///
/// # 返回
/// - `true`: 密码强度足够
/// - `false`: 密码强度不足
///
/// # 规则
/// - 密码长度 < 12 字符：需要包含至少 3 种字符类型（大写、小写、数字、特殊字符）
/// - 密码长度 >= 12 字符：需要包含至少 2 种字符类型
///
/// # 安全
/// - 字符计数使用 `chars().count()` 而非字节长度，支持多字节字符
/// - 防止过于简单的密码（如纯数字、纯小写字母）
pub(crate) fn is_password_strong_enough(pw: &str) -> bool {
    let has_upper = pw.chars().any(|c| c.is_uppercase());
    let has_lower = pw.chars().any(|c| c.is_lowercase());
    let has_digit = pw.chars().any(|c| c.is_ascii_digit());
    let has_special = pw.chars().any(|c| !c.is_alphanumeric());
    let categories: u8 = [has_upper, has_lower, has_digit, has_special]
        .into_iter()
        .map(u8::from)
        .sum();

    // Require at least 3 of 4 character categories for passwords under 12
    // chars. Counted in chars, not bytes, to match register()'s length
    // validation — a byte count would let a multibyte password slip into
    // the lenient 2-category tier.
    if pw.chars().count() < 12 {
        categories >= 3
    } else {
        // Longer passwords can be all lowercase with at least one non-alpha
        categories >= 2
    }
}

/// 验证邮箱地址格式
///
/// # 参数
/// - `email`: 邮箱地址字符串
///
/// # 返回
/// - `true`: 邮箱格式有效
/// - `false`: 邮箱格式无效
///
/// # 验证规则
/// - 邮箱长度在 1-254 字符之间
/// - 包含且仅包含一个 `@` 符号
/// - 本地部分（@ 前）长度在 1-64 字符之间
/// - 域名部分（@ 后）包含至少一个 `.`，且不以 `.` 开头或结尾
/// - 域名中不包含连续的 `..`
/// - 不包含空格或控制字符（防止 SMTP 头注入）
///
/// # 安全
/// - 防止 SMTP 头注入攻击
/// - 防止命令注入
pub(crate) fn is_valid_email(email: &str) -> bool {
    if email.is_empty() || email.len() > 254 {
        return false;
    }
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 {
        return false;
    }
    let (local, domain) = (parts[0], parts[1]);
    if local.is_empty() || local.len() > 64 {
        return false;
    }
    if domain.is_empty() || !domain.contains('.') {
        return false;
    }
    if domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }
    if domain.contains("..") {
        return false;
    }
    // Reject whitespace and control characters: they break the envelope
    // and can be abused for header/command injection in SMTP.
    if email.chars().any(char::is_whitespace) || email.chars().any(char::is_control) {
        return false;
    }
    true
}

/// 创建认证错误响应
///
/// # 参数
/// - `msg`: 错误信息
///
/// # 返回
/// - `AuthResponse`: 包含错误信息的响应对象（`ok: false`）
fn auth_err(msg: impl Into<String>) -> AuthResponse {
    AuthResponse {
        ok: false,
        token: None,
        error: Some(msg.into()),
    }
}

/// 将密码操作错误转换为服务层错误
///
/// # 行为
/// - 记录错误日志
/// - 返回通用内部错误（不泄露密码哈希实现细节）
impl From<password::PasswordError> for ServiceError {
    fn from(e: password::PasswordError) -> Self {
        tracing::error!("password operation failed: {}", e);
        ServiceError::internal("password error")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strong_password_accepted() {
        assert!(is_password_strong_enough("MyStr0ngPwd"));
        assert!(is_password_strong_enough("Abcdef1!"));
        assert!(is_password_strong_enough("correct-horse-battery"));
    }

    #[test]
    fn test_weak_password_rejected() {
        assert!(!is_password_strong_enough("password"));
        assert!(!is_password_strong_enough("12345678"));
        assert!(!is_password_strong_enough("abcdefgh"));
        assert!(!is_password_strong_enough("PASSWORD1"));
    }

    #[test]
    fn test_long_password_with_two_categories() {
        assert!(is_password_strong_enough("longbutonlylowercase1"));
        assert!(is_password_strong_enough("longbutonlyUPPERCASE1"));
    }

    #[test]
    fn dummy_argon2_hash_is_parseable() {
        // SECURITY: DUMMY_ARGON2_HASH is used to keep the "user not found"
        // login branch running a real argon2 verify. If it ever becomes
        // unparseable, verify() fails fast and the username-enumeration
        // timing oracle reopens.
        match password::verify("not-a-real-password", DUMMY_ARGON2_HASH) {
            Ok(_) => {}
            Err(e) => panic!(
                "DUMMY_ARGON2_HASH must be a parseable argon2 hash, got: {}",
                e
            ),
        }
    }

    #[test]
    fn username_control_chars_are_rejected_by_validation() {
        // The check lives in register(); this guards the predicate used there.
        let bad = ["a\nbc", "a\rbc", "a\u{0}bc"];
        for name in bad {
            assert!(
                name.chars().any(char::is_control),
                "{:?} should contain control chars",
                name
            );
        }
        assert!("alice".chars().all(|c| !c.is_control()));
    }

    #[test]
    fn test_valid_email() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("a.b@c.d"));
        assert!(is_valid_email("user+tag@example.com"));
    }

    #[test]
    fn test_invalid_email() {
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("no-at"));
        assert!(!is_valid_email("@no-local"));
        assert!(!is_valid_email("no-domain@"));
        assert!(!is_valid_email("user@.com"));
        assert!(!is_valid_email("user@com."));
        assert!(!is_valid_email("user@dom..ain"));
        assert!(!is_valid_email("user name@example.com"));
        assert!(!is_valid_email("user\nname@example.com"));
    }
}
