# Architecture

CheckAI is structured as a modular Rust application with a modern TypeScript web UI, providing clear separation of concerns.

## Module Overview

### Rust Backend

```bash
build.rs             # Ensures web/dist/ exists for rust-embed at compile time
src/
├── main.rs          # Entry point, CLI parsing, server setup
├── types.rs         # Core types: pieces, board, squares, JSON protocol
├── movegen.rs       # Move generation and validation engine
├── game.rs          # Game state management and API response types
├── api.rs           # REST API handlers with OpenAPI annotations
├── ws.rs            # WebSocket API, broadcaster, and session actors
├── storage.rs       # Persistent binary storage with zstd compression
├── export.rs        # Game export (text, PGN, JSON)
├── update.rs        # Self-update and version check
├── terminal.rs      # Terminal input parsing (moves, SAN, REPL commands)
├── i18n.rs          # Internationalization helpers
├── zobrist.rs       # Zobrist hashing (compile-time key generation)
├── engine_time.rs   # Clock shim so the search compiles for native and WASM
├── eval.rs          # PeSTO evaluation + king safety + mobility
├── search.rs        # PVS + lock-free TT + Lazy SMP + MultiPV + book/tablebase
├── opening_book.rs  # Polyglot opening book reader
├── tablebase.rs     # Syzygy endgame tablebase interface
├── analysis.rs      # Analysis orchestrator (async jobs + sync position search)
├── analysis_api.rs  # Analysis REST API endpoints
└── cli/             # One module per command plus shared CLI infrastructure
    ├── mod.rs           # CliCommand trait, CliContext, shared error type
    ├── engine.rs        # The shared --threads/--hash/--book argument group
    ├── theme.rs         # Colour/TTY detection and width helpers
    ├── board_renderer.rs# Coloured-square and ASCII board rendering
    ├── animate.rs       # In-place redraw, move animation, reveal effects
    ├── progress.rs      # Spinners, progress bars, live thinking panel
    ├── panel.rs         # Box-drawing panels and tables
    ├── score.rs         # Score formatting, eval bars, sparklines, accuracy
    ├── clock.rs         # Time controls and the two-sided game clock
    ├── fen.rs           # FEN import/export for Game
    ├── pgn.rs           # SAN rendering/parsing and PGN read/write
    ├── level.rs         # The 1-10 difficulty ladder
    └── play.rs · watch.rs · analyze.rs · eval.rs · bench.rs · perft.rs
        · uci.rs · welcome.rs        # the commands themselves
```

### WebAssembly Crate

```bash
wasm/
├── Cargo.toml       # WASM crate manifest (cdylib + rlib)
└── src/
    ├── lib.rs         # WASM bindings: analysis, game mgmt, export, board
    └── engine_time.rs # web-time clock shim for the shared search engine
```

The WASM crate re-uses core source files from the parent crate via
`#[path = "../../src/..."]` directives — `types`, `movegen`, `eval`, `zobrist`,
`polyglot_keys`, `opening_book`, `tablebase` **and `search`**. Until 1.0.0 the
search was a local copy that had drifted far behind the native engine; routing
the two clock types through `engine_time` removed the last platform difference,
so the WebAssembly build now runs exactly the same search and cannot fall
behind again. It stays single-threaded, because WebAssembly has no worker
threads.

### JavaScript Package

```bash
npm/
├── package.json     # @josunlp/checkai (GitHub Packages, Bun-managed)
├── bin/checkai.mjs  # Node.js CLI entry point
├── src/index.mjs    # ESM library API
└── README.md        # package documentation
```

### TypeScript Web UI

```bash
web/src/
├── main.ts       # Entry point — navigation, effects, event binding
├── types.ts      # All interfaces, enums, and constants
├── store.ts      # Reactive state (bQuery signals)
├── api.ts        # Typed REST API client
├── ws.ts         # WebSocket manager with auto-reconnect
├── i18n.ts       # 8-language internationalization
├── ui.ts         # DOM utilities (setText, showToast, formatBytes)
├── board.ts      # SVG chess board renderer
├── game.ts       # Game CRUD, move execution, FEN/PGN
├── archive.ts    # Archive browsing and replay controls
├── analysis.ts   # Game-review panel with job polling
├── engine.ts     # Live engine panel (eval bar, MultiPV, book, tablebase)
└── styles.css    # Tailwind CSS v4 with custom @theme tokens
```

