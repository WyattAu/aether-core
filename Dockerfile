# syntax=docker/dockerfile:1
# 
# Aether Container Image
#
# PURPOSE:
#   - Development and testing environments
#   - CI/CD pipeline builds
#   - Kubernetes transitional deployments
#
# NOT INTENDED FOR:
#   - Production bare-metal deployments (compile from source instead)
#   - Maximum performance scenarios
#   - Direct hardware access (use native deployment)
#
# For production deployment, see: .docs/deployment_guide.md
#
# ============================================
# Stage 1: Build Dependencies (Cached)
# ============================================
FROM rust:1.85-slim AS chef
RUN cargo install cargo-chef
WORKDIR /app

# ============================================
# Stage 2: Analyze Dependencies
# ============================================
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ============================================
# Stage 3: Build Dependencies
# ============================================
FROM chef AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Build dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release --bin aether

# ============================================
# Stage 4: Runtime Image
# ============================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 aether

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/aether /usr/local/bin/aether

# Copy default config
COPY aether.toml /etc/aether/aether.toml

# Create directories
RUN mkdir -p /var/lib/aether /var/log/aether && \
    chown -R aether:aether /var/lib/aether /var/log/aether /etc/aether

USER aether

# Expose ports
EXPOSE 9000/udp 9001/tcp

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9001/health || exit 1

# Environment
ENV AETHER_CONFIG=/etc/aether/aether.toml
ENV AETHER_DATA=/var/lib/aether
ENV AETHER_LOGS=/var/log/aether

ENTRYPOINT ["aether"]
CMD ["run"]
