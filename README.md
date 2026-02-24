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

- **Swagger/OpenAPI Documentation** — Auto-generated interactive API docs
  available at `/swagger-ui/`.

- **Terminal Interface** — Colored board display with interactive move input
  for local two-player games.

## Quick Start

### Build

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

## Project Structure

```bash
checkai/
├── Cargo.toml          # Dependencies and project metadata
├── README.md           # This file
├── docs/
│   └── AGENT.md        # Chess rules and JSON protocol for AI agents
└── src/
    ├── main.rs         # Entry point, CLI parsing, server setup
    ├── types.rs        # Core types (pieces, board, squares, JSON protocol)
    ├── movegen.rs      # Move generation and validation engine
    ├── game.rs         # Game state management and API response types
    ├── api.rs          # REST API handlers with OpenAPI annotations
    └── terminal.rs     # Terminal interface with colored output
```

## Chess Rules

This implementation follows the **FIDE Laws of Chess (2023)**. See
[`docs/AGENT.md`](docs/AGENT.md) for the complete rulebook and JSON protocol
specification used by AI agents.

## License

MIT
