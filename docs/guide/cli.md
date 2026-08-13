# CLI Commands

CheckAI is one binary with eleven commands. Running it with no arguments prints
an animated welcome screen listing them all.

| Command   | What it does                                              |
| --------- | --------------------------------------------------------- |
| `serve`   | REST + WebSocket API server with Swagger UI and web UI    |
| `play`    | Play in the terminal, against the engine or a human       |
| `watch`   | Watch the engine play itself                              |
| `analyze` | Annotate a position, a move list or a PGN file            |
| `eval`    | Inspect the evaluation, ranked moves, book and tablebase   |
| `bench`   | Fixed twelve-position benchmark suite                     |
| `perft`   | Verify move generation against known node counts          |
| `uci`     | Speak UCI on stdin/stdout for chess GUIs                  |
| `export`  | Export archived games as text, PGN or JSON                |
| `update`  | Update to the latest release from GitHub                  |
| `version` | Print the current version                                 |

## Global options

| Flag            | Description                                    |
| --------------- | ---------------------------------------------- |
| `--lang <LANG>` | Override locale (e.g. `de`, `fr`, `zh-CN`)     |
| `--no-color`    | Disable colour (the `NO_COLOR` env var works too) |
| `--help`        | Print help information                         |
| `--version`     | Print version information                      |

The language is auto-detected from:

1. `--lang` CLI flag
2. `CHECKAI_LANG` environment variable
3. System locale
4. Fallback: English

## Engine options

Every command that runs a search — `play`, `watch`, `analyze`, `eval`, `bench` —
accepts the same engine option group:

| Flag              | Description                                                | Default     |
| ----------------- | ---------------------------------------------------------- | ----------- |
| `--threads <N>`   | Lazy SMP search threads; `0` = one per CPU core             | `1`         |
| `--hash <MB>`     | Transposition table size                                     | per command |
| `--nodes <N>`     | Node budget per search                                       | unlimited   |
| `--multipv <N>`   | Report the best N lines (1–16)                               | `1`         |
| `--book <FILE>`   | Polyglot opening book (`.bin`)                               | none        |
| `--book-best`     | Always play the most popular book move instead of sampling   | off         |
| `--tablebase <D>` | Syzygy tablebase directory                                   | none        |

See [The Search Engine](/guide/engine) for what these actually do.

## `checkai serve`

Start the REST API server with WebSocket support, Swagger UI and the embedded
web UI.

```bash
checkai serve [OPTIONS]
```

| Option                                 | Default   | Description                                                              |
| -------------------------------------- | --------- | ------------------------------------------------------------------------ |
| `-p, --port <PORT>`                    | `8080`    | Port to listen on                                                        |
| `--host <HOST>`                        | `0.0.0.0` | Host address to bind to                                                  |
| `--data-dir <DIR>`                     | `data`    | Directory for game storage                                               |
| `--book-path <PATH>`                   | —         | Path to Polyglot opening book (`.bin`)                                   |
| `--tablebase-path <PATH>`              | —         | Path to Syzygy tablebase directory                                       |
| `--analysis-depth <DEPTH>`             | `30`      | Minimum search depth for game analysis (≥ 30)                            |
| `--tt-size-mb <SIZE>`                  | `64`      | Transposition table size in MB                                           |
| `--analysis-max-jobs <N>`              | `256`     | Maximum number of analysis jobs kept in memory                           |
| `--analysis-max-concurrent-jobs <N>`   | `4`       | Maximum analysis jobs to run in parallel                                 |
| `--analysis-completed-ttl-secs <SECS>` | `3600`    | Time-to-live for completed analysis jobs                                 |
| `--analysis-position-max-threads <N>`  | `4`       | Search threads one live position analysis may use                        |
| `--analysis-position-max-movetime-ms <MS>` | `10000` | Longest time budget for one live position analysis                     |
| `--analysis-max-concurrent-positions <N>` | `4`    | Live position analyses allowed to run at the same time                   |

```bash
checkai serve                                          # default
checkai serve --port 3000 --lang de                    # custom port, German
checkai serve --book-path book.bin --tablebase-path tb/ # with knowledge
```

## `checkai play`

Play in the terminal. By default you take White against the engine at level 5.

```bash
checkai play [OPTIONS]
```

