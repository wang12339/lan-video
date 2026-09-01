# Atmos Video 安全审计报告

**审计日期**: 2026-08-13
**审计范围**: 源码审查 + 渗透测试报告分析
**项目**: Atmos Video (Rust/Axum 后端 + React/Vite 前端)

---

## 一、总体评估

Atmos Video 整体安全水平**中等偏上**。项目在认证、密码存储、SQL注入防护、安全头等方面做得很好，但存在若干信息泄露和配置问题需要修复。渗透测试报告发现 4 个高危、6 个中危、3 个低危漏洞，共 13 个安全问题。

**安全评分: 7/10** — 核心安全机制扎实，但攻击面暴露过多。

---

## 二、渗透测试报告分析

### 2.1 高危漏洞 (4个)

| ID | 漏洞 | 状态 | 严重程度 |
|---|---|---|---|
| H1 | `/metrics`、`/health`、`/server/info` 无需认证 | ✅ 已确认 | 高 — 泄露系统内部状态 |
| H2 | `/recommendations/trending` 和 `/recent` 泄露视频元数据 | ✅ 已确认 | 高 — 视频库范围完全暴露 |
| H3 | `/docs/openapi.json` 完整API文档公开 | ✅ 已确认 | 高 — 43个端点的攻击面信息 |
| H4 | Token 存储在 localStorage (前端) | ✅ 源码确认 | 高 — XSS可窃取所有token |

### 2.2 中危漏洞 (6个)

| ID | 漏洞 | 说明 |
|---|---|---|
| M1 | 缺少CSRF防护 | 有 SameSite=Strict 部分防护，但无CSRF token |
| M2 | 视频ID可枚举 | 自增整数ID (3819-15850) |
| M3 | 速率限制内存存储 | 服务器重启后重置 |
| M4 | 注册接口暴露 | 端点存在但返回"已关闭" |
| M5 | 评论/标签无所有权检查 | 任何用户可在任何视频上操作 |
| M6 | Cookie Secure 默认禁用 | `COOKIE_SECURE=false` 时token明文传输 |

### 2.3 低危漏洞 (3个)

| ID | 漏洞 | 说明 |
|---|---|---|
| L1 | CSP允许Cloudflare CDN脚本 | CDN被攻破可注入恶意脚本 |
| L2 | m3u8文件类型检查 | 源码审查发现已修复(见下文) |
| L3 | 错误日志存localStorage | 最多50条，可能泄露API结构 |

---

## 三、源码安全审查详细分析

### 3.1 认证机制 ✅ 优秀

**Token 生成与存储:**
- Token 为 256-bit 字母数字字符串（非UUID），7天过期
- 登录前验证token格式 (`is_valid_auth_token`)，拒绝畸形token避免DB查询放大攻击
- 支持 Bearer header 和 Cookie 两种认证方式
- Token 与租户绑定（multi-tenant隔离），防止跨租户token使用

**密码安全:**
- 使用 Argon2id（内存硬哈希）+ OsRng 随机盐
- 密码要求: 8-128字符，短密码要求3/4字符类别，长密码(≥12)要求2/4
- 时序攻击防护: 用户不存在时仍执行 dummy argon2 verify（`DUMMY_ARGON2_HASH`）
- 用户名枚举防护: 登录失败统一返回"用户名或密码错误"
- 并发注册竞态处理: 唯一约束冲突映射为友好错误
- 密码不在登录前trim，保持一致性

**会话管理:**
- 非管理员用户同一时间只允许一个活跃会话
- 管理员可强制下线用户（token撤销）
- 媒体认证有10秒TTL缓存（moka），减少DB查询

### 3.2 安全中间件 ✅ 优秀

**安全头 (`security.rs`):**
```
X-Content-Type-Options: nosniff
X-Frame-Options: DENY
Referrer-Policy: no-referrer
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload (仅HTTPS)
Permissions-Policy: geolocation=(), microphone=(), camera=()
Content-Security-Policy: default-src 'self'; frame-ancestors 'none'; object-src 'none'; ...
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
```

