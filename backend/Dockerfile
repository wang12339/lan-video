FROM rust:1.85-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates ffmpeg curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/lan-video-backend /usr/local/bin/
RUN mkdir -p /data/media /data/data && chmod 777 /data/media /data/data
USER nobody
ENV MEDIA_ROOT=/data/media
EXPOSE 8080
ENTRYPOINT ["lan-video-backend"]
