# CLAUDE.md

本文件为 Claude Code (claude.ai/code) 在此仓库中操作时提供指引。

## 项目概览

Atmos Video — 局域网视频播放平台，两部分单仓：

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
```

### Web PWA (React)

```bash
cd webapp
npm install                          # 安装依赖
npm run dev                          # 开发服务器（localhost:5173）
npm run build                        # 构建生产版本到 dist/
```

构建后由后端在 `/webapp/` 路径提供静态文件服务。

## 架构

### 后端请求流程

```
main.rs → build_router(app.rs) → 中间件栈 → handlers → services → repositories → SQLx
```

- **入口**：`main.rs` 启动 Axum 服务、执行数据库迁移、启动后台任务
- **路由**：`app.rs` 组装路由组和中间件（认证、限流、CORS、追踪、请求 ID）
- **中间件**：`bearer_auth`（token/cookie）、`admin_auth`、`media_auth`（支持 `<video>` range 请求）、`rate_limit`（DashMap 原子操作）、`request_id`（X-Request-ID 追踪）
- **处理器**：提取状态/认证/参数，校验输入，委派给 service，返回 JSON
- **服务层**：业务逻辑（视频扫描、去重、缩略图生成、认证流程）
- **数据层**：SQLx 查询 PostgreSQL
- **状态**：`AppState`（Arc 包装）持有 repository、service、配置、限流器、缓存

### Web PWA 结构 (React)

React SPA，由 Vite 构建，后端提供静态文件服务。核心模块：

- `src/api/` — API 层（client.ts, auth.ts, videos.ts, playback.ts, utils.ts）
- `src/components/` — 共享组件（Layout, VideoCard, AuthDialog）
- `src/pages/` — 页面组件（Home, Player, Gallery, Upload, Profile）
- `src/context/` — 全局状态（AuthContext）
- `src/styles/` — CSS 变量和全局样式

## 数据库

PostgreSQL 16。迁移文件在 `backend/migrations/`，启动时由 `db.rs` 自动按序执行，通过 `_schema_migrations` 表跟踪。

主要表：`users` → `auth_tokens` / `videos` → `playback_history` / `user_likes` / `user_favorites`

迁移 011 添加了性能索引：`idx_videos_category`、`idx_videos_original_name_size`、`idx_videos_no_cover`（部分索引）、`idx_playback_history_user_updated`（覆盖索引）。

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `DATABASE_URL` | `postgres://kuaile@localhost:5432/lan_video` | PostgreSQL 连接串 |
| `SERVER_PORT` | `8082` | 后端监听端口 |
| `MEDIA_ROOT` | `./media` | 本地媒体文件存储 |
| `WEBA_ROOT` | `./webapp/dist` | Web PWA 构建输出目录 |
| `REGISTRATION_ENABLED` | `false` | 是否允许注册 |
| `CORS_ORIGIN` | `http://localhost:8082` | 允许的 CORS 来源 |
| `COOKIE_SECURE` | `false` | Cookie Secure 标志 |
| `DB_MAX_CONNECTIONS` | `10` | PostgreSQL 连接池大小 |
| `SENTRY_DSN` | （空） | Sentry 崩溃上报 DSN |
| `RUST_LOG` | `info` | 日志级别（tracing-subscriber EnvFilter） |

## CI/CD

GitHub Actions（`.github/workflows/ci.yml`）：
- **backend**：fmt → clippy → build → test（带 PostgreSQL 16 服务）

## 约定

- 后端：分层架构 — handler 不直接访问数据库，必须通过 service/repository
- 后端：`DashMap` 实现原子限流，`Moka` 缓存视频列表查询（10 秒 TTL）
- 后端：异步上下文中使用 `tokio::task::spawn_blocking` 处理文件系统操作
- 后端：认证 token 为 256 位 Alphanumeric（非 UUID），7 天过期
- Web：React 18 + TypeScript + Vite
- Web：所有用户可见文本为中文（zh-CN）
- 通用：每个 handler 都有输入校验（用户名 2-64 字符、密码 6-128 字符、标题 ≤500、批量操作 ≤1000）