## Data Flow

```bash
Browser UI ──► REST API (api.rs) ──► GameManager (game.rs) ──► MoveGen (movegen.rs)
                                        │
Browser UI ──► WebSocket (ws.rs) ───────┘
                                        │
                                   Storage (storage.rs) ──► data/active/
                                        │                    data/archive/
                                        │
                                   Broadcaster (ws.rs) ──► Subscribed Clients
```

## Key Design Decisions

### Shared State

Game state is managed through `AppState`, which wraps:

- `GameManager` behind an `Arc<Mutex<...>>` for thread-safe access
- `AnalysisManager` for async job management
- `GameBroadcaster` (Actix actor) for WebSocket event dispatch

### Actix Actor Model

WebSocket connections use the Actix actor system:

- Each WebSocket connection is a `WsSession` actor
- The `GameBroadcaster` actor manages subscriptions and dispatches events
- Messages are typed and handled through the actor mailbox pattern

### Embedded Assets

The web UI is compiled into the binary via `rust-embed`. The Vite-built SPA from `web/dist/` takes priority, falling back to legacy assets in `web/`:

```rust
#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct DistAssets;

#[derive(RustEmbed)]
#[folder = "web/"]
struct WebAssets;
```

### Frontend Architecture

The web UI uses a signal-driven reactive architecture:

- **@bquery/bquery** — lightweight DOM library with TypeScript-first API
- **Signals** from `@bquery/bquery/reactive` for reactive state management
- **Tailwind CSS v4** with custom `@theme` tokens for consistent design
- **Vite** for development server with HMR and production bundling

State flows unidirectionally: user actions → API calls → signal updates → effects re-render the DOM.

### Zobrist Hashing

Position hashing for the transposition table uses Zobrist hashing with compile-time generated random keys. This provides:

- Fast incremental hash updates on each move
- Excellent collision resistance
- Zero runtime initialization cost

### Evaluation Features

The evaluation function combines multiple scoring components:

- **PeSTO tables** — separate midgame/endgame piece-square tables with phase interpolation
- **King safety** — pawn shield analysis, open file penalties near the king, enemy piece tropism (Chebyshev distance)
- **Piece mobility** — pseudo-legal square counts for knights, bishops, rooks, and queens with per-phase scoring
- **Pawn structure** — doubled, isolated, passed, backward, and connected pawn evaluation
- **Positional bonuses** — bishop pair, rook on open/semi-open files
- **Tempo bonus** — small bonus for the side to move
- **Space advantage** — bonus for pawns advanced past the center into the opponent's half

### Search Techniques

The alpha-beta search employs numerous pruning and ordering optimizations:

- **Iterative deepening** with per-MultiPV-line aspiration windows
- **Principal Variation Search** (PVS) — narrowed alpha-beta windows
- **Lock-free transposition table** — two atomic words per slot guarded by an XOR checksum, shared by every Lazy SMP thread
- **Lazy SMP** — up to 64 threads searching the same position through one shared table
- **Singular extensions** — an extra ply when the TT move beats every alternative
- **Null-move pruning** — adaptive reduction with a verification search at high depth
- **Late Move Reductions** (LMR) — log-log table adjusted for PV nodes, killers, history and the improving flag
- **Internal Iterative Reduction** (IIR) — cheapen subtrees with no TT move (depth ≥ 4)
- **Late Move Pruning** (LMP) — skip late quiets at shallow depth, tightened on non-improving nodes
- **History pruning** — drop quiets that have repeatedly failed
- **Razoring** — drop into quiescence when the eval is far below alpha at depth ≤ 2
- **Killer moves**, **counter-moves**, **butterfly history** and **one-ply continuation history** for move ordering
- **Static Exchange Evaluation** (SEE) — capture ordering and pruning of losing captures
- **Futility pruning** — skips quiet moves far below alpha (depth ≤ 3)
- **Quiescence search** — resolves captures, promotions and check evasions to avoid horizon effects

See [The Search Engine](/guide/engine) for the full picture, including time
management and strength limiting.
