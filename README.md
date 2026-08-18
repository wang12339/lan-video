# Atmos Video

视频分享与播放平台。支持视频上传、转码、分享、评论、收藏、播放列表、标签管理、多租户、邮箱验证/密码重置、热门推荐等功能。

> **注意**：尽管仓库名为 `atmos-android`，这**不是** Android 项目。它是一个两部分组成的单体仓库：Rust/Axum 后端 + React/Vite 前端。

## 技术栈

| 层      | 技术                                    |
| ------- | --------------------------------------- |
| 后端    | Rust, Axum, SQLx, PostgreSQL, FFmpeg    |
| 前端    | React 18, TypeScript, Vite, react-router-dom |
| 数据库  | PostgreSQL 16+                           |

## 快速开始

### 前置要求

- Rust 工具链 (1.81+)
- Node.js 20+
- PostgreSQL 16+
- FFmpeg (用于视频转码和缩略图)
- pkg-config + libssl (Linux)

### 1. 启动数据库

```bash
# macOS (Homebrew)
brew install postgresql@16
brew services start postgresql@16
createdb atmos_video

# 或使用 Docker
docker compose up -d
```

### 2. 启动后端

```bash
cd backend
cp .env.example .env  # 编辑 DATABASE_URL, PUBLIC_URL 等配置
cargo run --release   # 首次编译较慢，后续增量编译快
```

后端将在 `http://localhost:8082` 启动。迁移在启动时自动运行。

### 3. 启动前端（开发模式）

```bash
cd webapp
npm install
npm run dev          # 开发服务器在 localhost:5173
                     # 自动代理 /videos, /auth, /admin 等到 :8082
```

### 构建前端（生产）

```bash
cd webapp
npm run build        # tsc + vite build → dist/
```

前端构建产物由后端在 `/webapp/` 路径自动提供。

## 环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `DATABASE_URL` | `postgres://kuaile@localhost:5432/atmos_video` | 数据库连接 |
| `SERVER_PORT` | `8082` | 服务端口 |
| `PUBLIC_URL` | (必填) | 外部可访问的基 URL，用于分享链接、热链接防护和 HTTPS 重定向 |
| `MEDIA_ROOT` | `./media` | 媒体文件存储目录 |
| `WEBAPP_ROOT` | `./webapp/dist` | 前端构建产物目录（SPA 静态文件） |
| `LOG_DIR` | `./logs` | 日志目录 |
| `DATA_DIR` | `./data` | 数据目录 |
| `DB_MAX_CONNECTIONS` | `100` | PostgreSQL 连接池上限 |
| `MIGRATIONS_DIR` | `backend/migrations/` | 迁移目录覆盖（默认自动发现） |
| `REGISTRATION_ENABLED` | `false` | 是否允许公开注册 |
| `APP_ENV` | `production` | `production` 启用 HTTPS 重定向与严格 CORS；`development` 放宽 |
| `ALLOW_FIRST_USER_ADMIN` | `false` | 首个注册用户是否自动成为管理员（默认否） |
| `UPLOAD_QUOTA_BYTES` | `53687091200` | 单用户存储配额（50 GB），`0` 表示不限制 |
| `CORS_ORIGIN` | (空) | 允许的跨域来源（逗号分隔） |
| `COOKIE_SECURE` | `true` | 是否设置 Secure Cookie 标志 |
| `SMTP_HOST` 等 `SMTP_*` | (空) | SMTP 邮件服务（密码重置、邮箱验证必需） |
| `REDIS_URL` | (空) | 可选 Redis（未配置时回退到内存限流/缓存） |
| `TRUSTED_PROXY` | `0` | 是否信任 `X-Forwarded-For` / `cf-connecting-ip` |
| `HASHID_SALT` | 内置默认 | Hash ID 盐值，生产环境必须自定义 |
| `TRANSCODE_TIMEOUT_SECS` | `3600` | 单次 ffmpeg 转码超时（秒） |
| `FFPROBE_TIMEOUT_SECS` | `30` | 单次 ffprobe 超时（秒） |
| `TRANSCODE_CONCURRENCY` | `1` | 并发转码数 |
| `RUST_LOG` | `info` | 日志级别 |

完整变量列表见 `backend/.env.example`。

## 项目结构

```
backend/              # Rust/Axum 后端
  src/
    main.rs           # 入口
    app.rs            # 路由定义 + 中间件装配
    config.rs         # 环境变量配置
    db.rs             # 数据库连接 + 迁移
    state.rs          # AppState（Arc 包装，注入请求扩展）
    metrics.rs        # Prometheus 指标
    openapi.rs        # 手写的 OpenAPI 3.1 文档
    handlers/         # HTTP 处理器
    services/         # 业务逻辑层
    repositories/     # 数据访问层
    middleware/       # 认证、限流、热链接防护、租户解析、带宽限制等
    models/           # 数据模型
    util/             # 工具函数
  migrations/         # SQL 迁移文件（自动发现，共 40 个）
  tests/              # 单元测试 + 集成测试（需 DATABASE_URL）+ OpenAPI 一致性测试

webapp/               # React/Vite 前端
  src/
    api/              # API 客户端
    components/       # 通用组件
    pages/            # 页面（懒加载）
    context/          # React Context（Auth）
    hooks/            # 自定义 Hooks
    lib/              # 第三方库封装（queryClient 等）
    locales/          # i18n 语言包（zh-CN / en-US）
    utils/            # 工具函数
    styles/           # 全局样式
    test/             # Vitest 单元测试
```

## 测试

```bash
cd backend
cargo test                  # 单元测试 + 无需 DB 的测试
DATABASE_URL=... cargo test -- --test-threads=1   # 含集成测试（需要 PostgreSQL）
cargo test --test openapi_route_tests  # 路由与 OpenAPI 文档一致性测试（无需 DB）

cd webapp
npm test                    # Vitest 单元测试
```

## 贡献

1. 确保 `cargo fmt --check` 和 `cargo clippy -- -D warnings` 通过
2. 添加测试覆盖关键路径
3. 遵循分层架构：handler → service → repository
4. 所有用户可见文本为中文

## 安全

发现安全漏洞？请通过 Issues 报告，或联系维护者。

## 许可

MIT
