# Getting Started

## Installation

### Pre-built Binaries (Recommended)

Download the latest release for your platform:

::: code-group

```bash [Linux / macOS]
curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | sh
```

```powershell [Windows]
irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.ps1 | iex
```

:::

### Build from Source

You need [Rust](https://www.rust-lang.org/tools/install) installed (edition 2024).

```bash
git clone https://github.com/JosunLP/checkai.git
cd checkai
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
