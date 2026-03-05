# What is CheckAI?

CheckAI is a **Rust application** that provides both a terminal interface and a full-featured REST + WebSocket API for playing chess. It is designed to let **AI agents play chess against each other**, strictly following the [FIDE 2023 Laws of Chess](https://www.fide.com/FIDE/handbook/LawsOfChess.pdf).

## Why CheckAI?

Building AI agents that play chess requires a reliable, rule-compliant game server. CheckAI fills that role by providing:

- **Correctness** — Every move is validated against the complete FIDE 2023 ruleset.
- **Programmability** — A clean JSON API lets any language or framework drive the game.
- **Analysis** — A built-in engine evaluates games with 30+ ply search depth, classifying every move from "Best" to "Blunder".
- **Flexibility** — Use the REST API, WebSocket API, terminal CLI, or embedded Web UI — whichever fits your workflow.

## Core Components

### Chess Engine

The engine in `movegen.rs` generates and validates all legal moves, including:

- Standard piece movement (King, Queen, Rook, Bishop, Knight, Pawn)
- Castling (kingside and queenside) with full condition checking
- En passant capture
- Pawn promotion
- Check, checkmate, and stalemate detection
- Draw conditions: 50-move rule, threefold repetition, insufficient material, dead position

### REST API

A JSON-based HTTP API powered by [Actix Web](https://actix.rs/) where agents can:

- Create and manage games
- Query game state and legal moves
- Submit moves and special actions (resign, claim draw)
- Auto-generated Swagger/OpenAPI docs at `/swagger-ui/`

### WebSocket API

A reactive WebSocket endpoint at `/ws` that mirrors every REST operation and adds:

- Real-time push events for moves, state changes, and game deletions
- Per-game subscriptions
- Archive replay support

### Analysis Engine

An asynchronous analysis pipeline that:

- Evaluates completed games at configurable depth (minimum 30 plies)
- Uses PVS/Negascout with transposition tables, null-move pruning, LMR, killer/history heuristics
- Classifies moves as Best / Excellent / Good / Inaccuracy / Mistake / Blunder
- Reports centipawn loss and principal variation per move

### Storage

Binary game storage with [zstd](https://facebook.github.io/zstd/) compression for archiving completed games. Export in text, PGN, and JSON formats.

## Tech Stack

| Component        | Technology                         |
| ---------------- | ---------------------------------- |
| Language         | Rust (Edition 2024)                |
| Web Framework    | Actix Web 4                        |
| WebSocket        | Actix Web Actors                   |
| Serialization    | serde / serde_json                 |
| API Docs         | utoipa + utoipa-swagger-ui         |
| CLI              | clap 4                             |
| Compression      | zstd                               |
| i18n             | rust-i18n                          |
| Asset Embedding  | rust-embed                         |
| Containerization | Docker + docker-compose            |
| WASM             | wasm-pack + wasm-bindgen           |
| npm Package      | @josunlp/checkai (GitHub Packages) |
