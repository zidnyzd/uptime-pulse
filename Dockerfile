# Multi-stage Dockerfile for UptimePulse (Multi-arch: linux/amd64 & linux/arm64)
# Ultra-lightweight final image (~15MB) based on Alpine Linux

# --- Build Stage ---
FROM rust:alpine AS builder

WORKDIR /app

# Install build dependencies required for bundled rusqlite (C-compiler) and cmake
RUN apk add --no-cache musl-dev gcc make cmake perl

# Copy manifest files first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy source code and embedded web assets
COPY src/ ./src/
COPY public/ ./public/

# Build optimized release binary
RUN cargo build --release --locked && \
    strip target/release/uptime-pulse

# --- Final Runtime Stage ---
FROM alpine:3.21

# Install CA certificates, timezone data, and ping utility (iputils-ping provides reliable RTT statistics)
RUN apk add --no-cache ca-certificates tzdata iputils-ping libcap && \
    setcap cap_net_raw+ep /bin/ping 2>/dev/null || true

WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/uptime-pulse /app/uptime-pulse

# Setup persistent data directory for SQLite database
RUN mkdir -p /data

# Default runtime configuration
ENV UPTIME_HOST=0.0.0.0 \
    UPTIME_PORT=3001 \
    UPTIME_DB_PATH=/data/uptime.db \
    UPTIME_RETENTION_DAYS=90

EXPOSE 3001
VOLUME ["/data"]

ENTRYPOINT ["/app/uptime-pulse"]
