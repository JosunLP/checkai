# REST API

CheckAI provides a JSON-based REST API powered by Actix Web. Interactive Swagger documentation is available at `/swagger-ui/` when the server is running.

## Base URL

```bash
http://localhost:8080/api
```

## Game Endpoints

### Create a Game

```http
POST /api/games
```

Creates a new chess game with the standard starting position.

**Response** `200 OK`:

```json
{
  "game_id": "550e8400-e29b-41d4-a716-446655440000",
  "message": "New chess game created. White to move."
}
```

---

### List Games

```http
GET /api/games
```

Returns all active games.

**Response** `200 OK`:

```json
[
  {
    "game_id": "550e8400-...",
    "status": "in_progress",
    "turn": "white",
    "fullmove_number": 1
  }
]
```

---

### Get Game State

```http
GET /api/games/{id}
```

Returns the complete game state including board, turn, castling rights, and move history.

**Response** `200 OK`:

```json
{
  "board": {
    "a1": "R", "b1": "N", "c1": "B", "d1": "Q", "e1": "K",
    "f1": "B", "g1": "N", "h1": "R",
    "a2": "P", "b2": "P", "c2": "P", "d2": "P", "e2": "P",
    "f2": "P", "g2": "P", "h2": "P",
    "a7": "p", "b7": "p", "c7": "p", "d7": "p", "e7": "p",
    "f7": "p", "g7": "p", "h7": "p",
    "a8": "r", "b8": "n", "c8": "b", "d8": "q", "e8": "k",
    "f8": "b", "g8": "n", "h8": "r"
  },
  "turn": "white",
  "castling": {
    "white": { "kingside": true, "queenside": true },
    "black": { "kingside": true, "queenside": true }
  },
  "en_passant": null,
  "halfmove_clock": 0,
  "fullmove_number": 1,
  "status": "in_progress",
  "move_history": []
}
```

---

### Delete a Game

```http
DELETE /api/games/{id}
```

Deletes a game permanently.

**Response** `200 OK`:

```json
{
  "message": "Game deleted."
}
```

---

### Submit a Move

```http
POST /api/games/{id}/move
Content-Type: application/json
```

**Request Body**:

```json
{
  "from": "e2",
  "to": "e4",
  "promotion": null
}
```

| Field       | Type           | Required | Description                                            |
| ----------- | -------------- | -------- | ------------------------------------------------------ |
| `from`      | string         | Yes      | Starting square (e.g. `"e2"`)                          |
| `to`        | string         | Yes      | Target square (e.g. `"e4"`)                            |
| `promotion` | string \| null | Yes      | Promotion piece (`"Q"`, `"R"`, `"B"`, `"N"`) or `null` |

**Special move encoding**:

- **Castling**: King moves two squares (e.g. `"e1"` → `"g1"` for kingside)
- **En passant**: Normal pawn diagonal capture; system removes the captured pawn
- **Promotion**: `promotion` must be set when a pawn reaches the last rank

**Response** `200 OK`:

```json
{
  "message": "Move e2→e4 accepted.",
  "status": "in_progress"
}
```

---

### Submit a Special Action

```http
POST /api/games/{id}/action
Content-Type: application/json
```

**Request Body** (examples):

```json
{ "action": "resign" }
```

```json
{ "action": "claim_draw", "reason": "threefold_repetition" }
```

```json
{ "action": "claim_draw", "reason": "fifty_move_rule" }
```

```json
{ "action": "offer_draw" }
```

```json
{ "action": "accept_draw" }
```

---

### Get Legal Moves

```http
GET /api/games/{id}/moves
```

Returns all legal moves for the current side to move.

**Response** `200 OK`:

```json
{
  "moves": [
    { "from": "e2", "to": "e4", "promotion": null },
    { "from": "e2", "to": "e3", "promotion": null },
    { "from": "d2", "to": "d4", "promotion": null }
  ]
}
```

---

### Get ASCII Board

```http
GET /api/games/{id}/board
```

Returns a text representation of the current board.

**Response** `200 OK`:

```bash
  a b c d e f g h
8 r n b q k b n r 8
7 p p p p p p p p 7
6 . . . . . . . . 6
5 . . . . . . . . 5
4 . . . . . . . . 4
3 . . . . . . . . 3
2 P P P P P P P P 2
1 R N B Q K B N R 1
  a b c d e f g h
```

---

## FEN & PGN Endpoints

### Export FEN

```http
GET /api/games/{id}/fen
```

Returns the current position in full FEN notation (6 fields: piece placement, turn, castling, en passant, halfmove clock, fullmove number).

**Response** `200 OK`:

```json
{
  "fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
}
```

---

### Import FEN

```http
POST /api/games/fen
```

Creates a new game from a FEN string. The FEN must contain all 6 fields.

**Request Body**:

```json
{
  "fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
}
```

**Response** `200 OK`:

```json
{
  "game_id": "550e8400-e29b-41d4-a716-446655440000",
  "message": "Game created from FEN."
}
```