**CSP 详细策略:**
- `default-src 'self'` — 严格限制默认源
- `frame-ancestors 'none'` + `X-Frame-Options: DENY` — 双重防点击劫持
- `object-src 'none'` + `frame-src 'none'` — 关闭插件/iframe逃逸
- `script-src` 允许 `cloudflareinsights.com` — 可接受的运维需求

**CORS 配置:**
- 仅在配置了 `CORS_ORIGIN` 时才启用 `allow_credentials(true)`
- 允许方法: GET/POST/PUT/DELETE/OPTIONS
- 允许自定义头: `x-csrf-token`, `x-upload-hash/name/size/category`
- Preflight缓存: 1小时

**HTTPS检测安全:**
- `X-Forwarded-Proto` 仅在可信代理(Cloudflare IP或TRUSTED_PROXY=1)时生效
- 非可信peer伪造 `X-Forwarded-Proto: https` 无法获取HSTS
- 有完整的单元测试覆盖各种场景

### 3.3 CSRF防护 ⚠️ 已实现但有改进空间

**当前实现 (`auth.rs:30-66`):**
- 仅对 Cookie 认证的 mutation 请求(POST/PUT/DELETE/PATCH)生效
- 检查 `X-Requested-With: XMLHttpRequest` 或 `X-CSRF-Token` 头
- CORS 层不包含 `x-requested-with`，所以跨域请求无法设置该头
- 这是一个合理的 defense-in-depth 策略

**改进建议:**
- 渗透测试报告M1标记"缺少CSRF防护"不完全准确，实际已有基于自定义头的防护
- 但建议增加显式CSRF token生成/验证机制，而非仅依赖自定义头

### 3.4 文件上传安全 ✅ 良好（已修复历史漏洞）

**文件类型验证 (`media_service.rs:684-760`):**
- 使用 `infer` crate 的 magic bytes 验证，不信任客户端Content-Type
- 支持格式: mp4, m4v, mov, avi, mkv, webm, flv, wmv, jpg, jpeg, png, webp, gif, bmp
- **m3u8 已修复**: 渗透报告L2提到的"始终返回Ok()"已修复，现在:
  - 检查 `#EXTM3U` 头
  - 拒绝嵌入HTML/script标签
  - 限制文件大小1MiB
  - 拒绝 `<?xml`, `<!doctype`, `<html`, `javascript:` 等注入

**上传配额:**
- 默认50GB/用户，可通过 `UPLOAD_QUOTA_BYTES` 配置
- 临时文件(`.upload_*`) 24小时自动清扫
- 每小时执行一次清扫任务

**图片验证:**
- `infer_image()` 使用 magic bytes 验证头像上传

### 3.5 SQL注入防护 ✅ 优秀

- **全部使用 SQLx 参数化查询** — 188处 `sqlx::query` 调用，无字符串拼接SQL
- 所有用户输入通过 `.bind()` 参数化
- 搜索服务使用 `escape_like_pattern()` 转义 LIKE 通配符
- 数据库错误不泄露到客户端（`AuthError::Internal("database error")`）

### 3.6 速率限制 ⚠️ 有效但有局限

**当前实现:**
- 用户名维度: 60秒内3次尝试，超限后封锁10分钟
- IP维度: 60秒内30次尝试
- 基于 DashMap（内存），每5分钟清理过期条目
- 日志安全: `log_safe()` 过滤控制字符防日志注入

**局限性:**
- 内存存储，服务器重启后重置（渗透报告M3）
- 无Redis时可通过重启绕过
- 用户名维度限制可被已知用户名的攻击者利用进行账户DoS（代码注释已承认）

### 3.7 环境变量与密钥管理 ✅ 良好

**`.env.example` 分析:**
- `DATABASE_URL` — 有示例格式
- `HASHID_SALT` — 默认baked-in值，建议生产环境设置随机值
- `COOKIE_SECURE` — 默认true（.env.example确认）
- `ALLOW_FIRST_USER_ADMIN` — 默认false，需要显式opt-in
- `REGISTRATION_ENABLED` — 默认false
- `TRUSTED_PROXY` — 默认0（不信任任意代理）
- `APP_ENV` — 默认production

**密钥存储:**
- 无硬编码密钥或密码（渗透报告已确认）
- SMTP密码通过环境变量配置
- Sentry DSN通过环境变量

