# Endgame Tablebases

CheckAI supports **Syzygy endgame tablebases** for perfect endgame play in the analysis engine.

## What are Syzygy Tablebases?

Syzygy tablebases contain precomputed optimal play for all positions with a small number of pieces (typically up to 6 or 7). When a position matches a tablebase entry, the engine knows the exact game-theoretic result (win/draw/loss) and the optimal move.

## Built-in Analytical Probing

Even without external tablebase files, CheckAI includes built-in analytical probing for common endgames:

| Endgame | Result                |
| ------- | --------------------- |
| K vs K  | Always drawn          |
| KR vs K | Win for stronger side |
| KQ vs K | Win for stronger side |

These are detected automatically and do not require any configuration.

## Setup with External Files

For comprehensive tablebase coverage:

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
