# Atmos Video 安全文档

本文档详细描述 Atmos Video 平台的安全架构、防护机制及安全运维指南。

---

## 目录

1. [认证机制](#1-认证机制)
2. [授权模型](#2-授权模型)
3. [输入验证策略](#3-输入验证策略)
4. [常见攻击防护](#4-常见攻击防护)
5. [安全配置建议](#5-安全配置建议)
6. [漏洞报告流程](#6-漏洞报告流程)

---

## 1. 认证机制

### 1.1 认证架构概述

Atmos Video 采用 **Bearer Token + Cookie** 双通道认证机制。认证流程经过以下中间件栈：

```
请求 → security_headers → inject_state → resolve_tenant → bearer_auth → role_auth → handler
```

### 1.2 Token 机制

| 属性 | 说明 |
|------|------|
| **Token 格式** | 256 位随机 Alphanumeric 字符串（非 UUID） |
| **生成方式** | 使用密码学安全随机数生成器（`OsRng`） |
| **有效期** | 7 天（`COOKIE_MAX_AGE = 604800`） |
| **存储方式** | 数据库 `auth_tokens` 表，支持多租户隔离 |
| **传递方式** | `Authorization: Bearer <token>` 请求头 或 HTTP Cookie |

**认证流程：**

1. 客户端发送 `POST /auth/login`，携带用户名和密码
2. 服务端验证凭据（通过 `bearer_auth` 中间件）
3. 验证成功后生成随机 token 并返回
4. 后续请求携带 token 访问受保护资源
5. 定期清理过期 token（每 5 分钟）

### 1.3 密码安全

| 属性 | 说明 |
|------|------|
| **哈希算法** | Argon2id（内存硬哈希函数，抗 GPU/ASIC 攻击） |
| **随机盐** | 每次哈希使用 `SaltString::generate(&mut OsRng)` 生成独立随机盐 |
| **密码长度** | 最小 8 字符，最大 128 字符 |
| **密码强度** | 拒绝常见弱密码和过于简单的密码 |

```rust
// 密码哈希示例（backend/src/util/password.rs）
pub fn hash(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}
```

**安全特性：**
- 用户名不存在时仍执行 dummy argon2 验证，防止用户名枚举时序攻击
- 登录失败不区分"用户不存在"和"密码错误"，返回统一错误信息
- 登录成功后重置该用户名的速率限制计数器

### 1.4 注册控制

- 默认关闭注册（`REGISTRATION_ENABLED=false`）
- 注册功能可由管理员动态开启/关闭（持久化到数据库）
- 支持用户审批机制（`approve_user` 管理接口）
- 首个注册用户默认不自动成为管理员（`ALLOW_FIRST_USER_ADMIN=false`）

### 1.5 密码重置与邮箱验证

- 支持密码重置流程（`/auth/forgot-password` → `/auth/reset-password`）
- 支持邮箱验证（`/auth/send-verification-email` → `/auth/verify-email`）
- 依赖 SMTP 配置（`SMTP_HOST`、`SMTP_PORT`、`SMTP_USERNAME`、`SMTP_PASSWORD`、`SMTP_FROM`）

---

## 2. 授权模型

### 2.1 角色体系

Atmos Video 采用基于角色的访问控制（RBAC），角色通过数值权限级别区分：

| 角色级别 | 角色名 | 权限说明 |
|---------|--------|---------|
| `0` | 未激活 | 无任何访问权限 |
| `1`+ | 普通用户（Viewer） | 浏览视频、播放、评论、收藏等基本操作 |
| Admin | 管理员 | 用户管理、视频管理、系统配置、数据统计等全部权限 |

### 2.2 路由权限矩阵

```
公开路由（无需认证）
├── /health                          # 健康检查
├── /auth/register                   # 用户注册
├── /auth/login                      # 用户登录
├── /auth/forgot-password            # 忘记密码
├── /auth/reset-password             # 重置密码
├── /auth/verify-email               # 邮箱验证
├── /tags                            # 标签列表（只读）
├── /recommendations/trending        # 热门推荐
├── /recommendations/recent          # 最新视频
├── /recommendations/similar/{id}    # 相似推荐
└── /share/{token}                   # 分享链接访问

用户路由（Bearer Auth + Role >= 1）
├── /auth/user/*                     # 用户信息管理
├── /videos/*                        # 视频浏览、搜索、收藏
├── /playback/*                      # 播放历史
├── /playlists/*                     # 播放列表
├── /comments/*                      # 评论
├── /videos/{id}/share               # 分享链接管理
└── /recommendations                 # 个性化推荐

管理员路由（Bearer Auth + Admin Auth）
├── /admin/users/*                   # 用户管理
├── /admin/videos/*                  # 视频管理（上传、编辑、删除、转码）
├── /admin/tags/*                    # 标签管理
├── /admin/stats                     # 系统统计
├── /admin/config/*                  # 系统配置
├── /admin/system                    # 系统信息
└── /admin/logs                      # 日志管理

内部路由（Bearer Auth + Admin Auth）
├── /server/info                     # 服务器信息
├── /metrics                         # 指标数据
└── /metrics/prometheus              # Prometheus 格式指标

文档路由（Bearer Auth + Role >= 1）
├── /docs/openapi.json               # OpenAPI 规范
└── /docs                            # API 文档
```

### 2.3 认证中间件栈

```
bearer_auth          → 验证 token 有效性，注入用户信息到请求扩展
├── token 验证       → 查询 auth_tokens 表，验证 token 存在且未过期
├── 用户信息注入     → 将 user_id、role、tenant_id 注入请求扩展
└── Cookie 支持      → 同时支持 Authorization 头和 Cookie 两种方式

role_auth(N)         → 验证用户角色级别 >= N
├── 角色 1+          → 普通用户可访问
└── 角色检查         → 从请求扩展中获取用户角色进行比对

admin_auth           → 验证管理员权限
├── 管理员角色检查   → 验证 is_admin 标志
└── 操作日志         → 记录管理员操作
```

### 2.4 多租户隔离

- 每个请求通过 `resolve_tenant` 中间件解析租户
- 租户基于 `Host` 请求头进行路由
- 所有数据查询自动注入 `tenant_id` 过滤条件
- 用户、视频、播放列表等资源严格按租户隔离

### 2.5 资源所有权检查

- 用户只能操作自己创建的资源（播放列表、评论、分享链接等）
- 管理员可操作所有资源
- 删除操作前进行所有权验证

---

## 3. 输入验证策略

### 3.1 通用验证规则

| 字段 | 验证规则 |
|------|---------|
| 用户名 | 长度 2-64 字符，拒绝控制字符（`\n`、`\r`、`\t`、`\x1b` 等） |
| 密码 | 长度 8-128 字符，拒绝常见弱密码 |
| 视频标题 | 最大 500 字符 |
| 批量操作 | 单次最多 1000 条记录 |
| 搜索查询 | 长度限制 + 特殊字符转义 |
| 标签名 | 长度限制 + 去重 |

### 3.2 认证输入验证

```rust
// 用户名验证（backend/src/services/auth_service.rs）
if username.is_empty() || password.is_empty() {
    return Ok(auth_err("用户名和密码不能为空"));
}
if username.len() < 2 || username.len() > 64 {
    return Ok(auth_err("用户名长度需在 2-64 个字符之间"));
}
// 拒绝控制字符，防止日志注入和 UI 注入
if username.chars().any(char::is_control) {
    return Ok(auth_err("用户名包含非法字符"));
}
```

### 3.3 日志注入防护

所有用户输入在写入日志前都经过 `sanitize_for_log()` 处理：

```rust
fn sanitize_for_log(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}
```

- 将控制字符（换行符、制表符、ANSI 转义序列等）替换为 `?`
- 保留非 ASCII 可打印字符（中文等）
- 防止攻击者通过用户名等字段伪造日志条目

### 3.4 SQL 注入防护

- 使用 SQLx 参数化查询，所有用户输入通过绑定参数传递
- 禁止字符串拼接构造 SQL
- 利用 Rust 类型系统保证查询参数类型安全

### 3.5 请求体大小限制

| 路由类型 | 大小限制 |
|---------|---------|
| 普通 API | Axum 默认限制 |
| 上传路由 | `DefaultBodyLimit::disable()`（无限制） |
| 评论/描述 | 最大长度限制 |

### 3.6 Referer/Origin 验证

热链接防护中间件对 `Referer` 和 `Origin` 头进行严格验证：

```rust
// 拒绝以下格式
"example.com/"           // 缺少协议
"//example.com/"         // 协议相对路径
"https:/example.com"     // 格式错误
"javascript:alert(1)"   // 非 HTTP 协议
"null"                   // 沙盒 iframe 来源
```

---

## 4. 常见攻击防护

### 4.1 CSRF 防护

- **CORS 策略**：仅允许配置的来源（`CORS_ORIGIN` 环境变量）
- **SameSite Cookie**：Cookie 设置 `SameSite` 属性
- **自定义请求头**：要求 `X-CSRF-Token` 头（通过 CORS 允许头部配置）
- **Preflight 缓存**：`Access-Control-Max-Age: 3600`（1 小时）

```rust
// CORS 配置（backend/src/middleware/security.rs）
CorsLayer::new()
    .allow_methods([GET, POST, PUT, DELETE, OPTIONS])
    .allow_headers([CONTENT_TYPE, AUTHORIZATION, X-CSRF-TOKEN, ...])
    .max_age(Duration::from_secs(3600))
```

### 4.2 点击劫持防护

- `X-Frame-Options: DENY`
- CSP `frame-ancestors 'none'`
- CSP `frame-src 'none'`
- COOP `Cross-Origin-Opener-Policy: same-origin`

### 4.3 XSS 防护

**Content Security Policy (CSP)：**

```
default-src 'self';
base-uri 'self';
form-action 'self';
frame-ancestors 'none';
frame-src 'none';
object-src 'none';
img-src 'self' data:;
media-src 'self' blob:;
style-src 'self';
font-src 'self' data:;
script-src 'self' https://static.cloudflareinsights.com;
connect-src 'self' https://static.cloudflareinsights.com;
```

- 禁止内联脚本（`unsafe-inline` 未启用）
- 禁止 `eval()`
- 禁止第三方脚本（仅允许 Cloudflare Insights）
- 禁止插件嵌入（`object-src 'none'`）

### 4.4 MIME 类型嗅探防护

- `X-Content-Options: nosniff`（全局响应头）

### 4.5 速率限制

#### 登录速率限制（用户名级别）

| 参数 | 值 |
|------|-----|
| 窗口时间 | 60 秒 |
| 最大尝试次数 | 5 次 |
| 封锁时长 | 300 秒（5 分钟） |

#### 登录速率限制（IP 级别）

| 参数 | 值 |
|------|-----|
| 窗口时间 | 60 秒 |
| 最大尝试次数 | 30 次 |
| 封锁时长 | 0（仅窗口限制） |

#### 分享链接速率限制

| 参数 | 值 |
|------|-----|
| 窗口时间 | 60 秒 |
| 最大尝试次数 | 30 次 |
| 封锁时长 | 0 |

**实现特点：**
- 支持 Redis 持久化（`REDIS_URL` 配置）
- Redis 不可用时自动降级为内存限流（DashMap）
- 使用 Lua 脚本保证 Redis 操作原子性
- 定期清理过期条目（每 5 分钟）

### 4.6 热链接防护

**`/media/*` 路径受 `hotlink_guard` 中间件保护：**

- 验证 `Referer` 头的 Host 必须匹配 `PUBLIC_URL` 的 Host
- 验证 `Origin` 头必须匹配 `PUBLIC_URL` 的完整 Origin（scheme + host + port）
- 缺少 `Referer` 和 `Origin` 的请求视为直接下载，允许通过
- 格式错误的头部值直接拒绝（403 Forbidden）
- Host 比较为大小写不敏感
- Origin 比较遵循 RFC 6454（不同端口视为不同源）

**防护的攻击向量：**
- 用户名伪装：`https://example.com@evil.example/`
- 反斜杠绕过：`https://example.com\@evil.example/`
- URL 编码绕过：`https://example%2Ecom/`
- 尾部点号：`https://example.com./`
- 缺少协议：`example.com/`
- 协议相对：`//example.com/`

### 4.7 HSTS 防护

**条件性 HSTS（仅 HTTPS）：**

```
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
```

- 仅当请求通过受信任代理（Cloudflare IP 范围 或 `TRUSTED_PROXY=1`）且 `X-Forwarded-Proto: https` 时发送
- 非受信任对等方无法伪造 `X-Forwarded-Proto` 来获取 HSTS 头
- RFC 6797 §7.2 合规：HSTS 绝不在纯 HTTP 响应中发送

### 4.8 分享 Token 枚举防护

- 分享链接 token 使用密码学安全随机数生成
- 公开端点 `/share/{token}` 受速率限制（30 次/分钟/IP）
- 响应大小差异被速率限制中和，防止基于响应大小的侧信道攻击

### 4.9 用户枚举防护

- 注册时用户名已存在会执行 dummy argon2 哈希验证，消除时序差异
- 登录失败返回统一错误信息，不区分用户不存在和密码错误
- 批量操作有数量限制（≤1000）

### 4.10 目录遍历防护

- 媒体文件通过 Axum 的 `ServeDir` 提供服务
- 静态文件服务基于文件系统路径，不允许路径穿越
- SPA 回退仅返回 `index.html`，不暴露服务器文件结构

---

## 5. 安全配置建议

### 5.1 环境变量安全配置

#### 必须配置

| 变量 | 建议值 | 说明 |
|------|--------|------|
| `PUBLIC_URL` | `https://your-domain.com` | 生产环境必须使用 HTTPS |
| `APP_ENV` | `production` | 启用 HTTPS 重定向和严格 CORS |
| `COOKIE_SECURE` | `true` | Cookie 仅通过 HTTPS 传输 |
| `CORS_ORIGIN` | `https://your-domain.com` | 仅允许可信来源 |
| `REGISTRATION_ENABLED` | `false` | 默认关闭，按需开启 |
| `HASHID_SALT` | 随机生成的长字符串 | 必须跨重启保持稳定 |

#### 数据库安全

| 变量 | 建议值 | 说明 |
|------|--------|------|
| `DATABASE_URL` | 使用强密码的连接串 | 限制数据库用户权限 |
| `DB_MAX_CONNECTIONS` | `100`（默认） | 根据负载调整 |

#### Redis 安全

| 变量 | 建议值 | 说明 |
|------|--------|------|
| `REDIS_URL` | `redis://:password@host:6379/0` | 启用密码认证 |

#### 代理配置

| 变量 | 建议值 | 说明 |
|------|--------|------|
| `TRUSTED_PROXY` | `0`（默认） | 仅在使用反向代理时设为 `1` |

### 5.2 反向代理配置

#### Nginx 示例

```nginx
server {
    listen 443 ssl http2;
    server_name video.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # 传递真实客户端 IP
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header Host $host;

    # 文件上传大小限制
    client_max_body_size 10G;

    location / {
        proxy_pass http://127.0.0.1:8082;
    }
}
```

#### Cloudflare 配置

- 启用 Cloudflare WAF
- 配置 Rate Limiting 规则
- 启用 Bot Management
- 配置 Page Rules 缓存策略

### 5.3 数据库安全

```sql
-- 创建专用数据库用户
CREATE USER atmos_app WITH PASSWORD 'strong_random_password';

-- 授予最小必要权限
GRANT CONNECT ON DATABASE atmos_video TO atmos_app;
GRANT USAGE ON SCHEMA public TO atmos_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO atmos_app;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO atmos_app;

-- 设置默认权限
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO atmos_app;
```

### 5.4 SSL/TLS 配置

- 最低 TLS 版本：TLS 1.2
- 推荐 TLS 版本：TLS 1.3
- 禁用不安全密码套件
- 启用 HSTS preload

### 5.5 文件权限

```bash
# 媒体文件目录
chmod 750 /path/to/media
chown atmos:atmos /path/to/media

# 日志目录
chmod 750 /path/to/logs
chown atmos:atmos /path/to/logs

# 配置文件
chmod 600 /path/to/.env
chown atmos:atmos /path/to/.env
```

### 5.6 日志安全

- 所有用户输入在日志中经过 `sanitize_for_log()` 处理
- 日志级别建议：`info`（生产环境）
- 敏感信息（密码、token）绝不写入日志
- 使用结构化日志便于安全审计

### 5.7 定期安全任务

| 任务 | 频率 | 说明 |
|------|------|------|
| 过期 Token 清理 | 每 5 分钟 | 自动执行 |
| 过期分享链接清理 | 每小时 | 自动执行 |
| 播放会话清理 | 每 30 秒 | 自动执行 |
| 速率限制条目清理 | 每 5 分钟 | 自动执行 |
| 数据库备份 | 每日 | 需要配置 |
| 依赖漏洞扫描 | 每周 | `cargo audit` |

---

## 6. 漏洞报告流程

### 6.1 报告渠道

如果您发现了安全漏洞，请通过以下方式报告：

**首选方式（安全邮箱）：**
- 发送邮件至：`security@example.com`（请替换为实际邮箱）
- 使用 PGP 加密（公钥可在项目仓库获取）

**备选方式：**
- GitHub Security Advisories（私有报告）
- 项目维护者直接联系

### 6.2 报告内容

请在报告中包含以下信息：

1. **漏洞描述**
   - 漏洞类型（XSS、SQL 注入、CSRF 等）
   - 影响范围（受影响的 API 端点、功能模块）
   - 攻击向量（如何触发）

2. **复现步骤**
   - 详细的步骤说明
   - 请求/响应示例（去除敏感信息）
   - 截图或视频（如适用）

3. **影响评估**
   - 潜在影响（数据泄露、权限提升等）
   - 影响的用户范围
   - 严重程度自评

4. **环境信息**
   - 软件版本
   - 操作系统
   - 浏览器/客户端版本

### 6.3 响应流程

| 阶段 | 时间目标 | 说明 |
|------|---------|------|
| 确认收悉 | 24 小时 | 自动回复确认 |
| 初步评估 | 72 小时 | 验证漏洞有效性 |
| 详细分析 | 7 天 | 确定影响范围和修复方案 |
| 修复发布 | 14-30 天 | 根据严重程度调整 |
| 公开披露 | 修复后 90 天 | 协调披露时间 |

### 6.4 严重程度分类

| 级别 | 描述 | 响应时间 |
|------|------|---------|
| **严重 (Critical)** | 远程代码执行、认证绕过、大规模数据泄露 | 24 小时 |
| **高 (High)** | 权限提升、SQL 注入、敏感信息泄露 | 72 小时 |
| **中 (Medium)** | XSS、CSRF、信息泄露（有限范围） | 7 天 |
| **低 (Low)** | 配置问题、信息泄露（最小影响） | 14 天 |

### 6.5 安全研究者指南

#### 测试环境

- 请使用本地部署环境进行测试
- 不要在生产环境进行漏洞测试
- 不要访问或修改其他用户的数据

#### 允许的测试行为

- 手动安全测试
- 自动化扫描（低频率）
- 功能逻辑测试

#### 禁止的行为

- 拒绝服务攻击（DoS/DDoS）
- 社会工程学攻击
- 物理攻击
- 访问或修改生产数据
- 影响其他用户正常使用

### 6.6 致谢

我们感谢安全研究者的负责任披露。对于有效漏洞报告，我们提供：

- 安全公告中的致谢（可选匿名）
- 项目 README 中的贡献者列表
- 适当的漏洞赏金（如适用）

### 6.7 安全更新订阅

- 关注 GitHub Security Advisories
- 订阅项目 Release 通知
- 定期检查依赖安全公告

---

## 附录 A：安全相关源代码索引

| 模块 | 文件路径 | 说明 |
|------|---------|------|
| 认证中间件 | `backend/src/middleware/auth.rs` | Token 验证、角色检查 |
| 安全头中间件 | `backend/src/middleware/security.rs` | CSP、CORS、HSTS |
| 速率限制 | `backend/src/middleware/rate_limit.rs` | 登录限流、Redis 持久化 |
| 热链接防护 | `backend/src/middleware/hotlink.rs` | Referer/Origin 验证 |
| 分享限流 | `backend/src/middleware/share_rate_limit.rs` | 分享端点限流 |
| 租户解析 | `backend/src/middleware/tenant.rs` | 多租户隔离 |
| 密码工具 | `backend/src/util/password.rs` | Argon2id 哈希 |
| 认证服务 | `backend/src/services/auth_service.rs` | 注册、登录、密码重置 |
| 路由定义 | `backend/src/app.rs` | 路由组和中间件配置 |

## 附录 B：安全头参考

```
# 全局安全响应头
X-Content-Type-Options: nosniff
X-Frame-Options: DENY
Referrer-Policy: no-referrer
Permissions-Policy: geolocation=(), microphone=(), camera=()
Content-Security-Policy: <见 CSP 策略>
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin

# 条件安全头（仅 HTTPS）
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
```

---

*本文档最后更新：2025 年*

*如有疑问，请联系安全团队。*
