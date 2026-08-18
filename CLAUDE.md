# CLAUDE.md

本文件为 Claude Code (claude.ai/code) 在此仓库中操作时提供指引。

## 项目概览

Atmos Video — 视频分享与播放平台，两部分单仓：

| 组件 | 目录 | 技术栈 |
|------|------|--------|
| 后端 | `backend/` | Rust / Axum 0.8 / SQLx 0.8 / PostgreSQL 16 |
| Web PWA | `webapp/` | React 18 / TypeScript / Vite |

## 构建与测试

### 后端 (Rust)

```bash
cd backend
cargo fmt --check                    # 格式检查
cargo clippy -- -D warnings          # lint（警告即错误）
cargo build --release                # 构建
cargo test                           # 单元测试（无需数据库）
DATABASE_URL=... cargo test          # 含集成测试（需要 PostgreSQL）
cargo test -- --test-threads=1       # 串行运行（数据库测试用）
cargo test --test openapi_route_tests  # 路由与 OpenAPI 文档一致性测试（无需数据库）
```

### Web PWA (React)

```bash
cd webapp
npm install                          # 安装依赖
npm run dev                          # 开发服务器（localhost:5173）
npm run build                        # 构建生产版本到 dist/
npm test                             # Vitest 单元测试（src/test/）
```

构建后由后端在 `/webapp/` 路径提供静态文件服务。

## 架构

### 后端请求流程

```
main.rs → build_router(app.rs) → 中间件栈 → handlers → services → repositories → SQLx
```

- **入口**：`main.rs` 启动 Axum 服务、执行数据库迁移、启动后台任务
- **路由**：`app.rs` 组装路由组和中间件（认证、限流、CORS、追踪、请求 ID、租户解析）
- **中间件**：全局（由外到内）`security_headers` → `inject_state` → `resolve_tenant`（租户解析）→ `request_id`（X-Request-ID 追踪）→ `request_log` → `TraceLayer` → `CompressionLayer` → CORS；路由级：`bearer_auth`（token/cookie）、`role_auth(N)`、`admin_auth`、`media_auth`（支持 `<video>` range 请求）、`rate_limit`（DashMap 原子操作）、`share_rate_limit`、`hotlink_guard`（热链接防护）、`bandwidth_throttle`（带宽限制）
- **处理器**：提取状态/认证/参数，校验输入，委派给 service，返回 JSON
- **服务层**：业务逻辑（视频扫描、去重、缩略图生成、认证流程）
- **数据层**：SQLx 查询 PostgreSQL
- **状态**：`AppState`（Arc 包装）持有 repository、service、配置、限流器、缓存

### Web PWA 结构 (React)

React SPA，由 Vite 构建，后端提供静态文件服务。核心模块：

- `src/api/` — API 层（client.ts, auth.ts, videos.ts, playback.ts, comments.ts, tags.ts, playlists.ts, shares.ts, recommendations.ts, admin.ts, logs.ts, prefs.ts）
- `src/components/` — 共享组件（Layout, VideoCard, AuthDialog, Comments, Toast）
- `src/pages/` — 页面组件（Home, Player, Gallery, Upload, Profile）
- `src/context/` — 全局状态（AuthContext）
- `src/hooks/` — 自定义 Hooks（useAsyncData）
- `src/lib/` — 第三方库封装（queryClient）
- `src/locales/` — i18n 语言包（zh-CN / en-US）
- `src/styles/` — CSS 变量和全局样式
- `src/test/` — Vitest 单元测试

## 数据库

PostgreSQL 16。迁移文件在 `backend/migrations/`（当前 40 个），启动时由 `db.rs` 自动按序执行，通过 `_schema_migrations` 表跟踪。

主要表：`users` → `auth_tokens` / `videos` → `playback_history` / `user_likes` / `user_favorites`，以及 `tenants`（多租户）、`password_reset_tokens`、`email_verification_tokens`、`comments`、`playlists`、`share_links`、`tags` 等。

