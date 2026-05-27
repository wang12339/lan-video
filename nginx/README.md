# nginx TLS 反向代理

## 快速启用 HTTPS

```bash
# 1. 生成 TLS 证书（需要先安装 mkcert: brew install mkcert）
cd nginx
mkcert -install
mkcert -cert-file certs/cert.pem -key-file certs/key.pem \
    localhost 192.168.1.x your-domain.com

# 2. 启动（带 TLS profile）
cd ..
docker compose --profile tls up -d
```

## 生产环境

替换为 Let's Encrypt 证书：

```bash
# 用 certbot 获取免费证书
docker run -it --rm -v ./nginx/certs:/etc/letsencrypt certbot/certbot \
    certonly --standalone -d your-domain.com

# 修改 atmos.conf 中的 ssl_certificate 路径指向 Let's Encrypt 证书
```

## 架构

```
客户端 ──HTTPS:443──▶ nginx ──HTTP:8082──▶ Rust 后端
                        │
                        ▼
                  PostgreSQL (端口 5432)
```
