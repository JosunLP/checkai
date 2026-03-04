# Architecture

CheckAI is structured as a modular Rust application with clear separation of concerns.

## Module Overview

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
├── eval.rs          # PeSTO position evaluation
├── search.rs        # Alpha-beta search engine (PVS, TT, LMR, NMP)
├── opening_book.rs  # Polyglot opening book reader
├── tablebase.rs     # Syzygy endgame tablebase interface
├── analysis.rs      # Analysis orchestrator (async job queue)
└── analysis_api.rs  # Analysis REST API endpoints
```

## Data Flow

```bash
AI Agent ──► REST API (api.rs) ──► GameManager (game.rs) ──► MoveGen (movegen.rs)
                                        │
AI Agent ──► WebSocket (ws.rs) ─────────┘
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

The web UI is compiled into the binary via `rust-embed`, eliminating runtime file dependencies:

```rust
#[derive(RustEmbed)]
#[folder = "web/"]
struct WebAssets;
```

### Zobrist Hashing

Position hashing for the transposition table uses Zobrist hashing with compile-time generated random keys. This provides:

- Fast incremental hash updates on each move
- Excellent collision resistance
- Zero runtime initialization cost

### PeSTO Evaluation

The evaluation function uses Piece-Square Tables Only (PeSTO), which provides:

- Separate midgame and endgame evaluation
- Phase-based interpolation
- No expensive pattern recognition — pure table lookups
