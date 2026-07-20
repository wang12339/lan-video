# Atmos Video

局域网视频播放平台。支持视频上传、转码、分享、评论、收藏等功能，适用于家庭或办公室局域网内的多媒体共享。

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
createdb lan_video

# 或使用 Docker
docker compose up -d
```

### 2. 启动后端

```bash
cd backend
cp .env.example .env  # 编辑 DATABASE_URL 等配置
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
| `DATABASE_URL` | `postgres://kuaile@localhost:5432/lan_video` | 数据库连接 |
| `SERVER_PORT` | `8082` | 服务端口 |
| `PUBLIC_URL` | (空) | 外部可访问的基 URL，用于分享链接和热链接防护 |
| `MEDIA_ROOT` | `./media` | 媒体文件存储目录 |
| `REGISTRATION_ENABLED` | `false` | 是否允许公开注册 |
| `CORS_ORIGIN` | `http://localhost:8082` | 允许的跨域来源 |
| `RUST_LOG` | `info` | 日志级别 |
| `COOKIE_SECURE` | `false` | 是否设置 Secure Cookie 标志 |

完整变量列表见 `backend/.env.example`。

## 项目结构

```
backend/              # Rust/Axum 后端
  src/
    main.rs           # 入口
    app.rs            # 路由定义
    config.rs         # 环境变量配置
    db.rs             # 数据库连接 + 迁移
    handlers/         # HTTP 处理器
    services/         # 业务逻辑层
    repositories/     # 数据访问层
    middleware/       # 认证、限流、热链接防护等中间件
    models/           # 数据模型
    util/             # 工具函数
  migrations/         # SQL 迁移文件（自动发现）
  tests/              # 测试

webapp/               # React/Vite 前端
  src/
    api/              # API 客户端
    components/       # 通用组件
    pages/            # 页面（懒加载）
    context/          # React Context（Auth）
    styles/           # 全局样式
    test/             # 测试
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
