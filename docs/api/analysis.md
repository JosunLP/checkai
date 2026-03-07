# Analysis API

The Analysis API provides endpoints for submitting chess games for deep engine analysis and retrieving results.

These endpoints are architecturally isolated from the player-facing game API.

## Base URL

```bash
http://localhost:8080/api/analysis
```

## What the API Returns

The analysis API is **game-review oriented**, not a live search-info stream.

- While a job is running, you receive the job `status` only.
- When the job completes, `result` contains per-move annotations and a summary.
- The API does **not** expose live `score`, `nodes`, `nps`, `time_ms`, or a rolling principal variation while the job is in progress.

That distinction matters for web clients: poll the job endpoint for status, then read `result.summary` and `result.annotations` after completion.

## Endpoints

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
