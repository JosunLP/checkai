# Analysis API

The Analysis API provides endpoints for submitting chess games for deep engine analysis and retrieving results.

These endpoints are architecturally isolated from the player-facing game API.

## Base URL

```bash
http://localhost:8080/api/analysis
```

## Two ways to analyse

The API has two distinct modes, and picking the right one matters:

| | `POST /api/analysis/position` | `POST /api/analysis/game/{id}` |
| --- | --- | --- |
| Shape | Synchronous — answers in the same request | Asynchronous job you poll |
| Scope | One position | Every move of a game |
| Typical budget | 0.2–2 seconds | Minutes |
| Returns | Evaluation, best move, MultiPV lines, book, tablebase | Per-move annotations and a summary |
| Use for | Live evaluation bars, hint buttons, agent decisions | Post-game review |

The job-based mode is **game-review oriented**, not a live search-info stream:
while a job runs you receive its `status` only; `result` appears when it
completes. It does not expose a rolling score, node count or principal
variation. For anything interactive, use the position endpoint instead.

## Endpoints

### Analyse a Single Position

```http
POST /api/analysis/position
Content-Type: application/json
```

Runs one bounded search and returns the verdict immediately. Identify the
position with either `fen` or `game_id`.

**Request**

```json
{
  "fen": "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 4 4",
  "movetime_ms": 700,
  "multi_pv": 3,
  "threads": 2
}
```

| Field         | Type   | Default | Description                                     |
| ------------- | ------ | ------- | ----------------------------------------------- |
| `fen`         | string | —       | Position to analyse (4–6 field FEN)             |
| `game_id`     | string | —       | Analyse the current position of an active game  |
| `depth`       | number | 128     | Maximum search depth in plies                   |
| `movetime_ms` | number | 1000    | Time budget, capped at 60 000                   |
| `multi_pv`    | number | 1       | Number of principal variations (1–16)           |
| `threads`     | number | 1       | Lazy SMP search threads (1–64)                  |

**Response** `200 OK`

```json
{
  "fen": "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 4 4",
  "turn": "white",
  "best_move": { "from": "f3", "to": "f7", "promotion": null },
  "score_cp": 29999,
  "score_white_cp": 29999,
  "mate_in": 1,
  "static_eval_cp": 2,
  "depth": 8,
  "seldepth": 24,
  "nodes": 194560,
  "nps": 276363,
  "time_ms": 704,
  "hashfull": 17,
  "source": "search",
  "lines": [
    { "rank": 1, "score_cp": 29999, "score_white_cp": 29999, "mate_in": 1, "moves": ["f3f7"] },
    { "rank": 2, "score_cp": 35, "score_white_cp": 35, "mate_in": null, "moves": ["c4f7", "e8e7", "f7g8"] }
  ]
}
```

`score_white_cp` is always from White's point of view, which is what an
evaluation bar wants; `score_cp` is relative to the side to move. `source` is
`search`, `book` or `tablebase`. When a book or tablebase is configured on the
server, the response also carries `book` and `tablebase` objects.

**Errors**

| Status | Cause                                        |
| ------ | -------------------------------------------- |
| `400`  | Invalid FEN, or neither `fen` nor `game_id`  |
| `404`  | `game_id` does not exist                     |

### Submit Game for Analysis

```http
POST /api/analysis/game/{game_id}
Content-Type: application/json
```

Submits a completed game for asynchronous deep analysis.

**Request Body**:

```json
{
  "depth": 30
}
```

| Field   | Type   | Default | Description                 |
| ------- | ------ | ------- | --------------------------- |
| `depth` | number | 30      | Minimum search depth (≥ 30) |

**Response** `202 Accepted`:

```json
{
  "job_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "message": "Analysis submitted for game 550e8400-... (42 moves)"
}
```

---

### List Analysis Jobs

```http
GET /api/analysis/jobs
```

Returns all analysis jobs with their current status.

**Response** `200 OK`:

