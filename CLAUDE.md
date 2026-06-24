# CLAUDE.md

本文件为 Claude Code (claude.ai/code) 在此仓库中操作时提供指引。

## 项目概览

Atmos Video — 局域网视频播放平台，三部分单仓：

| 组件 | 目录 | 技术栈 |
|------|------|--------|
| 后端 | `backend/` | Rust / Axum 0.8 / SQLx 0.8 / PostgreSQL 16 |
| Android | `app/` | Kotlin / Jetpack Compose / Material 3 / Koin DI |
| Web PWA | `webapp/`（软链接 → `~/视频网页/`） | 原生 JS / CSS |

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

### Android (Kotlin)

```bash
./gradlew assembleDebug testDebugUnitTest          # 构建 + 单元测试
./gradlew assembleRelease                          # Release 构建（需要 keystore.properties）
./gradlew testDebugUnitTest                        # 仅单元测试
./gradlew connectedDebugAndroidTest                # 仪器化测试（需要设备/模拟器）
```

### Web PWA

无构建步骤，文件由后端在 `/webapp/` 直接提供服务。

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

### Android 架构

MVVM + Jetpack Compose + Koin 依赖注入：

```
Screen（Composable）→ ViewModel → Repository → API（Retrofit）/ Room（离线缓存）
```

- **DI**：`di/AppModule.kt` — Koin 模块，注册所有 ViewModel 和 Repository
- **导航**：`ui/navigation/AppNavigation.kt` — Compose Navigation 图
- **主题**：`ui/theme/` — Kawaii 暗色主题（Color.kt、Theme.kt、Type.kt）
- **网络**：`data/network/NetworkModule.kt` — Retrofit/OkHttp 单例，`uploadClient` 4 小时超时（仅上传用）
- **离线**：`data/local/` — Room 数据库缓存视频列表
- **认证**：`data/user/AuthSessionStore.kt` — EncryptedSharedPreferences（缓存实例）

### Web PWA 结构

多页面应用，由后端提供服务。核心模块：

- `js/api.js` — fetch 封装、token 管理、结构化错误日志
- `js/cards.js` — 共享 `createVideoCard()` / `createImageCard()`（懒加载、WebP）
- `js/selection.js` — 共享多选模式（多选 + 批量删除）
- `js/dom.js` — 工具函数：`escape()`（regex）、`toast()`、`setupSearch()`、`trapFocus()`
- `css/atmos.css` — 源样式表；`css/atmos.min.css` — 压缩版（HTML 加载此文件）
- `sw.js` — Service Worker（静态资源缓存优先、API 网络优先）
- `manifest.json` — PWA 清单

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
| `WEBA_ROOT` | `./webapp` | Web PWA 源文件目录 |
| `REGISTRATION_ENABLED` | `false` | 是否允许注册 |
| `CORS_ORIGIN` | `http://localhost:8082` | 允许的 CORS 来源 |
| `COOKIE_SECURE` | `false` | Cookie Secure 标志 |
| `DB_MAX_CONNECTIONS` | `10` | PostgreSQL 连接池大小 |
| `SENTRY_DSN` | （空） | Sentry 崩溃上报 DSN |
| `RUST_LOG` | `info` | 日志级别（tracing-subscriber EnvFilter） |

## CI/CD

GitHub Actions（`.github/workflows/ci.yml`）：
- **backend**：fmt → clippy → build → test（带 PostgreSQL 16 服务）
- **android**：assembleDebug + testDebugUnitTest（JDK 17）
- **release**（`v*` 标签触发）：两个 job 通过 → 签名 APK → GitHub Release

## 约定

- 后端：分层架构 — handler 不直接访问数据库，必须通过 service/repository
- 后端：`DashMap` 实现原子限流，`Moka` 缓存视频列表查询（10 秒 TTL）
- 后端：异步上下文中使用 `tokio::task::spawn_blocking` 处理文件系统操作
- 后端：认证 token 为 256 位 Alphanumeric（非 UUID），7 天过期
- Android：所有 ViewModel 通过 Koin 构造注入，使用普通 `ViewModel()`（非 AndroidViewModel）
- Android：Room 数据库离线缓存视频列表，网络优先 + 缓存降级
- Android：`BuildConfig.DEBUG` 守卫所有 `android.util.Log` 调用
- Web：文件哈希使用 `crypto.subtle.digest('SHA-256')`（非 MD5）
- Web：所有用户可见文本为中文（zh-CN）
- Web：关键 CSS 内联在 HTML `<head>` 中，完整样式表通过 `media="print" onload` 懒加载
- 通用：每个 handler 都有输入校验（用户名 2-64 字符、密码 6-128 字符、标题 ≤500、批量操作 ≤1000）
