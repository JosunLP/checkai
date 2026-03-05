# CheckAI — Chess Server for AI Agents

A Rust application that provides both a **terminal interface** and a **REST API**
for playing chess. Designed for AI agents to play chess against each other,
following the **FIDE 2023 Laws of Chess**.

**[📖 Documentation](https://josunlp.github.io/checkai/)** · **[Changelog](CHANGELOG.md)** · **[Releases](https://github.com/JosunLP/checkai/releases)**

## Features

- **Complete Chess Engine** — Full move generation and validation including
  castling, en passant, promotion, check/checkmate/stalemate detection,
  and all draw conditions (50-move rule, threefold repetition, insufficient
  material).

- **Deep Game Analysis** — Asynchronous analysis engine with a minimum search
  depth of 30 plies. Classifies every move as Best / Excellent / Good /
  Inaccuracy / Mistake / Blunder with centipawn loss and principal variation.
  Includes PeSTO evaluation with king safety and piece mobility, alpha-beta
  search with PVS, transposition table, null-move pruning, LMR, SEE pruning,
  futility pruning, killer/history heuristics, and quiescence search.

- **Opening Book & Endgame Tablebases** — Polyglot `.bin` opening book support
  and Syzygy endgame tablebase detection with limited analytical evaluation
  (no full tablebase probing / perfect endgame play yet).
- **REST API** — JSON-based API for AI agents to create games, query state,
  submit moves, and handle special actions (draw claims, resignation).
  Follows the protocol defined in [`docs/AGENT.md`](docs/AGENT.md).

- **Analysis API** — Separate `/api/analysis/*` endpoints for submitting games
  for deep analysis with real-time progress tracking. Architecturally isolated
  from the player-facing game endpoints.

- **WebSocket API** — Full WebSocket support at `/ws` mirroring every REST
  endpoint, with real-time event broadcasting. Clients can subscribe to
  individual games and receive push notifications for moves, state changes,
  and game deletions.

- **Swagger/OpenAPI Documentation** — Auto-generated interactive API docs
  available at `/swagger-ui/`.

- **Terminal Interface** — Colored board display with interactive move input
  for local two-player games.

- **Modern Web UI** — TypeScript single-page application built with
  @bquery/bquery, Tailwind CSS v4, and Vite. Features interactive SVG board,
  analysis panel, FEN/PGN tools, promotion dialog, and WebSocket auto-reconnect.
  Compiled into the binary via `rust-embed`.

- **Docker Support** — Multi-stage Dockerfile and docker-compose.yml for
  containerized deployment with volume mounts for game data, opening books,
  and tablebases.

- **Documentation** — Full [VitePress documentation site](https://josunlp.github.io/checkai/)
  with guides, API reference, and agent protocol specification. Automatically
  deployed to GitHub Pages on every release.

## Quick Start

### Install (pre-built binary)

**Linux / macOS:**

```bash
curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.ps1 | iex
```

### Uninstall

**Linux / macOS:**

```bash
curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.ps1 | iex
```

### Build from source

```bash
cargo build --release
```

### Run the API Server

```bash
# Default: http://0.0.0.0:8080
cargo run -- serve

# Custom port
cargo run -- serve --port 3000

# With opening book and tablebase
cargo run -- serve --book-path books/book.bin --tablebase-path tablebase/ --analysis-depth 30
```

Swagger UI will be available at `http://localhost:8080/swagger-ui/`.

### Run with Docker

```bash
# Build and start
docker compose up -d

# Follow logs
docker compose logs -f

# Stop
docker compose down
```

See `docker-compose.yml` to configure opening book and tablebase volume mounts.

### Play in Terminal

```bash
cargo run -- play
```

## API Endpoints

### Game API

| Method   | Path                     | Description             |
| -------- | ------------------------ | ----------------------- |
| `POST`   | `/api/games`             | Create a new game       |
| `GET`    | `/api/games`             | List all games          |
| `GET`    | `/api/games/{id}`        | Get full game state     |
| `DELETE` | `/api/games/{id}`        | Delete a game           |
| `POST`   | `/api/games/{id}/move`   | Submit a move           |
| `POST`   | `/api/games/{id}/action` | Submit a special action |
| `GET`    | `/api/games/{id}/moves`  | Get all legal moves     |
| `GET`    | `/api/games/{id}/board`  | Get ASCII board display |
| `GET`    | `/api/games/{id}/fen`    | Export FEN notation     |
| `POST`   | `/api/games/fen`         | Import game from FEN    |
| `GET`    | `/api/games/{id}/pgn`    | Export PGN notation     |
| `GET`    | `/ws`                    | WebSocket endpoint      |

### Analysis API

| Method   | Path                      | Description                |
| -------- | ------------------------- | -------------------------- |
| `POST`   | `/api/analysis/game/{id}` | Submit a game for analysis |
| `GET`    | `/api/analysis/jobs`      | List all analysis jobs     |
| `GET`    | `/api/analysis/jobs/{id}` | Get job status and results |
| `DELETE` | `/api/analysis/jobs/{id}` | Cancel or delete a job     |

## API Usage Example

### 1. Create a Game

```bash
curl -X POST http://localhost:8080/api/games
```

Response:

```json
{
  "game_id": "550e8400-e29b-41d4-a716-446655440000",
  "message": "New chess game created. White to move."
}
```

### 2. Get Game State

```bash
curl http://localhost:8080/api/games/{game_id}
```

### 3. Submit a Move (e.g. 1. e4)

```bash
curl -X POST http://localhost:8080/api/games/{game_id}/move \
  -H "Content-Type: application/json" \
  -d '{"from": "e2", "to": "e4", "promotion": null}'
```

### 4. Get Legal Moves

```bash
curl http://localhost:8080/api/games/{game_id}/moves
```

### 5. Resign

```bash
curl -X POST http://localhost:8080/api/games/{game_id}/action \
  -H "Content-Type: application/json" \
  -d '{"action": "resign"}'
```

### 6. Claim Draw

```bash
curl -X POST http://localhost:8080/api/games/{game_id}/action \
  -H "Content-Type: application/json" \
  -d '{"action": "claim_draw", "reason": "threefold_repetition"}'
```

### 7. Submit Game for Analysis

```bash
curl -X POST http://localhost:8080/api/analysis/game/{game_id} \
  -H "Content-Type: application/json" \
  -d '{"depth": 30}'
```

Response:

```json
{
  "job_id": "a1b2c3d4-...",
  "message": "Analysis submitted for game ... (42 moves)"
}
```

### 8. Get Analysis Results

```bash
curl http://localhost:8080/api/analysis/jobs/{job_id}
```

## WebSocket API

Connect to `ws://localhost:8080/ws` to use the fully reactive WebSocket API.

### Client → Server Messages

All messages are JSON with an `"action"` field. Include an optional
`"request_id"` for response correlation.

| Action              | Extra Fields                          |
| ------------------- | ------------------------------------- |
| `create_game`       | —                                     |
| `list_games`        | —                                     |
| `get_game`          | `game_id`                             |
| `delete_game`       | `game_id`                             |
| `submit_move`       | `game_id`, `from`, `to`, `promotion?` |
| `submit_action`     | `game_id`, `action_type`, `reason?`   |
| `get_legal_moves`   | `game_id`                             |
| `get_board`         | `game_id`                             |
| `subscribe`         | `game_id`                             |
| `unsubscribe`       | `game_id`                             |
| `list_archived`     | —                                     |
| `get_archived`      | `game_id`                             |
| `replay_archived`   | `game_id`, `move_number?`             |
| `get_storage_stats` | —                                     |

### Server → Client Messages

**Response** (to a command):

```json
{
  "type": "response",
  "action": "submit_move",
  "request_id": "abc123",
  "success": true,
  "data": { ... }
}
```

**Event** (pushed to subscribers):

```json
{
  "type": "event",
  "event": "game_updated",
  "game_id": "550e8400-...",
  "data": { ... }
}
```

### WebSocket Example (JavaScript)

```javascript
const ws = new WebSocket("ws://localhost:8080/ws");

ws.onopen = () => {
  // Create a game
  ws.send(JSON.stringify({ action: "create_game", request_id: "1" }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  if (msg.type === "response" && msg.action === "create_game") {
    const gameId = msg.data.game_id;

    // Subscribe to real-time events
    ws.send(JSON.stringify({ action: "subscribe", game_id: gameId }));

    // Make a move
    ws.send(JSON.stringify({
      action: "submit_move",
      game_id: gameId,
      from: "e2",
      to: "e4"
    }));
  }

  if (msg.type === "event") {
    console.log("Game event:", msg.event, msg.data);
  }
};
```

## Terminal Commands

When running in terminal mode (`checkai play`):

| Command   | Description                          |
| --------- | ------------------------------------ |
| `e2e4`    | Move piece (from-to notation)        |
| `e7e8Q`   | Pawn promotion (append piece letter) |
| `moves`   | List all legal moves                 |
| `board`   | Show the current board               |
| `resign`  | Resign the game                      |
| `draw`    | Claim a draw (if eligible)           |
| `history` | Show move history                    |
| `json`    | Show the game state as JSON          |
| `help`    | Show help message                    |
| `quit`    | Quit the application                 |

## Update

CheckAI checks for new versions automatically on startup. To update manually:

```bash
checkai update
```

This downloads the latest release from GitHub and replaces the current binary
in-place. Works on Linux, macOS, and Windows.

## Project Structure

```bash
checkai/
├── Cargo.toml          # Dependencies and project metadata
├── CHANGELOG.md        # Version history (Keep a Changelog)
├── README.md           # This file
├── Dockerfile          # Multi-stage Docker build
├── docker-compose.yml  # Container orchestration
├── .dockerignore       # Docker build exclusions
├── .github/
│   └── workflows/
│       ├── ci.yml      # CI pipeline (fmt, clippy, test, build)
│       ├── release.yml # Release pipeline (binaries + Docker image)
│       └── docs.yml    # Documentation deployment to GitHub Pages
├── scripts/
│   ├── install.sh      # Installer for Linux / macOS
│   ├── install.ps1     # Installer for Windows
│   ├── uninstall.sh    # Uninstaller for Linux / macOS
│   └── uninstall.ps1   # Uninstaller for Windows
├── docs/                # VitePress documentation site
│   ├── .vitepress/     # VitePress configuration
│   ├── guide/          # User guides (getting started, CLI, Docker, etc.)
│   ├── api/            # API reference (REST, WebSocket, Analysis)
│   ├── agent/          # Agent protocol docs (rules, schema, examples)
│   └── AGENT.md        # Chess rules and JSON protocol for AI agents
├── web/                # Browser-based game UI (TypeScript / bQuery / Tailwind / Vite)
│   ├── src/            # TypeScript source modules
│   ├── dist/           # Vite production build output (embedded into binary)
│   ├── index.vite.html # Vite HTML entry point
│   ├── vite.config.ts  # Vite + Tailwind configuration
│   └── package.json    # Frontend dependencies (bun)
└── src/
    ├── main.rs         # Entry point, CLI parsing, server setup
    ├── types.rs        # Core types (pieces, board, squares, JSON protocol)
    ├── movegen.rs      # Move generation and validation engine
    ├── game.rs         # Game state management and API response types
    ├── api.rs          # REST API handlers with OpenAPI annotations
    ├── ws.rs           # WebSocket API, broadcaster, and session actors
    ├── storage.rs      # Persistent binary game storage with zstd compression
    ├── export.rs       # Game export in text, PGN, and JSON formats
    ├── update.rs       # Self-update and version check against GitHub
    ├── terminal.rs     # Terminal interface with colored output
    ├── i18n.rs         # Internationalization helpers
    ├── zobrist.rs      # Zobrist hashing (compile-time key generation)
    ├── eval.rs         # PeSTO evaluation + king safety + piece mobility
    ├── search.rs       # Alpha-beta search (PVS, TT, LMR, NMP, SEE, futility)
    ├── opening_book.rs # Polyglot opening book reader
    ├── tablebase.rs    # Syzygy endgame tablebase interface
    ├── analysis.rs     # Analysis orchestrator (async job queue, pipeline)
    └── analysis_api.rs # Analysis REST API endpoints
```

## Documentation

Full documentation is available at **https://josunlp.github.io/checkai/**

- [Getting Started](https://josunlp.github.io/checkai/guide/getting-started) — Installation and first steps
- [API Reference](https://josunlp.github.io/checkai/api/rest) — REST, WebSocket, and Analysis API
- [Agent Protocol](https://josunlp.github.io/checkai/agent/overview) — JSON protocol for AI agents
- [Chess Rules](https://josunlp.github.io/checkai/agent/chess-rules) — Complete FIDE 2023 rule reference

The raw agent protocol specification is also available at [`docs/AGENT.md`](docs/AGENT.md).

## License

MIT
