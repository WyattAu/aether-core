# syntax=docker/dockerfile:1
#
# Aether Container Image — Python/FastAPI Server
#
# PURPOSE:
#   - Development and testing environments
#   - CI/CD pipeline builds
#   - Kubernetes deployments
#
# For production deployment, see: .docs/deployment_guide.md
#
# ============================================
# Stage 1: Build Dependencies (Cached)
# ============================================
FROM python:3.12-slim AS builder

WORKDIR /build

# Install build dependencies
COPY server/pyproject.toml .
RUN pip install --no-cache-dir --prefix=/install .

# ============================================
# Stage 2: Runtime Image
# ============================================
FROM python:3.12-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r aether && useradd -r -g aether -d /app aether

WORKDIR /app

# Copy installed packages from builder
COPY --from=builder /install /usr/local

# Copy server source code
COPY server/ .

# Create directories
RUN mkdir -p /var/lib/aether /var/log/aether && \
    chown -R aether:aether /app /var/lib/aether /var/log/aether

USER aether

# Expose ports: REST API, gRPC, Cluster Gossip
EXPOSE 8080 50051 7946

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Environment
ENV AETHER_STATE_BACKEND=memory
ENV AETHER_LOG_LEVEL=info

# Default: REST + gRPC server
CMD ["uvicorn", "server.app:app", "--host", "0.0.0.0", "--port", "8080"]
