FROM rust:1.75-slim AS backend-builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir -p src benches && echo 'fn main() {}' > src/main.rs && echo 'fn main() {}' > benches/tenant_performance.rs && cargo build --release --locked && rm -rf src benches
COPY backend/src ./src
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

COPY --from=backend-builder /app/target/release/atmos-video /usr/local/bin/
COPY --from=frontend-builder /app/dist /var/lib/atmos/webapp

ENV WEBAPP_ROOT=/var/lib/atmos/webapp
ENV MEDIA_ROOT=/var/lib/atmos/media

EXPOSE 8082
CMD ["atmos-video"]
