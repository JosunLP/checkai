# Docker

CheckAI provides a multi-stage Dockerfile and docker-compose configuration for containerized deployment.

## Quick Start

```bash
# Build and start
docker compose up -d

# Follow logs
docker compose logs -f

# Stop
docker compose down
```

## Docker Compose

The default `docker-compose.yml` binds port 8080 and persists game data:

```yaml
services:
  checkai:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: checkai
    ports:
      - "${CHECKAI_PORT:-8080}:8080"
    volumes:
      - checkai-data:/home/checkai/data
      - ./books:/home/checkai/books:ro
      - ./tablebase:/home/checkai/tablebase:ro
    environment:
      - RUST_LOG=${RUST_LOG:-info}
    command:
      - serve
      - --host
      - "0.0.0.0"
      - --port
      - "8080"
      - --data-dir
      - data
    restart: unless-stopped
```

### Custom Port

```bash
CHECKAI_PORT=3000 docker compose up -d
```

### With Opening Book and Tablebases

Place your files:

- Polyglot book: `./books/book.bin`
- Syzygy tablebases: `./tablebase/`

Then uncomment the book/tablebase arguments in `docker-compose.yml`:

```yaml
command:
  - serve
  - --host
  - "0.0.0.0"
  - --port
  - "8080"
  - --data-dir
  - data
  - --book-path
  - books/book.bin
  - --tablebase-path
  - tablebase
```

## Dockerfile

The Dockerfile uses a two-stage build:

1. **Build stage** — Uses `rust:1.87-bookworm` to compile the release binary with dependency caching.
2. **Runtime stage** — Uses `debian:bookworm-slim` with only the binary, locale files, and a non-root user.

The resulting image is minimal and runs as a non-root user (`checkai`).

## Docker Image from GHCR

Pre-built images are published to GitHub Container Registry on every release:

```bash
# Latest release
docker pull ghcr.io/josunlp/checkai:latest

# Specific version
docker pull ghcr.io/josunlp/checkai:0.3.1

# Run directly
docker run -p 8080:8080 ghcr.io/josunlp/checkai:latest serve
```

## Health Check

The container includes a health check that polls the API:

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:8080/api/games"]
  interval: 30s
  timeout: 5s
  start_period: 10s
```