### 3.8 Docker安全

项目中**未发现 Dockerfile 或 docker-compose 文件**。部署可能通过其他方式（如直接二进制部署或外部Docker配置）。

### 3.9 媒体文件访问控制 ✅ 优秀

- 所有媒体文件通过 `media_auth` 中间件保护
- 播放需要活跃会话（120秒心跳超时）
- 分享token绑定到特定视频，不可跨视频使用
- 孤立文件（`.upload_*`临时文件、未注册文件）对已登录用户也拒绝
- 缩略图/封面图对已登录用户开放（低敏感度元数据）
- 头像(`/media/avatars/*`)公开访问（设计如此）

---

## 四、已验证的安全控制（渗透测试确认有效）

| 安全控制 | 状态 |
|---|---|
| SQL注入防护 | ✅ 参数化查询有效 |
| 路径遍历防护 | ✅ media_auth拦截 + Cloudflare WAF |
| 媒体文件未授权访问 | ✅ 返回401 |
| 管理接口未授权访问 | ✅ 返回401 |
| CORS跨域读取 | ✅ 仅允许配置的域名 |
| Cloudflare WAF旁路 | ✅ 非浏览器POST被拦截 |
| 注册绕过 | ✅ 返回"注册功能已关闭" |
| 点击劫持防护 | ✅ X-Frame-Options: DENY + CSP frame-ancestors |
| HSTS | ✅ 仅HTTPS时发送，防止降级攻击 |
| 密码哈希 | ✅ Argon2id |
| 请求超时 | ✅ 全局30秒（上传2小时） |

---

## 五、安全改进建议

### 优先级 P0（立即修复）

1. **认证 `/metrics`、`/health`、`/server/info` 端点**
   - 至少要求admin认证，或在生产环境完全禁用
   - 这是最大的信息泄露风险

2. **限制 `/docs/openapi.json` 访问**
   - 生产环境应要求admin认证或禁用

3. **前端token存储改为 httpOnly Cookie**
   - 当前 localStorage 存储使XSS可直接窃取token
   - 后端已支持Cookie认证，前端需切换

### 优先级 P1（尽快修复）

4. **评论/标签添加所有权检查** (M5)
   - 验证操作用户是否为视频所有者

6. **默认启用 `COOKIE_SECURE=true`**
   - .env.example 已标注默认true，确认代码默认值一致

7. **将速率限制迁移到Redis**
   - 已有 `REDIS_URL` 配置支持，建议生产环境必选

### 优先级 P2（计划修复）

8. **视频ID使用HashID混淆** (M2)
   - `HASHID_SALT` 已有配置支持，API层面使用hashid替代自增ID

9. **限制推荐API返回的数据量** (H2)
   - 不返回完整视频元数据，或要求认证

10. **前端错误日志清理** (L3)
    - 减少localStorage中的错误记录数量或内容

---

## 六、安全亮点

项目中值得肯定的安全实践:

1. **时序攻击防护**: 用户不存在时执行dummy argon2 verify
2. **日志注入防护**: 所有用户输入经过 `sanitize_for_log()` / `log_safe()` 过滤
3. **HSTS安全发送**: 仅在可信代理的HTTPS请求中发送，防止降级攻击
4. **Cloudflare IP验证**: 硬编码CF IP范围，防止伪造代理头
5. **Multi-tenant隔离**: Token绑定租户，防止跨租户访问
6. **全面的单元测试**: 安全中间件有完整的测试覆盖
7. **SQL错误封装**: 数据库错误不泄露到客户端
8. **上传文件安全**: Magic bytes验证 + 内容检查 + 配额限制 + 自动清理
9. **密码强度检查**: 要求字符类别多样性
10. **竞态条件处理**: 并发注册的唯一约束冲突优雅处理

---

## 七、结论

Atmos Video 的核心安全架构设计良好，尤其在认证、密码存储、SQL注入防护和安全头方面达到了较高标准。主要风险集中在**信息泄露**（公开的监控端点、API文档、视频元数据）和**前端token存储**。建议优先修复P0级别的3个问题，可显著提升整体安全水位。
