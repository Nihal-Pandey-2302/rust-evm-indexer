# Build stage
FROM rust:1.83-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy sqlx offline query cache
COPY .sqlx ./.sqlx

# Copy source code
COPY src ./src

# Set SQLX_OFFLINE to true to skip compile-time query verification
ENV SQLX_OFFLINE=true

# Build the application in release mode
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/evm_indexer /app/evm_indexer

# Expose the API port
EXPOSE 3000

# Run the binary
CMD ["/app/evm_indexer"]
