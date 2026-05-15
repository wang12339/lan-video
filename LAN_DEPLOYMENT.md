# 局域网视频平台部署与联调

## 1. 启动后端（电脑）

### 方式 A：Docker（推荐）

```bash
docker compose up -d
```

自动启动后端 + PostgreSQL，监听 `0.0.0.0:8082`。

### 方式 B：Cargo 直连运行

```bash
# 需要先启动 PostgreSQL 并创建数据库
createdb lan_video

cd backend
cargo run
```

- 默认监听：`0.0.0.0:8082`
- 数据库：PostgreSQL（连接串见 `backend/.env`）
- 媒体目录：`backend/media/`

## 2. 导入视频数据

- 外链聚合：
```bash
curl -X POST http://127.0.0.1:8082/admin/videos/external \
  -H "Content-Type: application/json" \
  -d '{"title":"示例外链","description":"m3u8演示","streamUrl":"https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8","category":"demo"}'
```

- 本地上传：
```bash
curl -X POST "http://127.0.0.1:8082/admin/videos/upload?category=movie" \
  -F "file=@/absolute/path/sample.mp4"
```

- 扫描媒体目录：
```bash
curl -X POST "http://127.0.0.1:8082/admin/videos/scan?category=local"
```

## 3. 安卓端连接

1. 手机与电脑接入同一 Wi-Fi（或经以太网接入同一网段）。
2. App 启动后会**自动探活**已保存的地址；若无法访问，会在当前网段内扫描端口 8082 的 `GET /server/info` 以自动连接后端。
3. 可在「设置」中**手动填写** `http://<电脑局域网IP>:8082` 并保存。
4. 进入首页/搜索拉取视频并播放。

## 4. Web 端访问

浏览器打开 `http://<电脑局域网IP>:8082/webapp/`（Rust 后端已内置静态文件服务，无需额外启动任何服务）。