迁移 011 添加了性能索引：`idx_videos_category`、`idx_videos_original_name_size`、`idx_videos_no_cover`（部分索引）、`idx_playback_history_user_updated`（覆盖索引）。

迁移 039 `fix_search_dictionary_and_backfill`：全文搜索触发器与查询端统一为 `simple` 词典并回填 `search_vector`，回填 `auth_tokens.tenant_id`，补充 `videos(created_at)` 等索引。

迁移 040 `search_suggest_trgm_and_recommendation_views`：搜索建议加 `pg_trgm` GIN 索引（`title gin_trgm_ops`）与 `videos(views DESC, id DESC)` 索引，支撑两段式推荐与默认排序。

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `DATABASE_URL` | `postgres://kuaile@localhost:5432/atmos_video` | PostgreSQL 连接串 |
| `SERVER_PORT` | `8082` | 后端监听端口 |
| `MEDIA_ROOT` | `./media` | 本地媒体文件存储 |
| `WEBAPP_ROOT` | `./webapp/dist` | Web PWA 构建输出目录 |
| `LOG_DIR` | `./logs` | 日志目录 |
| `DATA_DIR` | `./data` | 数据目录 |
| `REGISTRATION_ENABLED` | `false` | 是否允许注册 |
| `APP_ENV` | `production` | `production` 启用 HTTPS 重定向 + 严格 CORS；`development` 放宽 |
| `UPLOAD_QUOTA_BYTES` | `53687091200` | 单用户存储配额（50 GB），`0` 禁用 |
| `ALLOW_FIRST_USER_ADMIN` | `false` | 首个注册用户是否自动成为管理员 |
| `CORS_ORIGIN` | (空) | 允许的 CORS 来源（逗号分隔） |
| `COOKIE_SECURE` | `true` | Cookie Secure 标志 |
| `SMTP_HOST` 等 `SMTP_*` | (空) | 密码重置 / 邮箱验证必需 |
| `REDIS_URL` | (空) | 可选 Redis，缺省回退内存限流/缓存 |
| `TRUSTED_PROXY` | `0` | 是否信任 `X-Forwarded-For` / `cf-connecting-ip` |
| `HASHID_SALT` | 内置默认 | Hash ID 盐值，生产环境必须自定义 |
| `DB_MAX_CONNECTIONS` | `100` | PostgreSQL 连接池大小 |
| `SENTRY_DSN` | （空） | Sentry 崩溃上报 DSN |
| `RUST_LOG` | `info` | 日志级别（tracing-subscriber EnvFilter） |

## CI/CD

GitHub Actions（`.github/workflows/ci.yml`）：
- **backend**：fmt → clippy → build → test（PostgreSQL 16 服务容器，`--test-threads=1`）
- **frontend**：tsc 类型检查 → Vitest → 构建
- **audit / deny**：cargo-audit 依赖漏洞审计、cargo-deny 许可证策略

## 约定

- 后端：分层架构 — handler 不直接访问数据库，必须通过 service/repository
- 后端：`DashMap` 实现原子限流，`Moka` 缓存视频列表查询（60 秒 TTL）与推荐结果（120 秒 TTL）
- 后端：异步上下文中使用 `tokio::task::spawn_blocking` 处理文件系统操作
- 后端：认证 token 为 256 位 Alphanumeric（非 UUID），7 天过期
- 后端：**改 `app.rs` 路由必须同步更新 `src/openapi.rs` 与 `tests/openapi_route_tests.rs` 的 `registered_routes()`**（双向一致性测试）
- 后端：上传与管理路由使用 2 小时超时，其余路由 30 秒
- 后端：`backend/tests/` 下的 HTTP 集成测试（`http_integration.rs`、`integration_auth.rs`、`integration_videos.rs`）需 `DATABASE_URL`，未设置时静默跳过
- Web：React 18 + TypeScript + Vite
- Web：所有用户可见文本为中文（zh-CN），i18n 语言包在 `src/locales/`
- 通用：每个 handler 都有输入校验（用户名 2-64 字符、密码 6-128 字符、标题 ≤500、批量操作 ≤1000）