| Option              | Default | Description                                            |
| ------------------- | ------- | ------------------------------------------------------ |
| `--vs <WHO>`        | `engine`| `engine` or `human` (two players share the terminal)   |
| `--color <SIDE>`    | `white` | `white`, `black` or `random`                           |
| `--level <1-10>`    | `5`     | Engine difficulty                                      |
| `--movetime <MS>`   | ladder  | Override thinking time per move                        |
| `--depth <N>`       | ladder  | Override maximum search depth                          |
| `--time <SPEC>`     | —       | Time control, e.g. `5+3`, `90+30`, `30s`               |
| `--fen <FEN>`       | startpos| Start from a custom position                           |
| `--pgn <FILE>`      | —       | Resume a game from a PGN file                          |
| `--board <THEME>`   | `wood`  | `wood`, `ice`, `club`, `mono` or `ascii`               |
| `--ascii`           | off     | ASCII piece letters instead of Unicode glyphs          |
| `--flip`            | off     | Render from Black's perspective                        |
| `--no-animation`    | off     | Disable the move animation                             |

```bash
checkai play --level 9                    # a much stronger opponent
checkai play --time 5+3                   # five minutes plus 3s increment
checkai play --color black --board ice    # play Black on a blue board
checkai play --book book.bin --threads 4  # give the engine book and cores
checkai play --pgn game.pgn               # continue a saved game
```

### In-game commands

Moves are accepted in coordinate notation (`e2e4`, `e7e8q`) **or** standard
algebraic notation (`e4`, `Nf3`, `exd5`, `O-O`, `Qh4#`).

| Command       | Alias   | Description                              |
| ------------- | ------- | ---------------------------------------- |
| `moves`       | `m`     | List all legal moves in SAN              |
| `board`       | `b`     | Redraw the board                         |
| `flip`        |         | Flip the board orientation               |
| `history`     | `hist`  | Numbered move history                    |
| `fen`         | `f`     | Print the current FEN                    |
| `pgn`         |         | Print the game as PGN                    |
| `json`        | `j`     | Print the game state as JSON             |
| `hint`        | `i`     | Ask the engine for a suggestion          |
| `analyze`     | `a`     | Deeper multi-line analysis               |
| `eval`        | `e`     | Static evaluation breakdown              |
| `book`        |         | Opening-book moves for this position     |
| `tb`          |         | Endgame tablebase verdict                |
| `undo`        | `u`     | Take back the last full move             |
| `redo`        |         | Replay a move that was taken back        |
| `level N`     | `l`     | Change the engine level mid-game         |
| `save [file]` | `s`     | Save the game as PGN                     |
| `load <file>` | `o`     | Load a game from PGN                     |
| `new`         |         | Start a fresh game                       |
| `resign`      | `r`     | Resign                                   |
| `draw`        | `d`     | Claim a draw when eligible               |
| `help`        | `h`     | Show the command table                   |
| `quit`        | `q`     | Leave the session                        |

### Difficulty levels

| Level | Max depth | Move time | Hash   | Skill | Feels like         |
| ----- | --------- | --------- | ------ | ----- | ------------------ |
| 1     | 2         | 60 ms     | 4 MB   | 2     | absolute beginner  |
| 2     | 3         | 120 ms    | 8 MB   | 5     | casual club player |
| 3     | 4         | 250 ms    | 16 MB  | 8     | improving amateur  |
| 4     | 6         | 500 ms    | 32 MB  | 11    | solid club player  |
| 5     | 10        | 1000 ms   | 64 MB  | 14    | strong club player |
| 6     | 14        | 2000 ms   | 64 MB  | 17    | expert             |
| 7     | full      | 3000 ms   | 128 MB | full  | master             |
| 8     | full      | 5000 ms   | 128 MB | full  | strong master      |
| 9     | full      | 7500 ms   | 256 MB | full  | very strong        |
| 10    | full      | 10000 ms  | 256 MB | full  | full strength      |

## `checkai watch`

Watch two engine instances play each other.

```bash
checkai watch [OPTIONS]
```

| Option                | Default | Description                                          |
| --------------------- | ------- | ---------------------------------------------------- |
| `--level <1-10>`      | —       | Same level for both sides                            |
| `--level-white <1-10>`| `5`     | Level for White                                      |
| `--level-black <1-10>`| `5`     | Level for Black                                      |
| `--movetime <MS>`     | ladder  | Thinking time per move                               |
| `--time <SPEC>`       | —       | Play the whole game on a clock, e.g. `3+2`           |
| `--fen <FEN>`         | startpos| Start from a custom position                         |
| `--max-moves <N>`     | `200`   | Stop after this many full moves                      |
| `--adjudicate <CP>`   | `0`     | End the game once one side is this far ahead         |
| `--delay <MS>`        | `800`   | Pause between moves                                  |
| `--pgn-out <FILE>`    | —       | Write the finished game as PGN                       |
| `--quiet`             | off     | Only print the move ticker                           |
| `--board <THEME>`     | `wood`  | Board colour palette                                 |

