# Stage 1: Build webapp
FROM node:20-alpine AS webapp-builder
WORKDIR /webapp
COPY webapp/package.json webapp/package-lock.json ./
RUN npm ci
COPY webapp/ ./
RUN npm run build

# Stage 2: Build backend
FROM rust:1.81-slim-bookworm AS backend-builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --locked 2>/dev/null || true
COPY backend/ ./
RUN touch src/main.rs
RUN cargo build --release --locked

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libc6 ffmpeg && rm -rf /var/lib/apt/lists/*
COPY --from=backend-builder /app/target/release/lan-video-backend /app/lan-video-backend
COPY --from=webapp-builder /webapp/dist /app/webapp/dist
COPY backend/migrations /app/migrations
EXPOSE 8082
CMD ["/app/lan-video-backend"]
