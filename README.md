# LAN Video - 局域网视频播放平台

全栈局域网视频播放方案，支持手机 App 和浏览器访问。

## 项目结构

```
lan-video/
├── backend/          # Rust (Actix-Web) 后端
│   ├── src/          # 源代码
│   ├── migrations/   # 数据库迁移
│   └── run_backend.command  # 一键启动脚本 (macOS)
├── app/              # Android App (Kotlin)
└── webapp/           # Web 前端 PWA
```

## 快速启动

### 前置条件

- [Rust](https://rustup.rs/)
- [PostgreSQL](https://postgresql.org/) (>= 16)

### 1. 配置数据库

```bash
createdb lan_video
```

### 2. 配置环境变量

```bash
cp backend/.env.example backend/.env
# 编辑 .env 填写数据库连接信息
```

### 3. 启动

**macOS**: 双击 `backend/run_backend.command`

**手动启动**:
```bash
cd backend
cargo run
```

### 4. 访问

- **Web 前端**: http://localhost:8082/webapp/
- **API 健康检查**: http://localhost:8082/health
- **Android App**: 连接 `http://<电脑IP>:8082`

首次使用时需要在 App 注册第一个账号，该账号自动成为管理员。

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | Rust, Actix-Web, SQLx, PostgreSQL |
| Android | Kotlin, Retrofit, ExoPlayer, Coil |
| Web | 原生 JS PWA, Service Worker |

## Docker 部署

```bash
docker compose up -d
```

默认监听 `0.0.0.0:8082`，详见 `docker-compose.yml`。

## API 概览

| 端点 | 说明 |
|------|------|
| `GET /health` | 健康检查 |
| `GET /server/info` | 服务器信息 |
| `POST /auth/register` | 注册 |
| `POST /auth/login` | 登录 |
| `GET /videos` | 视频列表(分页) |
| `GET /playback/history/{id}` | 播放进度 |
| `POST /admin/videos/upload` | 上传视频 |
