# CheckAI — Chess Server for AI Agents

A Rust application that provides both a **terminal interface** and a **REST API**
for playing chess. Designed for AI agents to play chess against each other,
following the **FIDE 2023 Laws of Chess**.

## Features

- **Complete Chess Engine** — Full move generation and validation including
  castling, en passant, promotion, check/checkmate/stalemate detection,
  and all draw conditions (50-move rule, threefold repetition, insufficient
  material).

- **REST API** — JSON-based API for AI agents to create games, query state,
  submit moves, and handle special actions (draw claims, resignation).
  Follows the protocol defined in [`docs/AGENT.md`](docs/AGENT.md).

- **WebSocket API** — Full WebSocket support at `/ws` mirroring every REST
  endpoint, with real-time event broadcasting. Clients can subscribe to
  individual games and receive push notifications for moves, state changes,
  and game deletions.

- **Swagger/OpenAPI Documentation** — Auto-generated interactive API docs
  available at `/swagger-ui/`.

- **Terminal Interface** — Colored board display with interactive move input
  for local two-player games.

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
```

Swagger UI will be available at `http://localhost:8080/swagger-ui/`.

### Play in Terminal

```bash
cargo run -- play
```

## API Endpoints

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
| `GET`    | `/ws`                    | WebSocket endpoint      |

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
├── .github/
│   └── workflows/
│       ├── ci.yml      # CI pipeline (fmt, clippy, test, build)
│       └── release.yml # Release pipeline (cross-platform binaries)
├── scripts/
│   ├── install.sh      # Installer for Linux / macOS
│   ├── install.ps1     # Installer for Windows
│   ├── uninstall.sh    # Uninstaller for Linux / macOS
│   └── uninstall.ps1   # Uninstaller for Windows
├── docs/
│   └── AGENT.md        # Chess rules and JSON protocol for AI agents
├── web/                # Browser-based game UI
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
    └── terminal.rs     # Terminal interface with colored output
```

## Chess Rules

This implementation follows the **FIDE Laws of Chess (2023)**. See
[`docs/AGENT.md`](docs/AGENT.md) for the complete rulebook and JSON protocol
specification used by AI agents.

## License

MIT
