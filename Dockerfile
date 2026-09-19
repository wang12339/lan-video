FROM rust:1.95-slim AS backend-builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir -p src benches && echo 'fn main() {}' > src/main.rs && cargo build --release --locked && rm -rf src benches
COPY backend/src ./src
COPY backend/benches ./benches
COPY backend/templates ./templates
RUN touch src/main.rs && cargo build --release --locked

FROM node:22-slim AS frontend-builder
WORKDIR /app
COPY webapp/package.json webapp/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY webapp/ .
RUN npm run build

FROM debian:trixie-slim
# curl 供 HEALTHCHECK 使用；ffmpeg 供转码
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3t64 \
    ffmpeg \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 非 root 运行账号；media/webapp 与工作目录归其所有，保证运行时可写 media
RUN groupadd --system atmos \
    && useradd --system --gid atmos --home-dir /app --shell /usr/sbin/nologin atmos \
    && mkdir -p /var/lib/atmos/media /var/lib/atmos/webapp /app

COPY --from=backend-builder /app/target/release/atmos-video-backend /usr/local/bin/
COPY --from=frontend-builder /app/dist /var/lib/atmos/webapp

# db.rs 运行时从 CARGO_MANIFEST_DIR(烧录为 /app)/migrations 自动发现迁移
WORKDIR /app
COPY backend/migrations /app/migrations

RUN chown -R atmos:atmos /var/lib/atmos /app

ENV WEBAPP_ROOT=/var/lib/atmos/webapp
ENV MEDIA_ROOT=/var/lib/atmos/media

EXPOSE 8082

HEALTHCHECK --interval=30s --timeout=5s --start-period=40s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8082/health || exit 1

USER atmos
CMD ["atmos-video-backend"]
