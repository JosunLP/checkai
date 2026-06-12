<div align="center">

# CheckAI

**_Chess Server for AI Agents_**

A Rust-powered chess server and CLI with REST, WebSocket, and deep analysis APIs — following FIDE 2023 rules.

[![CI](https://github.com/JosunLP/checkai/actions/workflows/ci.yml/badge.svg)](https://github.com/JosunLP/checkai/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE.md)
[![Rust](https://img.shields.io/badge/Rust-edition_2024-orange.svg)](https://www.rust-lang.org/)
[![GitHub All Releases](https://img.shields.io/github/downloads/josunlp/checkai/total.svg?label=Downloads)](https://github.com/JosunLP/checkai/releases)

[Documentation](https://josunlp.github.io/checkai/) | [Changelog](CHANGELOG.md) | [Releases](https://github.com/JosunLP/checkai/releases)

</div>

---

## Features

### Chess Engine

- **Full FIDE 2023 Rules** — Move generation and validation with castling, en passant, promotion, check/checkmate/stalemate, and all draw conditions (50-move rule, threefold repetition, insufficient material)
- **Search** — Iterative deepening with aspiration windows; Principal Variation Search (PVS / Negascout) with re-searches; a transposition table with generation-based aging, depth-preferred replacement, and a cached static eval (probed in both main and quiescence search); adaptive null-move pruning with a high-depth verification search; table-driven Late Move Reductions; Late Move Pruning; reverse-futility (static null move), classic futility, and razoring at frontier nodes; Internal Iterative Reduction; check extensions; mate-distance pruning; full Static Exchange Evaluation (SEE) for capture ordering and pruning; killer-move, counter-move, and gravity-style history heuristics; in-tree repetition detection and 50-move awareness; and a quiescence search with stand-pat, per-capture delta pruning, SEE pruning, and TT cutoffs. Hard time/node limits are enforced inside the tree via `SearchLimits` / `search_limited`, discarding partial iterations
- **PeSTO Evaluation** — Tapered midgame/endgame piece-square tables interpolated by game phase, plus pawn structure (passed, doubled, isolated, backward, connected pawns), bishop-pair bonus, rook open/semi-open file bonuses, king safety (open-file and pawn-shield penalties), per-piece mobility, and a tempo bonus. The evaluation is always relative to the side to move
- **Async Game Analysis** — A separate job-based analysis service performs deep (30+ ply) game review on top of the same engine
- **Opening Book** — Polyglot `.bin` format with binary search lookups
- **Endgame Tablebases** — Syzygy tablebase detection with analytical evaluation for common endgames

### APIs & Interfaces

- **REST API** — JSON-based endpoints for game management, moves, draw claims, resignation, FEN/PGN import/export ([Agent Protocol](docs/AGENT.md))
- **Analysis API** — Separate `/api/analysis/*` endpoints for asynchronous game review with job progress, completed summaries, and per-move annotations
- **WebSocket API** — Full real-time API at `/ws` mirroring REST endpoints with push notifications and game subscriptions
- **Swagger/OpenAPI** — Auto-generated interactive API docs at `/swagger-ui/`
- **Animated Terminal CLI** — A richly-featured terminal experience: play against the built-in engine (10-level ladder) or a second human, watch an engine-vs-engine showcase, analyze positions and games, benchmark the engine, run perft, and speak UCI for chess GUIs. Animated boards, eval bar, and search spinners (built on `crossterm` + `indicatif`) render only on a TTY and degrade to clean plain text when piped; honors `--no-color` / `NO_COLOR`

### Web & Deployment

- **Modern Web UI** — TypeScript SPA with @bquery/bquery, Tailwind CSS v4, Vite — interactive SVG board, analysis panel, FEN/PGN tools, promotion dialog, WebSocket auto-reconnect. Compiled into the binary via `rust-embed`
- **Desktop UI** — Electron app built with Svelte — dedicated desktop shell with persistent sessions, local backend launch controls, dashboard/game/analysis/archive views, inline log inspection, and desktop-focused settings
- **Docker Support** — Multi-stage Dockerfile and docker-compose.yml with volume mounts for game data, opening books, and tablebases
- **Internationalization** — 8 languages (EN, DE, FR, ES, ZH, JA, PT, RU) with auto-detection and per-request API selection
- **Self-Update** — Automatic version checks and `checkai update` for in-place binary updates
- **JavaScript Package** — [`@josunlp/checkai`](https://github.com/JosunLP/checkai/packages) on GitHub Packages — the full chess engine compiled to WebAssembly, usable as a Bun or Node.js CLI/library package

## Quick Start

### Install

Recommended: pin the release you want and verify the downloaded binary against the
published SHA-256 checksums before installing it. The examples below use
`v0.8.0`; check the [Releases](https://github.com/JosunLP/checkai/releases)
page and replace it with the current or desired release tag.

```bash
# Linux / macOS
CHECKAI_VERSION=v0.8.0
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
[ "$OS" = "darwin" ] || OS="linux"
ARCH="$(uname -m)"
case "$ARCH" in x86_64|amd64) ARCH=x86_64 ;; arm64|aarch64) ARCH=aarch64 ;; esac
ASSET="checkai-${OS}-${ARCH}"
BASE_URL="https://github.com/JosunLP/checkai/releases/download/${CHECKAI_VERSION}"

curl -fSLO "${BASE_URL}/${ASSET}"
curl -fSLO "${BASE_URL}/checksums-sha256.txt"
CHECKSUM_LINE="$(grep "  ${ASSET}$" checksums-sha256.txt)" || {
  echo "Error: Asset ${ASSET} not found in checksums-sha256.txt" >&2
  exit 1
}
if command -v sha256sum >/dev/null 2>&1; then
  echo "${CHECKSUM_LINE}" | sha256sum -c -
elif command -v shasum >/dev/null 2>&1; then
  echo "${CHECKSUM_LINE}" | shasum -a 256 -c -
else
  echo "Error: Neither sha256sum nor shasum found. On Linux, install coreutils; on macOS, shasum should be pre-installed." >&2
  exit 1
fi
chmod +x "${ASSET}"
sudo install -m 0755 "${ASSET}" /usr/local/bin/checkai
```

```powershell
# Windows (PowerShell)
$Version = "v0.8.0"
$Asset = "checkai-windows-x86_64.exe"
$BaseUrl = "https://github.com/JosunLP/checkai/releases/download/$Version"
Invoke-WebRequest "$BaseUrl/$Asset" -OutFile $Asset
Invoke-WebRequest "$BaseUrl/checksums-sha256.txt" -OutFile checksums-sha256.txt
$Expected = ((Select-String .\checksums-sha256.txt -Pattern "  $([regex]::Escape($Asset))$").Line -split "\s+")[0].ToLowerInvariant()
$Actual = (Get-FileHash ".\$Asset" -Algorithm SHA256).Hash.ToLowerInvariant()
if ($Actual -ne $Expected) { throw "Checksum verification failed for $Asset" }
New-Item -ItemType Directory "$env:LOCALAPPDATA\checkai" -Force | Out-Null
Move-Item -Force ".\$Asset" "$env:LOCALAPPDATA\checkai\checkai.exe"
```

For the shortest install path, you can pipe the installer script directly to your
shell. This executes the current `main` branch script immediately, so only use it
if you accept that trade-off:

```bash
curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | iex
```

### Uninstall

```bash
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.sh | iex
```

> **Tip:** The installer script automatically detects the operating system, architecture, and latest release, while the uninstaller script detects the operating system — no manual changes required. The direct installer shortcut is quick, but it executes a remote script before you can verify the release asset yourself.
> Prefer the pinned release commands above when you want release integrity checks before installation.
> See the [Getting Started guide](https://josunlp.github.io/checkai/guide/getting-started) for details.

### Build from Source

```bash
git clone https://github.com/JosunLP/checkai.git
cd checkai

# Build web UI (requires Bun)
cd web && bun install && bun run build && cd ..

# Build the Rust binary
cargo build --release
```

### Desktop App

The repository now also includes a dedicated Electron desktop shell in `desktop/`.

```bash
cd desktop
bun install --frozen-lockfile
bun run build
bun run start
```

By default the desktop app targets `http://127.0.0.1:8080`, can persist backend launch settings between sessions, and can start a local `checkai serve` process for you. The embedded live workspace is intentionally limited to loopback URLs for safety; non-local targets can still be opened in your browser. Packaged desktop releases can also check GitHub Releases for updates, download them, and prompt for restart-based installation from inside the app. Release automation now publishes updater-compatible artifacts (AppImage/zip/NSIS) together with native installer packages per platform (`.deb`, `.dmg`, `.msi`). To keep Windows desktop updates working, release builds continue to ship the updater-compatible NSIS package alongside the MSI installer.

### Start the Server

```bash
checkai serve                    # Default: http://0.0.0.0:8080
checkai serve --port 3000        # Custom port
checkai serve \
  --book-path books/book.bin \
  --tablebase-path tablebase/ \
  --analysis-depth 30            # With opening book + tablebases
```

Open `http://localhost:8080/` for the Web UI or `/swagger-ui/` for interactive API docs.

### Docker

```bash
docker compose up -d             # Build and start
docker compose logs -f           # Follow logs
docker compose down              # Stop
```

### JavaScript Package (WebAssembly)

The chess engine is also available as a Bun/Node.js package via **GitHub Packages**:

```bash
# Configure GitHub Packages registry (Bun reads .npmrc)
echo "@josunlp:registry=https://npm.pkg.github.com" >> ~/.npmrc

# Install as CLI
bun add --global @josunlp/checkai
checkai fen
checkai search "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1" --depth 15

# Or use as library
bun add @josunlp/checkai
```

```javascript
import { engine } from "@josunlp/checkai";
const moves = engine.legalMoves(engine.startingFen());
const result = engine.bestMove(engine.startingFen(), 10);
```

See the [package README](npm/README.md) for the full API reference.

### Terminal Mode

```bash
checkai play
```

## Command-Line Interface

`checkai` is a single binary with the subcommands below. Two global options apply to every command: `-l, --lang <LANG>` (override the locale, e.g. `de`, `fr`, `zh-CN`) and `--no-color` (disable colored output; the `NO_COLOR` env var is honored too). Run `checkai <command> --help` for the full flag list.

| Command   | Description                                                           | Example                                         |
|-----------|-----------------------------------------------------------------------|-------------------------------------------------|
| `serve`   | Start the REST + WebSocket API server with Swagger UI                 | `checkai serve --port 3000`                     |
| `play`    | Play in the terminal — **defaults to playing vs the built-in engine** | `checkai play --level 9 --color black`          |
| `watch`   | Watch the engine play itself (engine-vs-engine showcase)              | `checkai watch --level-white 9 --level-black 3` |
| `analyze` | Analyze a position (`--fen`) or annotate a whole game (`--moves`)     | `checkai analyze --fen "<FEN>" --depth 16`      |
| `bench`   | Run the fixed engine benchmark suite (nodes, time, NPS)               | `checkai bench --depth 12`                      |
| `perft`   | Verify move generation with perft node counts                         | `checkai perft 5 --divide`                      |
| `uci`     | Run as a UCI engine on stdin/stdout (for chess GUIs / match runners)  | `checkai uci`                                   |
| `export`  | Export archived games as text, PGN, or JSON                           | `checkai export --all --format pgn`             |
| `update`  | Update CheckAI to the latest version from GitHub                      | `checkai update`                                |
| `version` | Print the current version                                             | `checkai version`                               |

Highlights:

- **`play` plays vs the engine by default** — pick a strength on the 1–10 level ladder (`--level`, default 5) and a side (`--color white|black|random`). Use `--vs human` for a local two-player game. Other flags: `--movetime`, `--depth`, `--fen`, `--ascii`, `--flip`.
- **`watch`** runs an engine-vs-engine showcase — set both sides with `--level`, or asymmetrically with `--level-white` / `--level-black`; tune pacing with `--delay` (ms between moves) and cap length with `--max-moves`.
- **Animated, TTY-aware UI** — animated boards, an evaluation bar, and live search spinners/progress bars (powered by `crossterm` + `indicatif`) appear only when stdout is a terminal; piped output stays plain and parseable.
- **Color & locale** — `--no-color` / the `NO_COLOR` env var disable ANSI styling, and `--lang` selects one of 8 bundled languages (English is the source of truth and fallback).

```bash
checkai play                              # White vs the engine at level 5
checkai play --vs human --ascii           # Local two-player game, ASCII board
checkai watch --movetime 200 --delay 0    # Fast engine-vs-engine game
checkai analyze --moves "e2e4 e7e5 g1f3"  # Annotate a line move by move
checkai bench --depth 8                    # Faster, shallower benchmark
checkai perft 6                            # Exact node counts up to depth 6
```

### UCI Mode

`checkai uci` speaks the UCI protocol on stdin/stdout so you can drop CheckAI into any UCI-compatible GUI (Arena, Cute Chess, BanksiaGUI, …) or match runner. The UCI output is a machine protocol and is intentionally not localized.

```text
$ checkai uci
uci
position startpos moves e2e4
go movetime 1000
quit
```

Want to know how strong it really is? Measure it yourself. With [cutechess-cli](https://github.com/cutechess/cutechess) you can run CheckAI against any other UCI engine — for example Stockfish as a convenient, widely-available opponent — and read the result off the scoreboard:

```bash
cutechess-cli \
  -engine name=CheckAI cmd=checkai arg=uci \
  -engine name=Opponent cmd=stockfish \
  -each proto=uci tc=10+0.1 -games 100 -repeat -recover \
  -pgnout match.pgn
```

Adjust the opponent, time control, and game count to taste; the resulting score and Elo estimate are the honest measure of CheckAI's playing strength.

## API Reference

### Game Endpoints

| Method   | Path                     | Description                         |
| -------- | ------------------------ | ----------------------------------- |
| `POST`   | `/api/games`             | Create a new game                   |
| `GET`    | `/api/games`             | List all games                      |
| `GET`    | `/api/games/{id}`        | Get full game state                 |
| `DELETE` | `/api/games/{id}`        | Delete a game                       |
| `POST`   | `/api/games/{id}/move`   | Submit a move                       |
| `POST`   | `/api/games/{id}/action` | Special action (resign, draw claim) |
| `GET`    | `/api/games/{id}/moves`  | List legal moves                    |
| `GET`    | `/api/games/{id}/board`  | ASCII board display                 |
| `GET`    | `/api/games/{id}/fen`    | Export FEN notation                 |
| `POST`   | `/api/games/fen`         | Import game from FEN                |
| `GET`    | `/api/games/{id}/pgn`    | Export PGN notation                 |

### Analysis Endpoints

| Method   | Path                      | Description              |
| -------- | ------------------------- | ------------------------ |
| `POST`   | `/api/analysis/game/{id}` | Submit game for analysis |
| `GET`    | `/api/analysis/jobs`      | List all analysis jobs   |
| `GET`    | `/api/analysis/jobs/{id}` | Get job status & results |
| `DELETE` | `/api/analysis/jobs/{id}` | Cancel or delete a job   |

### WebSocket

Connect to `ws://localhost:8080/ws` for real-time bidirectional communication.

| Action                           | Fields                                |
| -------------------------------- | ------------------------------------- |
| `create_game`                    | —                                     |
| `list_games`                     | —                                     |
| `get_game`                       | `game_id`                             |
| `delete_game`                    | `game_id`                             |
| `submit_move`                    | `game_id`, `from`, `to`, `promotion?` |
| `submit_action`                  | `game_id`, `action_type`, `reason?`   |
| `get_legal_moves`                | `game_id`                             |
| `subscribe` / `unsubscribe`      | `game_id`                             |
| `list_archived` / `get_archived` | `game_id`                             |
| `replay_archived`                | `game_id`, `move_number?`             |

> Full API documentation with request/response schemas: [REST](https://josunlp.github.io/checkai/api/rest) | [WebSocket](https://josunlp.github.io/checkai/api/websocket) | [Analysis](https://josunlp.github.io/checkai/api/analysis)

## Usage Examples

### REST API

```bash
# Create a game
curl -X POST http://localhost:8080/api/games
# → { "game_id": "550e8400-...", "message": "New chess game created. White to move." }

# Submit a move (1. e4)
curl -X POST http://localhost:8080/api/games/{game_id}/move \
  -H "Content-Type: application/json" \
  -d '{"from": "e2", "to": "e4"}'

# Get legal moves
curl http://localhost:8080/api/games/{game_id}/moves

# Resign
curl -X POST http://localhost:8080/api/games/{game_id}/action \
  -H "Content-Type: application/json" \
  -d '{"action": "resign"}'

# Claim draw
curl -X POST http://localhost:8080/api/games/{game_id}/action \
  -H "Content-Type: application/json" \
  -d '{"action": "claim_draw", "reason": "threefold_repetition"}'

# Submit game for deep analysis
curl -X POST http://localhost:8080/api/analysis/game/{game_id} \
  -H "Content-Type: application/json" \
  -d '{"depth": 30}'
# → { "job_id": "a1b2c3d4-...", "message": "Analysis submitted ..." }

# Get analysis results
curl http://localhost:8080/api/analysis/jobs/{job_id}
```

### WebSocket and Real-Time Events

```javascript
const ws = new WebSocket("ws://localhost:8080/ws");

ws.onopen = () => {
  ws.send(JSON.stringify({ action: "create_game", request_id: "1" }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  if (msg.type === "response" && msg.action === "create_game") {
    const gameId = msg.data.game_id;
    ws.send(JSON.stringify({ action: "subscribe", game_id: gameId }));
    ws.send(JSON.stringify({
      action: "submit_move", game_id: gameId, from: "e2", to: "e4"
    }));
  }

  if (msg.type === "event") {
    console.log("Game event:", msg.event, msg.data);
  }
};
```

## Terminal Commands

These commands are available during an interactive `checkai play` game (single-letter aliases are shown by `help`):

| Command   | Description                          |
| --------- | ------------------------------------ |
| `e2e4`    | Move piece (from-to notation)        |
| `e7e8Q`   | Pawn promotion (append piece letter) |
| `moves`   | List all legal moves                 |
| `board`   | Show current board                   |
| `history` | Show move history                    |
| `fen`     | Show the current position as FEN     |
| `json`    | Game state as JSON                   |
| `hint`    | Ask the engine for a suggested move  |
| `undo`    | Take back the last move              |
| `resign`  | Resign the game                      |
| `draw`    | Claim a draw (if eligible)           |
| `help`    | Show help                            |
| `quit`    | Quit                                 |

## Updating

CheckAI checks for new versions on startup. Update manually:

```bash
checkai update
```

## Project Structure

```bash
checkai/
├── build.rs              # Ensures web/dist/ exists for rust-embed
├── Cargo.toml            # Dependencies and project metadata
├── Dockerfile            # Multi-stage Docker build
├── docker-compose.yml    # Container orchestration
├── .github/workflows/
│   ├── ci.yml            # CI (fmt, clippy, test, build)
│   ├── release.yml       # Release (binaries + Docker image)
│   └── docs.yml          # Documentation → GitHub Pages
├── scripts/
│   ├── install.sh        # Installer (Linux / macOS / Windows)
│   └── uninstall.sh      # Uninstaller (Linux / macOS / Windows)
├── docs/                 # VitePress documentation site
├── locales/              # i18n YAML files (8 languages)
├── wasm/                 # WebAssembly crate (wasm-pack)
│   ├── Cargo.toml        # WASM crate manifest
│   └── src/
│       ├── lib.rs        # WASM bindings (game mgmt, export, board)
│       └── search.rs     # Search with web-time::Instant
├── npm/                  # JS package (@josunlp/checkai)
│   ├── package.json      # Scoped to GitHub Packages
│   ├── bin/checkai.mjs   # Node.js CLI entry point
│   ├── src/index.mjs     # Library API exports
│   └── README.md         # package documentation
├── desktop/              # Electron desktop UI (Svelte renderer + native shell)
│   ├── bun.lock          # Bun lockfile for desktop workspace
│   ├── package.json      # Desktop build + packaging scripts
│   ├── index.html        # Renderer entry point
│   └── src/
│       ├── shared-types.ts   # Shared IPC contract (main, preload, renderer)
│       ├── main.ts           # Svelte renderer bootstrap
│       ├── App.svelte        # Root desktop UI component
│       ├── electron-main.ts
│       ├── preload.ts
│       └── styles.scss       # Desktop-specific styles
├── web/                  # TypeScript Web UI (bQuery + Tailwind + Vite)
│   ├── src/              # 12 TypeScript source modules
│   ├── dist/             # Vite production build (embedded into binary)
│   └── index.vite.html   # Vite HTML entry point
└── src/
    ├── main.rs           # Entry point, CLI, server setup
    ├── types.rs          # Core types (pieces, board, JSON protocol)
    ├── movegen.rs        # Move generation and validation
    ├── game.rs           # Game state management
    ├── api.rs            # REST API handlers + OpenAPI
    ├── ws.rs             # WebSocket API + broadcaster
    ├── storage.rs        # Persistent storage (zstd compression)
    ├── export.rs         # Export (text, PGN, JSON)
    ├── eval.rs           # PeSTO evaluation + king safety + mobility
    ├── search.rs         # Alpha-beta (PVS, TT, LMR, NMP, SEE, futility)
    ├── analysis.rs       # Analysis orchestrator (async job queue)
    ├── analysis_api.rs   # Analysis REST endpoints
    ├── opening_book.rs   # Polyglot opening book reader
    ├── tablebase.rs      # Syzygy endgame tablebase interface
    ├── zobrist.rs        # Zobrist hashing
    ├── terminal.rs       # Terminal interface
    ├── i18n.rs           # Internationalization helpers
    └── update.rs         # Self-update + version check
```

## Documentation

Full documentation at **<https://josunlp.github.io/checkai/>**

| Section                                                                    | Description                          |
| -------------------------------------------------------------------------- | ------------------------------------ |
| [Getting Started](https://josunlp.github.io/checkai/guide/getting-started) | Installation and first steps         |
| [REST API](https://josunlp.github.io/checkai/api/rest)                     | Full REST endpoint reference         |
| [WebSocket API](https://josunlp.github.io/checkai/api/websocket)           | Real-time bidirectional API          |
| [Analysis API](https://josunlp.github.io/checkai/api/analysis)             | Deep game analysis endpoints         |
| [Agent Protocol](https://josunlp.github.io/checkai/agent/overview)         | JSON protocol for AI agents          |
| [Chess Rules](https://josunlp.github.io/checkai/agent/chess-rules)         | FIDE 2023 rule reference             |
| [Architecture](https://josunlp.github.io/checkai/guide/architecture)       | Module overview and design decisions |
| [JavaScript Package](npm/README.md)                                        | WASM package API reference           |

The raw agent protocol specification is also available at [`docs/AGENT.md`](docs/AGENT.md).

## License

[MIT](LICENSE.md)
