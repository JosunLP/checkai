# Configuration

CheckAI can be configured through CLI flags, environment variables, and request-level parameters.

## Server Configuration

All server settings are passed as CLI flags to `checkai serve`:

| Setting            | CLI Flag                 | Default   | Description                                   |
| ------------------ | ------------------------ | --------- | --------------------------------------------- |
| Port               | `--port`                 | `8080`    | HTTP server port                              |
| Host               | `--host`                 | `0.0.0.0` | Bind address                                  |
| Data directory     | `--data-dir`             | `data`    | Storage for active/archived games             |
| Opening book       | `--book-path`            | —         | Polyglot `.bin` file                          |
| Tablebase          | `--tablebase-path`       | —         | Syzygy tablebase directory                    |
| Analysis depth     | `--analysis-depth`       | `30`      | Minimum plies for analysis engine             |
| TT size            | `--tt-size-mb`           | `64`      | Transposition table memory in MB              |
| Max retained jobs  | `--max-retained-jobs`    | —         | Maximum number of completed analysis jobs kept |
| Max concurrent jobs| `--max-concurrent-jobs`  | —         | Maximum number of analysis jobs run in parallel |
| Completed-job TTL  | `--completed-job-ttl`    | —         | Time-to-live for completed analysis jobs (e.g. `24h`) |

## Environment Variables

| Variable       | Description                                           |
| -------------- | ----------------------------------------------------- |
| `CHECKAI_LANG` | Override locale (e.g. `de`, `fr`, `es`)               |
| `RUST_LOG`     | Log level (`error`, `warn`, `info`, `debug`, `trace`) |
| `CHECKAI_PORT` | Port when using Docker Compose                        |

## Language / Locale

The locale is resolved in priority order:

1. **CLI flag**: `--lang de`
2. **Environment variable**: `CHECKAI_LANG=de`
3. **System locale**: auto-detected via `sys-locale`
4. **Fallback**: `en`

### Per-Request Locale (API)

API responses can be localized per request:

```bash
# Via query parameter
curl http://localhost:8080/api/games?lang=de

# Via Accept-Language header
curl -H "Accept-Language: de" http://localhost:8080/api/games
```

## Data Directory Structure

```bash
data/
├── active/     # Currently running games (binary format)
└── archive/    # Completed games (zstd-compressed)
```

Games are automatically moved from `active/` to `archive/` when they end (checkmate, draw, resignation). Archived games are compressed with zstd for efficient storage.

## Analysis Configuration

The analysis engine settings control the depth and memory used for game analysis:

| Parameter        | Min | Recommended | Description                             |
| ---------------- | --- | ----------- | --------------------------------------- |
| `analysis-depth` | 30  | 30–40       | Deeper = slower but more accurate       |
| `tt-size-mb`     | 1   | 64–256      | Larger = fewer transposition collisions |

::: warning Performance Note
Analysis depth above 35 can take significantly longer per move. A depth of 30 is sufficient for most use cases and provides move classifications (Best through Blunder) with centipawn accuracy.
:::
