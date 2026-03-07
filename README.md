<div align="center">

# CheckAI

**_Chess Server for AI Agents_**

A Rust-powered chess server and CLI with REST, WebSocket, and deep analysis APIs — following FIDE 2023 rules.

[![CI](https://github.com/JosunLP/checkai/actions/workflows/ci.yml/badge.svg)](https://github.com/JosunLP/checkai/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE.md)
[![Rust](https://img.shields.io/badge/Rust-edition_2024-orange.svg)](https://www.rust-lang.org/)
[![GitHub All Releases](https://img.shields.io/github/downloads/josunlp/checkai/total.svg?label=Downloads)](https://github.com/JosunLP/checkai/releases)

[Documentation](https://josunlp.github.io/checkai/) | [Changelog](CHANGELOG.md) | [Releases](https://github.com/JosunLP/checkai/releases)

</div>

---

## Features

### Chess Engine

- **Full FIDE 2023 Rules** — Move generation and validation with castling, en passant, promotion, check/checkmate/stalemate, and all draw conditions (50-move rule, threefold repetition, insufficient material)
- **Deep Game Analysis** — Asynchronous engine with 30+ ply depth, PVS/Negascout, transposition table, null-move pruning, LMR, SEE, futility pruning, killer/history heuristics, and quiescence search
- **PeSTO Evaluation** — Midgame/endgame piece-square tables with king safety, pawn shield analysis, piece mobility, and phase interpolation
- **Opening Book** — Polyglot `.bin` format with binary search lookups
- **Endgame Tablebases** — Syzygy tablebase detection with analytical evaluation for common endgames

### APIs & Interfaces

- **REST API** — JSON-based endpoints for game management, moves, draw claims, resignation, FEN/PGN import/export ([Agent Protocol](docs/AGENT.md))
- **Analysis API** — Separate `/api/analysis/*` endpoints for asynchronous game review with job progress, completed summaries, and per-move annotations
- **WebSocket API** — Full real-time API at `/ws` mirroring REST endpoints with push notifications and game subscriptions
- **Swagger/OpenAPI** — Auto-generated interactive API docs at `/swagger-ui/`
- **Terminal Interface** — Colored board display with interactive move input for local two-player games

### Web & Deployment

- **Modern Web UI** — TypeScript SPA with @bquery/bquery, Tailwind CSS v4, Vite — interactive SVG board, analysis panel, FEN/PGN tools, promotion dialog, WebSocket auto-reconnect. Compiled into the binary via `rust-embed`
- **Docker Support** — Multi-stage Dockerfile and docker-compose.yml with volume mounts for game data, opening books, and tablebases
- **Internationalization** — 8 languages (EN, DE, FR, ES, ZH, JA, PT, RU) with auto-detection and per-request API selection
- **Self-Update** — Automatic version checks and `checkai update` for in-place binary updates
- **JavaScript Package** — [`@josunlp/checkai`](https://github.com/JosunLP/checkai/packages) on GitHub Packages — the full chess engine compiled to WebAssembly, usable as a Bun or Node.js CLI/library package

## Quick Start

### Install

**Linux / macOS:**

```bash
VERSION="0.5.2"
curl -fsSL -o install.sh \
  "https://raw.githubusercontent.com/JosunLP/checkai/v${VERSION}/scripts/install.sh"
sh install.sh
```

**Windows (PowerShell):**

```powershell
$Version = "0.5.2"
Invoke-WebRequest `
  -Uri "https://raw.githubusercontent.com/JosunLP/checkai/v$Version/scripts/install.ps1" `
  -OutFile install.ps1
.\install.ps1
```

> **Tip:** For production use, download and verify the script before running it.
> See the [Getting Started guide](https://josunlp.github.io/checkai/guide/getting-started) for details.

### Build from Source

```bash
git clone https://github.com/JosunLP/checkai.git
cd checkai

# Build web UI (requires Bun)
cd web && bun install && bun run build && cd ..

# Build the Rust binary
cargo build --release
```

### Start the Server

```bash
checkai serve                    # Default: http://0.0.0.0:8080
checkai serve --port 3000        # Custom port
checkai serve \
  --book-path books/book.bin \
  --tablebase-path tablebase/ \
  --analysis-depth 30            # With opening book + tablebases
```

Open `http://localhost:8080/` for the Web UI or `/swagger-ui/` for interactive API docs.

### Docker

```bash
docker compose up -d             # Build and start
docker compose logs -f           # Follow logs
docker compose down              # Stop
```

### JavaScript Package (WebAssembly)

The chess engine is also available as a Bun/Node.js package via **GitHub Packages**:

```bash
# Configure GitHub Packages registry (Bun reads .npmrc)
echo "@josunlp:registry=https://npm.pkg.github.com" >> ~/.npmrc

# Install as CLI
bun add --global @josunlp/checkai
checkai fen
checkai search "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1" --depth 15

# Or use as library
bun add @josunlp/checkai
```

```javascript
import { engine } from "@josunlp/checkai";
const moves = engine.legalMoves(engine.startingFen());
const result = engine.bestMove(engine.startingFen(), 10);
```

See the [package README](npm/README.md) for the full API reference.

### Terminal Mode

```bash
checkai play
```

## API Reference

### Game Endpoints

| Method   | Path                     | Description                         |
| -------- | ------------------------ | ----------------------------------- |
| `POST`   | `/api/games`             | Create a new game                   |
| `GET`    | `/api/games`             | List all games                      |
| `GET`    | `/api/games/{id}`        | Get full game state                 |
| `DELETE` | `/api/games/{id}`        | Delete a game                       |
| `POST`   | `/api/games/{id}/move`   | Submit a move                       |
| `POST`   | `/api/games/{id}/action` | Special action (resign, draw claim) |
| `GET`    | `/api/games/{id}/moves`  | List legal moves                    |
| `GET`    | `/api/games/{id}/board`  | ASCII board display                 |
| `GET`    | `/api/games/{id}/fen`    | Export FEN notation                 |
| `POST`   | `/api/games/fen`         | Import game from FEN                |
| `GET`    | `/api/games/{id}/pgn`    | Export PGN notation                 |

### Analysis Endpoints

| Method   | Path                      | Description              |
| -------- | ------------------------- | ------------------------ |
| `POST`   | `/api/analysis/game/{id}` | Submit game for analysis |
| `GET`    | `/api/analysis/jobs`      | List all analysis jobs   |
| `GET`    | `/api/analysis/jobs/{id}` | Get job status & results |
| `DELETE` | `/api/analysis/jobs/{id}` | Cancel or delete a job   |

### WebSocket

Connect to `ws://localhost:8080/ws` for real-time bidirectional communication.

| Action                           | Fields                                |
| -------------------------------- | ------------------------------------- |
| `create_game`                    | —                                     |
| `list_games`                     | —                                     |
| `get_game`                       | `game_id`                             |
| `delete_game`                    | `game_id`                             |
| `submit_move`                    | `game_id`, `from`, `to`, `promotion?` |
| `submit_action`                  | `game_id`, `action_type`, `reason?`   |
| `get_legal_moves`                | `game_id`                             |
| `subscribe` / `unsubscribe`      | `game_id`                             |
| `list_archived` / `get_archived` | `game_id`                             |
| `replay_archived`                | `game_id`, `move_number?`             |

> Full API documentation with request/response schemas: [REST](https://josunlp.github.io/checkai/api/rest) | [WebSocket](https://josunlp.github.io/checkai/api/websocket) | [Analysis](https://josunlp.github.io/checkai/api/analysis)

## Usage Examples

### REST API

```bash
# Create a game
curl -X POST http://localhost:8080/api/games
# → { "game_id": "550e8400-...", "message": "New chess game created. White to move." }

# Submit a move (1. e4)
curl -X POST http://localhost:8080/api/games/{game_id}/move \
  -H "Content-Type: application/json" \
  -d '{"from": "e2", "to": "e4"}'

# Get legal moves
curl http://localhost:8080/api/games/{game_id}/moves

# Resign
curl -X POST http://localhost:8080/api/games/{game_id}/action \
  -H "Content-Type: application/json" \
  -d '{"action": "resign"}'

# Claim draw
curl -X POST http://localhost:8080/api/games/{game_id}/action \
  -H "Content-Type: application/json" \
  -d '{"action": "claim_draw", "reason": "threefold_repetition"}'

# Submit game for deep analysis
curl -X POST http://localhost:8080/api/analysis/game/{game_id} \
  -H "Content-Type: application/json" \
  -d '{"depth": 30}'
# → { "job_id": "a1b2c3d4-...", "message": "Analysis submitted ..." }

# Get analysis results
curl http://localhost:8080/api/analysis/jobs/{job_id}
```

### WebSocket and Real-Time Events

```javascript
const ws = new WebSocket("ws://localhost:8080/ws");

ws.onopen = () => {
  ws.send(JSON.stringify({ action: "create_game", request_id: "1" }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  if (msg.type === "response" && msg.action === "create_game") {
    const gameId = msg.data.game_id;
    ws.send(JSON.stringify({ action: "subscribe", game_id: gameId }));
    ws.send(JSON.stringify({
      action: "submit_move", game_id: gameId, from: "e2", to: "e4"
    }));
  }

  if (msg.type === "event") {
    console.log("Game event:", msg.event, msg.data);
  }
};
```

## Terminal Commands

| Command   | Description                          |
| --------- | ------------------------------------ |
| `e2e4`    | Move piece (from-to notation)        |
| `e7e8Q`   | Pawn promotion (append piece letter) |
| `moves`   | List all legal moves                 |
| `board`   | Show current board                   |
| `resign`  | Resign the game                      |
| `draw`    | Claim a draw (if eligible)           |
| `history` | Show move history                    |
| `json`    | Game state as JSON                   |
| `help`    | Show help                            |
| `quit`    | Quit                                 |

## Updating

CheckAI checks for new versions on startup. Update manually:

```bash
checkai update
```

## Project Structure

```bash
checkai/
├── build.rs              # Ensures web/dist/ exists for rust-embed
├── Cargo.toml            # Dependencies and project metadata
├── Dockerfile            # Multi-stage Docker build
├── docker-compose.yml    # Container orchestration
├── .github/workflows/
│   ├── ci.yml            # CI (fmt, clippy, test, build)
│   ├── release.yml       # Release (binaries + Docker image)
│   └── docs.yml          # Documentation → GitHub Pages
├── scripts/
│   ├── install.sh        # Installer (Linux / macOS)
│   ├── install.ps1       # Installer (Windows)
│   ├── uninstall.sh      # Uninstaller (Linux / macOS)
│   └── uninstall.ps1     # Uninstaller (Windows)
├── docs/                 # VitePress documentation site
├── locales/              # i18n YAML files (8 languages)
├── wasm/                 # WebAssembly crate (wasm-pack)
│   ├── Cargo.toml        # WASM crate manifest
│   └── src/
│       ├── lib.rs        # WASM bindings (game mgmt, export, board)
│       └── search.rs     # Search with web-time::Instant
├── npm/                  # JS package (@josunlp/checkai)
│   ├── package.json      # Scoped to GitHub Packages
│   ├── bin/checkai.mjs   # Node.js CLI entry point
│   ├── src/index.mjs     # Library API exports
│   └── README.md         # package documentation
├── web/                  # TypeScript Web UI (bQuery + Tailwind + Vite)
│   ├── src/              # 12 TypeScript source modules
│   ├── dist/             # Vite production build (embedded into binary)
│   └── index.vite.html   # Vite HTML entry point
└── src/
    ├── main.rs           # Entry point, CLI, server setup
    ├── types.rs          # Core types (pieces, board, JSON protocol)
    ├── movegen.rs        # Move generation and validation
    ├── game.rs           # Game state management
    ├── api.rs            # REST API handlers + OpenAPI
    ├── ws.rs             # WebSocket API + broadcaster
    ├── storage.rs        # Persistent storage (zstd compression)
    ├── export.rs         # Export (text, PGN, JSON)
    ├── eval.rs           # PeSTO evaluation + king safety + mobility
    ├── search.rs         # Alpha-beta (PVS, TT, LMR, NMP, SEE, futility)
    ├── analysis.rs       # Analysis orchestrator (async job queue)
    ├── analysis_api.rs   # Analysis REST endpoints
    ├── opening_book.rs   # Polyglot opening book reader
    ├── tablebase.rs      # Syzygy endgame tablebase interface
    ├── zobrist.rs        # Zobrist hashing
    ├── terminal.rs       # Terminal interface
    ├── i18n.rs           # Internationalization helpers
    └── update.rs         # Self-update + version check
```

## Documentation

Full documentation at **<https://josunlp.github.io/checkai/>**

| Section                                                                    | Description                          |
| -------------------------------------------------------------------------- | ------------------------------------ |
| [Getting Started](https://josunlp.github.io/checkai/guide/getting-started) | Installation and first steps         |
| [REST API](https://josunlp.github.io/checkai/api/rest)                     | Full REST endpoint reference         |
| [WebSocket API](https://josunlp.github.io/checkai/api/websocket)           | Real-time bidirectional API          |
| [Analysis API](https://josunlp.github.io/checkai/api/analysis)             | Deep game analysis endpoints         |
| [Agent Protocol](https://josunlp.github.io/checkai/agent/overview)         | JSON protocol for AI agents          |
| [Chess Rules](https://josunlp.github.io/checkai/agent/chess-rules)         | FIDE 2023 rule reference             |
| [Architecture](https://josunlp.github.io/checkai/guide/architecture)       | Module overview and design decisions |
| [JavaScript Package](npm/README.md)                                        | WASM package API reference           |

The raw agent protocol specification is also available at [`docs/AGENT.md`](docs/AGENT.md).

## License

[MIT](LICENSE.md)