```bash
checkai watch --level-white 9 --level-black 3     # an uneven match
checkai watch --time 1+0 --delay 0                # bullet, no pauses
checkai watch --adjudicate 900 --pgn-out game.pgn # stop early, save the game
```

## `checkai analyze`

Annotate a position, a move list or a whole PGN file.

```bash
checkai analyze [OPTIONS]
```

| Option            | Description                                             |
| ----------------- | ------------------------------------------------------- |
| `--fen <FEN>`     | Position to analyse (or the start position for a game)  |
| `--moves <LIST>`  | Space-separated moves, coordinate or SAN                |
| `--pgn <FILE>`    | PGN file to import and annotate                         |
| `--depth <N>`     | Fixed search depth                                      |
| `--movetime <MS>` | Time budget (per move in game mode)                     |

Plus the shared [engine options](#engine-options); `--multipv <N>` reports the
best N lines in position mode.

```bash
checkai analyze --fen "<FEN>" --multipv 4     # the four best lines
checkai analyze --pgn game.pgn                # annotate a whole game
checkai analyze --moves "e4 e5 Nf3 Nc6 Bb5"   # annotate a move list
```

Game analysis reports, per move: the evaluation after it, its centipawn loss, a
`!`/`?!`/`?`/`??` marker, a quality class and the better alternative when one
exists. It closes with per-side accuracy and an evaluation curve.

## `checkai eval`

Inspect what the engine thinks about a position: the static evaluation, the
material balance, a ranked move list, the search statistics, the opening-book
entries and the tablebase verdict.

```bash
checkai eval --fen "<FEN>" [--depth N] [--top N]
```

```bash
checkai eval                                  # the starting position
checkai eval --fen "<FEN>" --top 20 --depth 10
checkai eval --fen "<FEN>" --book book.bin    # include book statistics
```

## `checkai bench`

Run the fixed twelve-position benchmark suite.

```bash
checkai bench [--depth N | --movetime MS] [--threads N] [--hash MB]
```

Single-threaded runs print a **bench signature** — the total node count over the
suite. That number is deterministic, so comparing it between builds shows
whether a change altered the search. Threaded runs are non-deterministic and
should be compared on time instead.

## `checkai perft`

Count the leaf nodes of the legal-move tree and compare against the published
reference values.

```bash
checkai perft [DEPTH] [--fen FEN] [--divide] [--threads N]
```

```bash
checkai perft            # startpos, depths 1-5, verified
checkai perft 6 --threads 0   # depth 6 on every core
checkai perft 4 --divide      # per-root-move subtotals
```

## `checkai uci`

Speak the Universal Chess Interface on stdin/stdout, so CheckAI can be used
from any chess GUI or match runner (cutechess-cli, fastchess, Arena, …).

```bash
checkai uci
```

### Supported options

| Option              | Type   | Effect                                  |
| ------------------- | ------ | --------------------------------------- |
| `Hash`              | spin   | Transposition table size in MB (1–4096) |
| `Threads`           | spin   | Lazy SMP search threads (1–64)          |
| `MultiPV`           | spin   | Number of principal variations (1–16)   |
| `Move Overhead`     | spin   | Latency subtracted from every budget    |
| `Ponder`            | check  | Advertises pondering support            |
| `OwnBook`           | check  | Use the configured opening book         |
| `BookFile`          | string | Path to a Polyglot `.bin` book          |
| `SyzygyPath`        | string | Path to a Syzygy tablebase directory    |
| `UCI_LimitStrength` | check  | Enable artificial strength limiting     |
| `UCI_Elo`           | spin   | Target strength, 800–2850               |
| `Skill Level`       | spin   | Direct 0–20 skill limit                 |
| `Clear Hash`        | button | Drop all learned tables                 |

### Supported commands

`uci`, `isready`, `setoption`, `ucinewgame`, `position`, `go`, `ponderhit`,
`stop`, `quit`, plus the conventional `d` (print board) and `eval` (static
evaluation) debug commands.

`go` accepts `depth`, `movetime`, `nodes`, `mate`, `searchmoves`, `wtime`,
`btime`, `winc`, `binc`, `movestogo`, `ponder` and `infinite`.

```text
$ checkai uci
uci
setoption name Threads value 4
setoption name MultiPV value 3
position startpos moves e2e4 e7e5
go movetime 1000
```

## `checkai export`

Export archived games.

```bash
checkai export [--list | --all | --game-id UUID] [--format text|pgn|json] [-o FILE]
```

```bash
checkai export --list                    # list archived games
checkai export --all --format pgn -o games.pgn
checkai export --game-id <UUID> --format json
```

## `checkai update`

Download and install the latest release from GitHub, verifying the published
SHA-256 checksum before replacing the running binary.

```bash
checkai update
```
