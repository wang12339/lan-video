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

## Docker 镜像部署与回滚

CI 在 main 分支构建并推送 `ghcr.io/<owner>/<repo>:main`。服务器使用仓库根目录
的 `docker-compose.yml`（`app` 服务引用该镜像，`IMAGE_NAME` / `IMAGE_TAG` 可覆盖）：

```bash
docker compose pull app
docker compose up -d --wait --no-deps app   # 健康检查通过才返回
```

部署 job 升级前会把当前镜像标记为 `:previous`；若 `/health` 检查失败会执行
`IMAGE_TAG=previous docker compose up -d --no-deps app` 自动回滚。手动回滚同理。

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

---

## 中文全文搜索（可选 zhparser）

搜索服务启动后首次执行中文查询时，会一次性探测数据库是否提供
**由 zhparser 驱动的 `chinese` 文本搜索配置**（要求
`pg_ts_config.cfgname = 'chinese'` 且 `pg_ts_parser.prsname = 'zhparser'`），
结果在进程内缓存：

- **已启用 zhparser**：迁移 `060_zhparser_optional.sql` 创建 `chinese` 配置、
  `n,v,a,i,e,l` 映射与表达式 GIN 索引 `idx_videos_search_zhparser`；
  `full_text_search` 对含非 ASCII 的查询改用
  `to_tsvector('chinese', title || ' ' || COALESCE(description, ''))`
  原生分词，并与 pg_trgm 子串匹配取并集（命中率只增不减）。
  纯 ASCII 查询仍走 `search_vector` + `simple`，英文排序语义不变。
- **未安装 / 无权限**：迁移 060 只输出 `NOTICE`，不改动任何对象；
  中文查询保持既有的 pg_trgm `ILIKE` 回退，行为与升级前一致。

> `search_vector` 始终由触发器用 `simple` 维护（不随环境切换）：列表过滤
> 路径（`video_repo.rs`）硬编码了 `plainto_tsquery('simple', ...)`，切换词典
> 会导致该路径静默失配。中文原生分词通过查询时表达式实现。

## 安装 zhparser

zhparser 非标准 contrib，需要源码编译（依赖 SCWS）：

```bash
# 1) 编译依赖（Debian/Ubuntu + PGDG 的 PostgreSQL 16）
apt-get update && apt-get install -y postgresql-server-dev-16 build-essential git

# 2) 安装 SCWS
git clone --depth 1 https://github.com/hightman/scws.git /src/scws
cd /src/scws && ./autogen.sh && ./configure --prefix=/usr/local && make && make install

# 3) 安装 zhparser（make install 自动写入 PG 的 lib/extension 目录）
git clone --depth 1 https://github.com/amutu/zhparser.git /src/zhparser
cd /src/zhparser && SCWS_HOME=/usr/local make && make install

# 4) 重启 PostgreSQL
```

应用启动时迁移 060 会自动执行 `CREATE EXTENSION IF NOT EXISTS zhparser`。
zhparser 不是 trusted 扩展，若应用数据库角色无 `CREATE EXTENSION` 权限，
由管理员先手动执行一次 `CREATE EXTENSION IF NOT EXISTS zhparser;`，
再让应用跑迁移（或手动对目标库执行 `migrations/060_zhparser_optional.sql`）。

> 若库里已存在同名但不是 zhparser 的 `chinese` 配置（历史测试夹具可能用
> `CREATE TEXT SEARCH CONFIGURATION chinese (COPY = simple)` 建过占位配置），
> 迁移 060 会跳过创建并输出 NOTICE。确认无业务依赖后 `DROP TEXT SEARCH
> CONFIGURATION chinese;` 再重跑 060 即可启用原生分词。

### Docker 构建（基于 pgdg，仅示例，不要求提交镜像）

```dockerfile
FROM postgres:16-bookworm AS zhparser-builder
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential git ca-certificates postgresql-server-dev-16 \
 && rm -rf /var/lib/apt/lists/*
RUN git clone --depth 1 https://github.com/hightman/scws.git /src/scws \
 && cd /src/scws && ./autogen.sh && ./configure --prefix=/usr/local \
 && make -j"$(nproc)" && make install
RUN git clone --depth 1 https://github.com/amutu/zhparser.git /src/zhparser \
 && cd /src/zhparser && SCWS_HOME=/usr/local make -j"$(nproc)" && make install

FROM postgres:16-bookworm
COPY --from=zhparser-builder /usr/local/lib/ /usr/local/lib/
COPY --from=zhparser-builder /usr/lib/postgresql/16/lib/ /usr/lib/postgresql/16/lib/
COPY --from=zhparser-builder /usr/share/postgresql/16/extension/ \
     /usr/share/postgresql/16/extension/
```

### 验证

```sql
SELECT c.cfgname, p.prsname
FROM pg_ts_config c JOIN pg_ts_parser p ON p.oid = c.cfgparser
WHERE c.cfgname = 'chinese';          -- 期望 prsname = zhparser

SELECT to_tsvector('chinese', '中文分词测试');
```