**Response** `400 Bad Request`:

```json
{
  "error": "Invalid FEN: expected 6 space-separated fields"
}
```

---

### Export PGN

```http
GET /api/games/{id}/pgn
```

Returns the game in PGN (Portable Game Notation) format with standard Seven Tag Roster headers.

**Response** `200 OK`:

```json
{
  "pgn": "[Event \"CheckAI Game\"]\n[Site \"CheckAI\"]\n[Date \"2025.03.05\"]\n[Round \"?\"]\n[White \"Player\"]\n[Black \"Player\"]\n[Result \"*\"]\n\n1. e2e4 e7e5 2. g1f3 *"
}
```

---

## Archive & Storage

Completed games are compressed and persisted to disk. The archive endpoints
list, retrieve, and replay archived games. The matching WebSocket actions
are documented under [WebSocket → Archive](./websocket.md).

### List Archived Games

```http
GET /api/archive
```

Returns summaries of all archived games together with overall storage
statistics.

**Response** `200 OK`:

```json
{
  "games": [
    {
      "game_id": "550e8400-e29b-41d4-a716-446655440000",
      "move_count": 42,
      "result": "white_wins",
      "end_reason": "checkmate",
      "start_timestamp": 1731000000,
      "end_timestamp": 1731003600,
      "compressed_bytes": 312,
      "raw_bytes": 1024
    }
  ],
  "total": 1,
  "storage": {
    "active_count": 0,
    "archived_count": 1,
    "active_bytes": 0,
    "archive_bytes": 312,
    "total_bytes": 312
  }
}
```

---

### Storage Statistics

```http
GET /api/archive/stats
```

Returns disk-usage statistics for both active and archived game files.

**Response** `200 OK`:

```json
{
  "active_count": 3,
  "archived_count": 42,
  "active_bytes": 8192,
  "archive_bytes": 73216,
  "total_bytes": 81408
}
```

---

### Get Archived Game

```http
GET /api/archive/{game_id}
```

Loads a completed game from the archive and returns the **final** position
together with the full game metadata. Use the replay endpoint below to
reconstruct any earlier position.

**Path Parameters**:

| Name      | Type   | Description               |
| --------- | ------ | ------------------------- |
| `game_id` | string | UUID of the archived game |

**Response** `200 OK`:

```json
{
  "game_id": "550e8400-e29b-41d4-a716-446655440000",
  "at_move": 42,
  "total_moves": 42,
  "state": {
    "board": { "a1": "R", "e1": "K", "h8": "k" },
    "turn": "black",
    "castling": {
      "white": { "kingside": false, "queenside": false },
      "black": { "kingside": false, "queenside": false }
    },
    "en_passant": null,
    "halfmove_clock": 0,
    "fullmove_number": 22,
    "status": "white_wins",
    "move_history": []
  },
  "is_over": true,
  "result": "white_wins",
  "is_check": true
}
```

**Errors**:

| Status            | Cause                           |
| ----------------- | ------------------------------- |
| `400 Bad Request` | `game_id` is not a valid UUID   |
| `404 Not Found`   | No archived game with this UUID |

---

### Replay Archived Game

```http
GET /api/archive/{game_id}/replay?move_number={n}
```

Reconstructs the exact position after `move_number` half-moves of the
archived game. This is the primary endpoint for post-game analysis.

**Path Parameters**:

| Name      | Type   | Description               |
| --------- | ------ | ------------------------- |
| `game_id` | string | UUID of the archived game |

**Query Parameters**:

| Name          | Type    | Default        | Description                                                      |
| ------------- | ------- | -------------- | ---------------------------------------------------------------- |
| `move_number` | integer | final position | Half-move index to replay to. `0` returns the starting position. |

**Response** `200 OK`: same shape as `GET /api/archive/{game_id}` —
`at_move` reflects the replayed half-move index (clamped to
`total_moves` when the requested number exceeds the game length).

**Example**:

```bash
curl "http://localhost:8080/api/archive/550e8400-e29b-41d4-a716-446655440000/replay?move_number=10"
```

**Errors**: same as `GET /api/archive/{game_id}`.

---

## Localization

All API responses respect the requested locale:

```bash
# Query parameter
curl http://localhost:8080/api/games?lang=de

# Header
curl -H "Accept-Language: fr" http://localhost:8080/api/games
```

## Swagger UI

Interactive API documentation is available at:

```bash
http://localhost:8080/swagger-ui/
```

## Error Responses

All error responses return a JSON object with an `error` field:

```json
{
  "error": "Description of what went wrong"
}
```

| Status Code                 | Meaning                                               | Example                           |
| --------------------------- | ----------------------------------------------------- | --------------------------------- |
| `400 Bad Request`           | Invalid input (bad FEN, illegal move, missing fields) | `{"error": "Illegal move: e2e5"}` |
| `404 Not Found`             | Game or resource does not exist                       | `{"error": "Game not found"}`     |
| `500 Internal Server Error` | Unexpected server error                               | `{"error": "Internal error"}`     |
