# Getting Started

## System Requirements

| Requirement              | Minimum                                            |
| ------------------------ | -------------------------------------------------- |
| OS                       | Linux (glibc 2.31+), macOS 12+, Windows 10+        |
| CPU                      | Any x86-64 or ARM64                                |
| RAM                      | 128 MB (+ transposition table size, default 64 MB) |
| Disk                     | ~20 MB for the binary                              |
| Rust (build from source) | 1.85.0+ (edition 2024)                             |
| Bun (web UI build)       | 1.3.10+                                            |

## Installation

### Pre-built Binaries (Recommended)

Run the installer directly in a single command:

::: code-group

```bash [Linux / macOS]
curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | sh
```

```powershell [Windows]
irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.ps1 | iex
```

:::

These commands fetch the current install script directly and install the latest CheckAI release for your platform.
If you prefer to inspect the script first, open or download the same URL before running it manually.

> [!WARNING]
> The one-line commands above execute a remote script immediately.
> Only use them if you trust the source. If you want a manual review step, open or download the same URL first and inspect it before running it yourself.

### Build from Source

You need [Rust](https://www.rust-lang.org/tools/install) and [Bun](https://bun.sh/) installed.

```bash
git clone https://github.com/JosunLP/checkai.git
cd checkai

# Build the embedded web UI first
cd web
bun install --frozen-lockfile
bun run build
cd ..

# Then build the Rust server/CLI
cargo build --release
```

The binary will be at `target/release/checkai` (or `checkai.exe` on Windows).

### Docker

```bash
docker compose up -d
```

See the [Docker guide](./docker) for full details.

## Quick Start

### Start the API Server

```bash
# Default: http://0.0.0.0:8080
checkai serve

# Custom port
checkai serve --port 3000
```

Swagger UI is automatically available at `http://localhost:8080/swagger-ui/`.

### Create and Play a Game via API

```bash
# 1. Create a new game
curl -X POST http://localhost:8080/api/games

# 2. Get game state
curl http://localhost:8080/api/games/{game_id}

# 3. Submit a move (1. e4)
curl -X POST http://localhost:8080/api/games/{game_id}/move \
  -H "Content-Type: application/json" \
  -d '{"from": "e2", "to": "e4", "promotion": null}'

# 4. Get legal moves
curl http://localhost:8080/api/games/{game_id}/moves
```

### Play in the Terminal

```bash
checkai play
```

This starts an interactive two-player game with a colored board display. Type `help` for available commands.

## Uninstall

::: code-group

```bash [Linux / macOS]
curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.sh | sh
```

```powershell [Windows]
irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.ps1 | iex
```

:::

## Next Steps

- [CLI Commands](./cli) — All available commands and flags
- [REST API Reference](../api/rest) — Complete endpoint documentation
- [Agent Protocol](../agent/overview) — How AI agents interact with CheckAI
