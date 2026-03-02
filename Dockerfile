# =============================================================================
# CheckAI — Multi-stage Docker build
# =============================================================================
# Stage 1: Build the Rust binary
# Stage 2: Create a minimal runtime image
# =============================================================================

# ---------------------------------------------------------------------------
# Stage 1 — Build
# ---------------------------------------------------------------------------
FROM rust:1.87-bookworm AS builder

WORKDIR /usr/src/checkai

# Copy manifests first (for Docker layer caching of dependencies)
COPY Cargo.toml Cargo.lock* ./

# Create a dummy main.rs to pre-build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Copy the full source tree
COPY . .

# Touch main.rs so Cargo rebuilds our code (not just deps)
RUN touch src/main.rs

# Build the release binary
RUN cargo build --release --locked 2>/dev/null || cargo build --release

# ---------------------------------------------------------------------------
# Stage 2 — Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user for running the application
RUN useradd --create-home --shell /bin/bash checkai

WORKDIR /home/checkai

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/checkai/target/release/checkai /usr/local/bin/checkai

# Copy locale files (needed at runtime for i18n)
COPY --from=builder /usr/src/checkai/locales ./locales

# Create data directories
RUN mkdir -p data/active data/archive books tablebase \
    && chown -R checkai:checkai /home/checkai

USER checkai

# The server listens on port 8080 by default
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/games || exit 1

# Default command: start the server
ENTRYPOINT ["checkai"]
CMD ["serve", "--host", "0.0.0.0", "--port", "8080", "--data-dir", "data"]
