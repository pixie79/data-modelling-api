# Multi-stage build for data-modelling-api

# Build stage
# Using nightly for Rust 2024 edition support
FROM rustlang/rust:nightly-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy SQLX offline metadata directory (required for offline builds)
# Generate it first with: cargo sqlx prepare -- --all-features
# If .sqlx doesn't exist, the build will fail - this is intentional to ensure
# SQLX queries are validated before deployment
COPY .sqlx ./.sqlx

# Copy source code
COPY src ./src
COPY migrations ./migrations

# Build the application with SQLX offline mode
# This requires .sqlx directory to be present (generated via cargo sqlx prepare)
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin api

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 appuser && chown -R appuser:appuser /app

# Copy binary from builder
COPY --from=builder /app/target/release/api /app/api

# Copy migrations
COPY --from=builder /app/migrations ./migrations

# Switch to non-root user
USER appuser

# Expose port
EXPOSE 8081

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8081/health || exit 1

# Run the application
CMD ["/app/api"]
