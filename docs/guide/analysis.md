# Analysis Engine

CheckAI includes a deep game analysis engine that evaluates completed chess games move by move.

## Overview

The analysis engine runs asynchronously and provides:

- **Search depth** of 30+ plies (configurable)
- **Move classification**: Best, Excellent, Good, Inaccuracy, Mistake, Blunder
- **Centipawn loss** per move
- **Principal variation** (best continuation) per move
- **Accuracy percentage** per side

## Search Algorithm

The engine uses a sophisticated search stack:

| Technique                  | Description                                                       |
| -------------------------- | ----------------------------------------------------------------- |
| Alpha-Beta with PVS        | Principal Variation Search (Negascout) for efficient tree pruning |
| Transposition Table        | Configurable (default 64 MB) Zobrist-hashed position cache        |
| Null-Move Pruning (NMP)    | Skip a turn to quickly detect strong positions                    |
| Late Move Reductions (LMR) | Reduce depth for unlikely moves                                   |
| Killer Heuristic           | Prioritize moves that caused cutoffs at the same depth            |
| History Heuristic          | Score moves by how often they caused cutoffs globally             |
| Quiescence Search          | Extend search through capture sequences to avoid horizon effects  |

## Evaluation

The position evaluation is based on **PeSTO** (Piece-Square Tables Only):

- **Midgame tables** — Piece-specific positional values for the opening/middlegame
- **Endgame tables** — Adjusted values for the endgame phase
- **Phase interpolation** — Smooth transition between midgame and endgame evaluation
- **Pawn structure** — Bonus/penalty for doubled, isolated, and passed pawns
- **Bishop pair** — Bonus for retaining both bishops

## Move Classification

Each move is classified based on centipawn loss compared to the engine's best move:

| Classification | Centipawn Loss | Description                           |
| -------------- | -------------- | ------------------------------------- |
| **Best**       | 0 cp           | The engine's top choice               |
| **Excellent**  | ≤ 10 cp        | Nearly optimal                        |
| **Good**       | ≤ 25 cp        | Solid, no significant loss            |
| **Inaccuracy** | ≤ 50 cp        | Slight imprecision                    |
| **Mistake**    | ≤ 100 cp       | Notable error, roughly a pawn's worth |
| **Blunder**    | > 100 cp       | Serious mistake, potentially losing   |

## Usage

Submit a completed game for analysis via the [Analysis API](../api/analysis):

```bash
# Submit for analysis
curl -X POST http://localhost:8080/api/analysis/game/{game_id} \
  -H "Content-Type: application/json" \
  -d '{"depth": 30}'

# Check progress
curl http://localhost:8080/api/analysis/jobs/{job_id}
```

## Configuration

```bash
checkai serve \
  --analysis-depth 35 \
  --tt-size-mb 128 \
  --book-path books/book.bin \
  --tablebase-path tablebase/
```

See [Configuration](./configuration) for all options.
