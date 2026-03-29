# ---- Build stage ----
FROM python:3.12-slim AS builder

WORKDIR /build

# Install build dependencies
COPY pyproject.toml .
RUN pip install --no-cache-dir --prefix=/install .

# ---- Runtime stage ----
FROM python:3.12-slim

WORKDIR /app

# Copy installed packages from builder
COPY --from=builder /install /usr/local

# Copy source code
COPY . .

# Create non-root user
RUN groupadd -r aether && useradd -r -g aether -d /app aether
RUN chown -R aether:aether /app

USER aether

# REST API, gRPC, Cluster Gossip
EXPOSE 8080 50051 7946

# Default: REST + gRPC server
CMD ["uvicorn", "server.app:app", "--host", "0.0.0.0", "--port", "8080"]
