# 阶段1: 构建Rust后端
FROM rust:1.75 as backend-builder
WORKDIR /app
COPY backend/ .
RUN cargo build --release --locked

# 阶段2: 构建前端
FROM node:20 as frontend-builder
WORKDIR /app
COPY webapp/ .
RUN npm ci && npm run build

# 阶段3: 运行时
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl3 \
    ffmpeg \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=backend-builder /app/target/release/atmos-video /usr/local/bin/
COPY --from=frontend-builder /app/dist /var/lib/atmos/webapp

ENV WEBAPP_ROOT=/var/lib/atmos/webapp
ENV MEDIA_ROOT=/var/lib/atmos/media

EXPOSE 8082
CMD ["atmos-video"]
