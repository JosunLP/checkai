# Endgame Tablebases

CheckAI supports **Syzygy tablebase integration scaffolding** in the analysis engine.

## What are Syzygy Tablebases?

Syzygy tablebases contain precomputed optimal play for all positions with a small number of pieces (typically up to 6 or 7). In a full Syzygy integration, an engine can read exact game-theoretic outcomes (WDL) and distance-to-zeroing (DTZ) values from `.rtbw`/`.rtbz` files.

## Current Status in CheckAI

At the moment, CheckAI does **not** perform binary probing of `.rtbw`/`.rtbz` files yet.

- If a simple endgame is analytically solved by built-in rules, CheckAI returns that result (source: `analytical`).
- If matching Syzygy files are present for a position, CheckAI currently logs that binary probing is not implemented, then falls back to a material-based heuristic WDL estimate (source: `heuristic`).
- Heuristic fallback results are **not** treated as true tablebase hits (`is_tablebase_position = false`).

In other words: external Syzygy files currently improve **coverage metadata** and future readiness, but they do **not** yet provide perfect-play WDL/DTZ probing inside CheckAI.

## Analytical Probing

CheckAI includes built-in analytical probing for common endgames. This is applied when a tablebase path is configured (via `--tablebase-path`) for positions where analytical results are provably correct, even if no corresponding `.rtbw`/`.rtbz` file is present:

| Endgame | Result                |
| ------- | --------------------- |
| K vs K  | Always drawn          |
| KR vs K | Win for stronger side |
| KQ vs K | Win for stronger side |

To enable this analytical probing path (and table file discovery), configure a tablebase path as described below.

## Setup with External Files

External Syzygy files are currently optional. Today they provide:

- Discovery/indexing of available `.rtbw`/`.rtbz` configurations
- Piece-count coverage/range information
- Forward compatibility for future full binary probing

They do **not yet** provide exact WDL/DTZ probing in CheckAI.

To configure external files anyway (recommended for future compatibility):

1. Download Syzygy tablebase files (`.rtbw` for WDL, `.rtbz` for DTZ)
2. Place them in a directory, e.g. `tablebase/`
3. Start the server with the `--tablebase-path` flag:

```bash
checkai serve --tablebase-path tablebase/
```

### Docker

```yaml
volumes:
  - ./tablebase:/home/checkai/tablebase:ro
command:
  - serve
  - --tablebase-path
  - tablebase
```

## File Formats

| Extension | Purpose                   | Description                         |
| --------- | ------------------------- | ----------------------------------- |
| `.rtbw`   | Win/Draw/Loss             | Game-theoretic result per position  |
| `.rtbz`   | Distance to Zeroing (DTZ) | Optimal number of moves to progress |

## Sources

Syzygy tablebases can be downloaded from:

- [Lichess Syzygy tables](https://tablebase.lichess.ovh/)
- [Syzygy GitHub](https://github.com/syzygy1/tb)

::: tip Size Considerations
3-4 piece tablebases are small (< 1 GB). 5-piece tablebases are ~1 GB. 6-piece tablebases are ~150 GB. 7-piece tablebases are ~140 TB. Choose what fits your storage.
:::