```json
{
  "jobs": [
    {
      "id": "a1b2c3d4-...",
      "game_id": "550e8400-...",
      "status": "Completed",
      "created_at": 1709337600,
      "completed_at": 1709337660
    },
    {
      "id": "b2c3d4e5-...",
      "game_id": "661f9511-...",
      "status": { "InProgress": { "moves_analyzed": 18, "total_moves": 30 } },
      "created_at": 1709337650
    }
  ],
  "count": 2
}
```

**Job status variants**:

| Status       | JSON representation                                           |
| ------------ | ------------------------------------------------------------- |
| `Queued`     | `"Queued"`                                                    |
| `InProgress` | `{ "InProgress": { "moves_analyzed": N, "total_moves": M } }` |
| `Completed`  | `"Completed"`                                                 |
| `Failed`     | `{ "Failed": { "error": "..." } }`                            |
| `Cancelled`  | `"Cancelled"`                                                 |

---

### Get Job Status and Results

```http
GET /api/analysis/jobs/{job_id}
```

Returns the full analysis job, including results when the job is completed.

While a job is still running, the response shape looks like this:

```json
{
  "id": "a1b2c3d4-...",
  "game_id": "550e8400-...",
  "status": { "InProgress": { "moves_analyzed": 18, "total_moves": 30 } },
  "created_at": 1709337600
}
```

**Response** `200 OK` (completed):

```json
{
  "id": "a1b2c3d4-...",
  "game_id": "550e8400-...",
  "status": "Completed",
  "result": {
    "annotations": [
      {
        "move_number": 1,
        "side": "white",
        "played_move": { "from": "e2", "to": "e4" },
        "best_move": { "from": "e2", "to": "e4" },
        "played_eval": 20,
        "best_eval": 20,
        "centipawn_loss": 0,
        "quality": "Best",
        "is_book_move": false,
        "is_tablebase_position": false,
        "search_depth": 30,
        "principal_variation": ["e2e4", "e7e5", "g1f3"]
      },
      {
        "move_number": 1,
        "side": "black",
        "played_move": { "from": "e7", "to": "e5" },
        "best_move": { "from": "e7", "to": "e5" },
        "played_eval": -20,
        "best_eval": -20,
        "centipawn_loss": 0,
        "quality": "Best",
        "is_book_move": false,
        "is_tablebase_position": false,
        "search_depth": 30,
        "principal_variation": ["e7e5", "g1f3", "b8c6"]
      }
    ],
    "summary": {
      "total_moves": 42,
      "best_moves": 12,
      "excellent_moves": 5,
      "good_moves": 3,
      "inaccuracies": 1,
      "mistakes": 0,
      "blunders": 0,
      "book_moves": 0,
      "average_centipawn_loss": 8.4,
      "white_accuracy": 85.5,
      "black_accuracy": 78.2,
      "white_avg_cp_loss": 6.2,
      "black_avg_cp_loss": 10.6
    },
    "depth": 30,
    "book_available": false,
    "tablebase_available": false
  },
  "created_at": 1709337600,
  "completed_at": 1709337660
}
```

---

### Cancel or Delete a Job

```http
DELETE /api/analysis/jobs/{job_id}
```

Cancels an in-progress job or deletes a completed one.

**Response** `200 OK`:

```json
{
  "message": "Job a1b2c3d4-... deleted"
}
```

## Move Classification Reference

| Classification | Centipawn Loss | Symbol |
| -------------- | -------------- | ------ |
| Best           | 0 cp           | !!     |
| Excellent      | ≤ 10 cp        | !      |
| Good           | 11–25 cp       | —      |
| Inaccuracy     | 26–50 cp       | ?!     |
| Mistake        | 51–100 cp      | ?      |
| Blunder        | > 100 cp       | ??     |
| Book           | n/a            | 📖      |

`Book` is emitted when the played move matches the configured opening book and is therefore not graded against deep search.

## Workflow

```bash
1. Play a game via /api/games/* endpoints
2. When the game ends, submit it for analysis:
   POST /api/analysis/game/{game_id}
3. Poll for progress:
   GET /api/analysis/jobs/{job_id}
4. When status is "completed", read the results
5. Optionally clean up:
   DELETE /api/analysis/jobs/{job_id}
```
