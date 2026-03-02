# CLI Commands

CheckAI provides three main commands: `serve`, `play`, and `export`.

## Global Options

| Flag            | Description                                |
| --------------- | ------------------------------------------ |
| `--lang <LANG>` | Override locale (e.g. `de`, `fr`, `zh-CN`) |
| `--help`        | Print help information                     |
| `--version`     | Print version information                  |

The language is auto-detected from:

1. `--lang` CLI flag
2. `CHECKAI_LANG` environment variable
3. System locale
4. Fallback: English

## `checkai serve`

Start the REST API server with WebSocket support and Swagger UI.

```bash
checkai serve [OPTIONS]
```

| Option                     | Default   | Description                              |
| -------------------------- | --------- | ---------------------------------------- |
| `-p, --port <PORT>`        | `8080`    | Port to listen on                        |
| `--host <HOST>`            | `0.0.0.0` | Host address to bind to                  |
| `--data-dir <DIR>`         | `data`    | Directory for game storage               |
| `--book-path <PATH>`       | —         | Path to Polyglot opening book (`.bin`)   |
| `--tablebase-path <PATH>`  | —         | Path to Syzygy tablebase directory       |
| `--analysis-depth <DEPTH>` | `30`      | Minimum search depth for analysis (≥ 30) |
| `--tt-size-mb <SIZE>`      | `64`      | Transposition table size in MB           |

### Examples

```bash
# Default server
checkai serve

# Custom port with German locale
checkai serve --port 3000 --lang de

# With opening book and tablebase
checkai serve --book-path books/book.bin --tablebase-path tablebase/

# Deep analysis with large transposition table
checkai serve --analysis-depth 40 --tt-size-mb 256
```

## `checkai play`

Start an interactive two-player terminal game.

```bash
checkai play
```

### Terminal Commands

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

## `checkai export`

Export archived games in human-readable format.

```bash
checkai export [OPTIONS]
```

| Option               | Default | Description                          |
| -------------------- | ------- | ------------------------------------ |
| `--data-dir <DIR>`   | `data`  | Directory for game storage           |
| `-f, --format <FMT>` | `text`  | Output format: `text`, `pgn`, `json` |
| `-g, --game-id <ID>` | —       | Export a specific game by UUID       |
| `-l, --list`         | —       | List all archived games              |
| `-a, --all`          | —       | Export all archived games            |

### Examples exporting games

```bash
# List all archived games
checkai export --list

# Export a specific game as PGN
checkai export --game-id 550e8400-... --format pgn

# Export all games as JSON
checkai export --all --format json
```

## `checkai update`

Check for updates and self-update the binary.

```bash
checkai update
```

This downloads the latest release from GitHub and replaces the current binary in-place. Works on Linux, macOS, and Windows.

::: tip Automatic Update Check
CheckAI checks for new versions automatically on startup and notifies you if an update is available.
:::
