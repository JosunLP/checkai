# Architecture

CheckAI is structured as a modular Rust application with a modern TypeScript web UI, providing clear separation of concerns.

## Module Overview

### Rust Backend

```bash
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
├── terminal.rs      # Terminal interface with colored output
├── i18n.rs          # Internationalization helpers
├── zobrist.rs       # Zobrist hashing (compile-time key generation)
├── eval.rs          # PeSTO evaluation + king safety + mobility
├── search.rs        # Alpha-beta PVS + TT + LMR + NMP + SEE + futility
├── opening_book.rs  # Polyglot opening book reader
├── tablebase.rs     # Syzygy endgame tablebase interface
├── analysis.rs      # Analysis orchestrator (async job queue)
└── analysis_api.rs  # Analysis REST API endpoints
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
├── analysis.ts   # Analysis panel with polling
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

- **Iterative deepening** with aspiration windows (initial delta ±25 cp)
- **Principal Variation Search** (PVS) — narrowed alpha-beta windows
- **Transposition table** — configurable hash table (64 MB default) with Zobrist keys
- **Null-move pruning** (R = 3) — skip a move to prove a position is strong
- **Late Move Reductions** (LMR) — reduce search depth for unlikely moves beyond the first 4
- **Internal Iterative Deepening** (IID) — shallow search at PV nodes without a TT move (depth ≥ 4)
- **Late Move Pruning** (LMP) — skip late quiet moves at depths 1–4 when threshold is exceeded
- **Razoring** — drop into quiescence search when eval + 300 cp ≤ alpha at depth ≤ 2
- **Killer moves** (2 slots per ply) and **history heuristic** (with aging between iterations) for move ordering
- **Counter-move heuristic** — prioritize the move that refuted the opponent's last move
- **Static Exchange Evaluation** (SEE) — filters bad captures at low depth
- **Futility pruning** — skips quiet moves when the static evaluation is far below alpha (depth ≤ 3)
- **Quiescence search** — resolves captures and checks to avoid horizon effects
