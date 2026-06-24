# ===== Build Stage =====
FROM rust:bookworm AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY backend/ ./backend/

WORKDIR /app/backend
RUN cargo build --release && \
    cp target/release/lan-video-backend /app/lan-video-backend

# ===== Runtime Stage =====
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 libpq5 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/lan-video-backend /app/

EXPOSE 8082
CMD ["/app/lan-video-backend"]
