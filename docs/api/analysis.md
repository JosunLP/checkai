# Analysis API

The Analysis API provides endpoints for submitting chess games for deep engine analysis and retrieving results.

These endpoints are architecturally isolated from the player-facing game API.

## Base URL

```bash
http://localhost:8080/api/analysis
```

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

**Response** `200 OK`:

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
[
  {
    "job_id": "a1b2c3d4-...",
    "game_id": "550e8400-...",
    "status": "completed",
    "progress": 100,
    "total_moves": 42
  },
  {
    "job_id": "b2c3d4e5-...",
    "game_id": "661f9511-...",
    "status": "in_progress",
    "progress": 60,
    "total_moves": 30
  }
]
```

---

### Get Job Status and Results

```http
GET /api/analysis/jobs/{job_id}
```

Returns detailed analysis results when the job is completed.

**Response** `200 OK` (completed):

```json
{
  "job_id": "a1b2c3d4-...",
  "game_id": "550e8400-...",
  "status": "completed",
  "depth": 30,
  "results": {
    "moves": [
      {
        "move_number": 1,
        "side": "white",
        "move": "e2e4",
        "classification": "Best",
        "centipawn_loss": 0,
        "eval_before": 20,
        "eval_after": 20,
        "best_move": "e2e4",
        "principal_variation": ["e2e4", "e7e5", "g1f3"]
      },
      {
        "move_number": 1,
        "side": "black",
        "move": "e7e5",
        "classification": "Best",
        "centipawn_loss": 0,
        "eval_before": -20,
        "eval_after": -20,
        "best_move": "e7e5",
        "principal_variation": ["e7e5", "g1f3", "b8c6"]
      }
    ],
    "white_accuracy": 85.5,
    "black_accuracy": 78.2,
    "summary": {
      "white": {
        "best": 12,
        "excellent": 5,
        "good": 3,
        "inaccuracy": 1,
        "mistake": 0,
        "blunder": 0
      },
      "black": {
        "best": 10,
        "excellent": 4,
        "good": 4,
        "inaccuracy": 2,
        "mistake": 1,
        "blunder": 0
      }
    }
  }
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
  "message": "Analysis job deleted."
}
```

## Move Classification Reference

| Classification | Centipawn Loss | Symbol |
| -------------- | -------------- | ------ |
| Best           | 0 cp           | !!     |
| Excellent      | ≤ 10 cp        | !      |
| Good           | ≤ 25 cp        | —      |
| Inaccuracy     | ≤ 50 cp        | ?!     |
| Mistake        | ≤ 100 cp       | ?      |
| Blunder        | > 100 cp       | ??     |

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
