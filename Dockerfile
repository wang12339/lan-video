FROM rust:1.95-slim AS backend-builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir -p src benches && echo 'fn main() {}' > src/main.rs && echo 'fn main() {}' > benches/tenant_performance.rs && cargo build --release --locked && rm -rf src benches
COPY backend/src ./src
COPY backend/benches ./benches
COPY backend/templates ./templates
RUN touch src/main.rs && cargo build --release --locked

FROM node:20-slim AS frontend-builder
WORKDIR /app
COPY webapp/package.json webapp/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY webapp/ .
RUN npm run build

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    ffmpeg \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=backend-builder /app/target/release/atmos-video-backend /usr/local/bin/
COPY --from=frontend-builder /app/dist /var/lib/atmos/webapp

# db.rs 运行时从 CARGO_MANIFEST_DIR(烧录为 /app)/migrations 自动发现迁移
WORKDIR /app
COPY backend/migrations /app/migrations

ENV WEBAPP_ROOT=/var/lib/atmos/webapp
ENV MEDIA_ROOT=/var/lib/atmos/media

EXPOSE 8082
CMD ["atmos-video-backend"]
