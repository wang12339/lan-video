# 部署指南

## 环境要求

- OS: Ubuntu 22.04 LTS
- Rust: 1.75+
- PostgreSQL: 16+
- Node.js: 18+
- FFmpeg: 6.0+
- Nginx: 1.24+

## 数据库设置

1. 创建数据库
```sql
CREATE DATABASE atmos_video;
CREATE USER atmos WITH PASSWORD 'your_password';
GRANT ALL PRIVILEGES ON DATABASE atmos_video TO atmos;
```

2. 运行迁移
```bash
cd backend
DATABASE_URL=postgres://atmos:your_password@localhost/atmos_video cargo run -- migrate
```

## 环境变量

创建 `.env` 文件：
```env
DATABASE_URL=postgres://atmos:password@localhost/atmos_video
SERVER_PORT=8082
PUBLIC_URL=https://your-domain.com
MEDIA_ROOT=/var/lib/atmos/media
WEBAPP_ROOT=/var/lib/atmos/webapp
REGISTRATION_ENABLED=true
APP_ENV=production
COOKIE_SECURE=true
```

## 构建步骤

1. 构建后端
```bash
cd backend
cargo build --release
```

2. 构建前端
```bash
cd webapp
npm install
npm run build
```

3. 部署文件
```bash
sudo mkdir -p /var/lib/atmos/{media,webapp}
sudo cp target/release/atmos-video /usr/local/bin/
sudo cp -r webapp/dist/* /var/lib/atmos/webapp/
```

## Nginx 配置

后端路由**没有 `/api` 前缀**（如 `/auth/login`、`/videos`、`/admin/...`、`/chat/...`、
`/media/...`、`/ws/chat`、`/health`），因此整站按原路径代理即可。

> `/media` 由后端 `media_auth` 中间件保护（会话/分享令牌、防盗链、限速），
> **不要**在 nginx 里 `alias` 静态媒体目录，否则会绕过鉴权。
> 完整可用配置（上传限流、WebSocket、缓存与安全头）见仓库 `nginx/nginx.conf`。

```nginx
server {
    listen 443 ssl;
    server_name your-domain.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://127.0.0.1:8082;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # 聊天 WebSocket 长连接需要 Upgrade 头
    location /ws/ {
        proxy_pass http://127.0.0.1:8082;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_read_timeout 3600s;
    }
}
```

Nginx 反代后建议启用 `TRUSTED_PROXY=1` 并把反代对端 IP 加入
`TRUSTED_PROXY_PEERS`（Docker 中后端看到的通常是网桥网关，如 `172.17.0.1`），
否则限流/审计记录的客户端 IP 会退化为代理地址。

## 监控

1. 健康检查：GET /health
2. 日志：/var/log/atmos/
3. 建议使用 Prometheus + Grafana

## 备份

```bash
# 数据库备份
pg_dump atmos_video > backup_$(date +%Y%m%d).sql

# 媒体文件备份
rsync -av /var/lib/atmos/media/ /backup/media/
```

## 故障排查

1. 检查日志：journalctl -u atmos-video
2. 检查数据库连接：psql $DATABASE_URL
3. 检查端口：netstat -tlnp | grep 8082