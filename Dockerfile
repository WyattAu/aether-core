# syntax=docker/dockerfile:1
#
# Aether Container Image
#
# Multi-stage build producing a minimal runtime image with
# aether-server and aether-cli binaries.
#
# For production deployment, see: docs/deployment_guide.md
#

# ============================================
# Stage 1: Build
# ============================================
FROM rustlang/rust:nightly-2026-03-01-slim AS builder

WORKDIR /build

# Install build dependencies (libssl for native-tls, libsqlite3 for rusqlite)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/
COPY crates/cli/Cargo.toml crates/cli/
COPY crates/server/Cargo.toml crates/server/
COPY crates/actor-sdk/Cargo.toml crates/actor-sdk/
COPY tests/Cargo.toml tests/

# Create stub sources to pre-build dependencies (cached layer)
RUN mkdir -p crates/core/src && echo "" > crates/core/src/lib.rs
RUN mkdir -p crates/actor-sdk/src && echo "" > crates/actor-sdk/src/lib.rs
RUN mkdir -p crates/server/src && echo "fn main() {}" > crates/server/src/main.rs
RUN mkdir -p crates/cli/src && echo "fn main() {}" > crates/cli/src/main.rs
RUN mkdir -p tests && echo "" > tests/lib.rs || true

# Pre-build dependencies (cached unless Cargo.toml/lock changes)
RUN cargo build --release 2>/dev/null || true

# Copy actual source code
COPY crates/ crates/
COPY tests/ tests/

# Touch entry points to invalidate the stub-only cache
RUN touch crates/core/src/lib.rs crates/actor-sdk/src/lib.rs \
    crates/server/src/main.rs crates/cli/src/main.rs

# Build the actual binaries
RUN cargo build --release --package aether-server --package aether-cli

# ============================================
# Stage 2: Runtime Image
# ============================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r aether && useradd -r -g aether -d /app aether

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /build/target/release/aether-server /usr/local/bin/
COPY --from=builder /build/target/release/aether-cli /usr/local/bin/

# Create data and log directories
RUN mkdir -p /var/lib/aether /var/log/aether /etc/aether && \
    chown -R aether:aether /app /var/lib/aether /var/log/aether /etc/aether

USER aether

# Expose ports: REST API (8080), gRPC (50051), Cluster Gossip (7946)
EXPOSE 8080 50051 7946

# Health check (adjust endpoint to match aether-server's actual health route)
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Environment
ENV AETHER_STATE_BACKEND=memory
ENV AETHER_LOG_LEVEL=info
ENV RUST_LOG=info

# Default entrypoint: aether-server
CMD ["aether-server"]
