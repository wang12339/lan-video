# Atmos Video

视频分享与播放平台

> **注意**：尽管仓库名为 `atmos-android`，这**不是** Android 项目。它是一个两部分组成的单体仓库：Rust/Axum 后端 + React/Vite 前端。

## ✨ 功能特性

- 🎬 **视频上传与管理** - 支持多种格式，自动转码为 HLS 自适应码率
- 📺 **HLS 自适应播放** - 根据网络状况自动切换画质
- 🔍 **全文搜索与推荐** - 基于 PostgreSQL 的全文搜索和智能推荐
- 📋 **播放列表管理** - 创建、编辑和分享播放列表
- 🔗 **分享链接生成** - 一键生成分享链接，支持密码保护和过期时间
- 👥 **多租户支持** - 完整的多租户架构
- 🔒 **完善的权限控制** - 基于角色的访问控制（RBAC）
- 💬 **评论系统** - 视频评论与互动
- 🏷️ **标签管理** - 灵活的视频标签系统
- ❤️ **收藏与点赞** - 个人收藏夹和点赞功能
- 📧 **邮箱验证** - 注册邮箱验证和密码重置
- 📊 **管理后台** - 完整的管理界面
- 🚀 **高性能** - Rust 后端 + Moka 缓存 + Redis 可选

## 🚀 快速开始

### 环境要求

- **Rust** 1.81+
- **PostgreSQL** 16+
- **Node.js** 20+
- **FFmpeg** - 视频转码和缩略图生成
- **pkg-config + libssl** - Linux 系统需要

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

后端将在 `http://localhost:8082` 启动。数据库迁移在启动时自动运行。

### 3. 启动前端（开发模式）

```bash
cd webapp
npm install
npm run dev          # 开发服务器在 localhost:5173
                     # 自动代理 /videos, /auth, /admin 等到 :8082
```

### 4. 构建前端（生产）

```bash
cd webapp
npm run build        # tsc + vite build → dist/
```

前端构建产物由后端在 `/webapp/` 路径自动提供静态文件服务。

## 🏗️ 架构

```
┌─────────────┐     ┌─────────────────────────────────────┐
│   Web App   │────▶│           Backend (Axum)             │
│  (React)    │     │  handlers → services → repositories  │
└─────────────┘     └──────────────────┬──────────────────┘
                                       │
                              ┌────────┴────────┐
                              │   PostgreSQL 16  │
                              └─────────────────┘
```

### 后端请求流程

```
main.rs → build_router(app.rs) → 中间件栈 → handlers → services → repositories → SQLx
```

**中间件执行顺序**（由外到内）：
- 全局：`security_headers` → `inject_state` → `resolve_tenant` → `request_id` → `request_log` → `TraceLayer` → `CompressionLayer` → CORS
- 路由级：`bearer_auth` → `role_auth(N)` → `admin_auth` → `rate_limit` → `hotlink_guard` → `bandwidth_throttle`

**分层规则**：handler 不直接访问数据库，必须通过 service → repository。

### 前端架构

React 18 SPA，由 Vite 构建，后端提供静态文件服务。

- **路由**：懒加载页面组件
- **状态管理**：React Context (AuthContext)
- **数据获取**：自定义 Hooks + React Query
- **国际化**：中文 (zh-CN) / 英文 (en-US)

## 📁 项目结构

```
backend/                    # Rust/Axum 后端
├── src/
│   ├── main.rs             # 入口
│   ├── app.rs              # 路由定义 + 中间件装配
│   ├── config.rs           # 环境变量配置
│   ├── db.rs               # 数据库连接 + 迁移
│   ├── state.rs            # AppState（Arc 包装）
│   ├── openapi.rs          # OpenAPI 3.1 文档
│   ├── handlers/           # HTTP 处理器
│   ├── services/           # 业务逻辑层
│   ├── repositories/       # 数据访问层
│   ├── middleware/          # 认证、限流、热链接防护等
│   ├── models/             # 数据模型
│   └── util/               # 工具函数
├── migrations/             # SQL 迁移文件（49 个，自动发现）
└── tests/                  # 单元测试 + 集成测试 + OpenAPI 一致性测试

webapp/                     # React/Vite 前端
└── src/
    ├── api/                # API 客户端
    ├── components/         # 通用组件
    ├── pages/              # 页面（懒加载）
    ├── context/            # React Context
    ├── hooks/              # 自定义 Hooks
    ├── locales/            # i18n 语言包
    ├── styles/             # 全局样式
    └── test/               # Vitest 单元测试
```

## 🔧 环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `DATABASE_URL` | `postgres://kuaile@localhost:5432/atmos_video` | 数据库连接 |
| `SERVER_PORT` | `8082` | 服务端口 |
| `PUBLIC_URL` | (必填) | 外部可访问的基 URL，用于分享链接、热链接防护和 HTTPS 重定向 |
| `MEDIA_ROOT` | `./media` | 媒体文件存储目录 |
| `WEBAPP_ROOT` | `./webapp/dist` | 前端构建产物目录 |
| `LOG_DIR` | `./logs` | 日志目录 |
| `DATA_DIR` | `./data` | 数据目录 |
| `DB_MAX_CONNECTIONS` | `100` | PostgreSQL 连接池上限 |
| `MIGRATIONS_DIR` | `backend/migrations/` | 迁移目录覆盖（默认自动发现） |
| `REGISTRATION_ENABLED` | `false` | 是否允许公开注册 |
| `APP_ENV` | `production` | `production` 启用 HTTPS 重定向与严格 CORS；`development` 放宽 |
| `ALLOW_FIRST_USER_ADMIN` | `false` | 首个注册用户是否自动成为管理员 |
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

## 🧪 测试

### 后端测试

```bash
cd backend

# 单元测试（无需数据库）
cargo test

# 含集成测试（需要 PostgreSQL）
DATABASE_URL=postgres://user:pass@localhost/atmos_video cargo test -- --test-threads=1

# 路由与 OpenAPI 文档一致性测试（无需数据库）
cargo test --test openapi_route_tests

# 格式检查和 lint
cargo fmt --check
cargo clippy -- -D warnings
```

### 前端测试

```bash
cd webapp

# Vitest 单元测试
npm test

# 类型检查
npm run build  # 包含 tsc 类型检查
```

## 📚 文档

- [架构文档](docs/ARCHITECTURE.md) - 系统架构详细说明
- [API 文档](docs/API.md) - REST API 参考
- [部署指南](docs/DEPLOYMENT.md) - 生产环境部署
- [贡献指南](CONTRIBUTING.md) - 如何参与贡献

## 🤝 贡献

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 确保代码质量：
   - `cargo fmt --check` - 代码格式
   - `cargo clippy -- -D warnings` - lint 检查
   - 添加测试覆盖关键路径
4. 遵循分层架构：handler → service → repository
5. 所有用户可见文本使用中文
6. 提交 Pull Request

## 🔒 安全

发现安全漏洞？请通过 [Issues](../../issues) 报告，或联系维护者。我们会在 48 小时内响应。

## 📄 许可证

本项目采用 [MIT License](LICENSE) 开源许可证。

## 🙏 致谢

感谢所有贡献者和以下开源项目：

- [Axum](https://github.com/tokio-rs/axum) - Rust Web 框架
- [React](https://react.dev/) - 用户界面库
- [PostgreSQL](https://www.postgresql.org/) - 数据库
- [FFmpeg](https://ffmpeg.org/) - 视频处理
