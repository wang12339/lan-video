# syntax=docker/dockerfile:1
#
# Atmos Video — 多阶段构建
#   Stage 1 webapp-builder : node:20-alpine  → npm ci + vite build → dist/
#   Stage 2 backend-builder: rust:1.95-slim  → cargo build --release → 二进制
#   Stage 3 runtime        : debian:bookworm-slim + ffmpeg + 非 root 用户

# ── Stage 1: 构建前端 ─────────────────────────────────────────────────
FROM node:20-alpine AS webapp-builder
WORKDIR /webapp
COPY webapp/package.json webapp/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY webapp/ ./
RUN npm run build

# ── Stage 2: 构建后端 ─────────────────────────────────────────────────
FROM rust:1.95-slim-bookworm AS backend-builder
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# 先只拷依赖清单，预编译依赖层（cargo 层缓存）：无 lib.rs 时 dummy main 也可成功
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
 && cargo build --release --locked || true
COPY backend/ ./
# touch 强制 cargo 识别复制进来的真实源码（mtime 机制）
RUN touch src/main.rs \
 && cargo build --release --locked

# ── Stage 3: 运行时 ───────────────────────────────────────────────────
FROM debian:bookworm-slim
# ffmpeg 提供 libx264/aac 编码器（transcoder.rs 硬编码 -c:v libx264 -c:a aac）
# 及 ffprobe（媒体探测/缩略图）；curl 用于健康检查；tini 用于 PID1 信号/僵尸回收
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates \
      curl \
      ffmpeg \
      tini \
 && rm -rf /var/lib/apt/lists/*

# 非 root 运行（安全审查：避免 root 权限）
RUN groupadd --system --gid 10001 atmos \
 && useradd --system --uid 10001 --gid atmos --home-dir /app --no-create-home --shell /usr/sbin/nologin atmos

WORKDIR /app
COPY --from=backend-builder /app/target/release/atmos-video-backend /app/atmos-video-backend
COPY --from=webapp-builder /webapp/dist /app/webapp/dist
COPY backend/migrations /app/migrations

# 运行时目录：media/logs/data 默认相对 WORKDIR 解析（./media 等），先建好并授权
RUN mkdir -p /app/media /app/logs /app/data \
 && chown -R atmos:atmos /app

USER atmos
EXPOSE 8082

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8082/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/app/atmos-video-backend"]
