# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-03-02

### Added

- **VitePress documentation site** — Complete project documentation built with VitePress and deployed to GitHub Pages
  - Guide section: Getting Started, CLI Commands, Docker, Configuration, Web UI, Analysis Engine, Opening Book, Tablebases, Architecture, Internationalization
  - API Reference: REST API, WebSocket API, Analysis API with full endpoint docs, request/response examples, and code samples (JavaScript, Python)
  - Agent Protocol: Overview, Game State schema, Move Output schema, Chess Rules (FIDE 2023), Special Actions, worked examples
  - Changelog page mirroring CHANGELOG.md
  - Local search, edit-on-GitHub links, dark mode support
- **GitHub Actions workflow** (`docs.yml`) — Automatically builds and deploys documentation to GitHub Pages on every release (`release: published`) with manual trigger support

## [0.3.0] - 2026-03-02

### Added

- **Deep game analysis engine** — Asynchronous analysis of complete games with a minimum search depth of 30 plies
  - Alpha-beta search with PVS/Negascout, transposition table (configurable, default 64 MB), null-move pruning, late move reductions, killer/history heuristics, quiescence search
  - PeSTO-style position evaluation with midgame/endgame piece-square tables, pawn structure analysis, bishop pair bonus
  - Move quality classification: Best, Excellent (≤10 cp), Good (≤25 cp), Inaccuracy (≤50 cp), Mistake (≤100 cp), Blunder (>100 cp)
  - Per-move centipawn loss, principal variation, accuracy percentages per side
  - Zobrist hashing with compile-time key generation
- **Opening book support** — Polyglot `.bin` format reader with binary search lookups
- **Endgame tablebase support** — Syzygy tablebase interface with analytical probing for common endgames (KvK, KRvK, KQvK, etc.) and infrastructure for full .rtbw/.rtbz files
- **Analysis REST API** at `/api/analysis/*` — Architecturally isolated from player-facing endpoints
  - `POST /api/analysis/game/{id}` — Submit game for async analysis
  - `GET /api/analysis/jobs` — List all analysis jobs
  - `GET /api/analysis/jobs/{id}` — Get job status and results
  - `DELETE /api/analysis/jobs/{id}` — Cancel or delete a job
- **Docker support** — Multi-stage Dockerfile, docker-compose.yml with volume mounts for data/books/tablebases, .dockerignore
- **Docker image CI** — Release workflow now builds and pushes Docker images to GHCR with semver tags
- CLI flags for analysis configuration: `--book-path`, `--tablebase-path`, `--analysis-depth`, `--tt-size-mb`
- Analysis locale strings for English and German
- New source modules: `zobrist.rs`, `eval.rs`, `search.rs`, `opening_book.rs`, `tablebase.rs`, `analysis.rs`, `analysis_api.rs`

## [0.2.2] - 2026-03-01

### Fixed

- Draw offer logic: offers now persist correctly after the offerer makes a move, allowing the opponent to accept or decline; previously, offers were cleared immediately on any move
- AGENT.md example 15 (Sicilian Defense): corrected `en_passant` field from `null` to `"e3"` after `1. e4`

### Added

- 37 comprehensive unit tests in `game.rs` covering all critical chess engine edge cases:
  - Draw offer lifecycle (persist, decline-by-moving, accept, self-accept rejection)
  - Resignation (both sides)
  - Checkmate patterns (Scholar's mate, Fool's mate)
  - Stalemate detection
  - Castling (kingside, blocked by check, blocked by attacked transit square)
  - En passant (capture, discovered check blocking, expiration after one move)
  - Pawn promotion (requirement enforcement, queen promotion)
  - Pinned pieces (rook along pin line, knight with no moves)
  - Halfmove clock (reset on pawn move, reset on capture)
  - Fullmove number increment after Black's move
  - Position history tracking and threefold repetition claim
  - 50-move rule claim (valid and premature)
  - Insufficient material (K vs K, K+N vs K, K+N+N vs K, K+B vs K+B same/different color)
  - Castling rights updates (king move, rook move, rook capture)
  - Game flow validation (move after game over, illegal move, opponent piece)

## [0.2.1] - 2026-03-01

### Fixed

- Web UI now embedded into the binary via `rust-embed`, eliminating the need for an external `web/` directory
  - Fixes `Specified path is not a directory: "web"` error when running after installation
  - Frontend is always in sync with the binary version — no separate copy/update step needed
- Removed `actix-files` dependency in favor of `rust-embed` for self-contained static asset serving
- Cleaned up broken web-directory copy logic from `update.rs`
- Reverted unnecessary web-copy additions in `install.ps1` (no longer needed)

## [0.2.0] - 2026-03-01

### Added

- Full internationalization (i18n) for all user-facing strings (backend + frontend)
  - Supported languages: English, German, French, Spanish, Chinese (Simplified), Japanese, Portuguese, Russian
  - English as default with automatic fallback
  - Backend: `rust-i18n` crate with YAML locale files and `t!()` macro
  - CLI: `--lang` flag for explicit locale override, auto-detection via `CHECKAI_LANG` env var and system locale
  - REST API: per-request locale via `?lang=` query parameter and `Accept-Language` header
  - Web UI: browser-based locale detection with language selector dropdown and localStorage persistence
- `i18n.rs` helper module for locale detection and HTTP request extraction
- Web UI language selector in header with live locale switching
- `web/js/i18n.js` frontend translation module with 8 languages (~120 keys each)
- CI/CD pipelines for GitHub Actions (build, test, release)
- Cross-platform install and uninstall scripts (Linux, macOS, Windows)
- Automatic update check on startup (notifies when a new version is available)
- `checkai update` command for in-place self-updating on all platforms
- CHANGELOG.md following Keep a Changelog format
- Semantic versioning (SemVer) for all releases

### Changed

- All source code comments translated to English
- All hardcoded user-facing strings in 10 Rust source modules replaced with `t!()` i18n calls
- Web UI default language changed from German to English with `data-i18n` attribute system
- `PIECE_NAMES` constant replaced with `pieceName()` function using i18n lookups

### Fixed

- Resolved 24 Clippy warnings (collapsible if-let, redundant closures, `&PathBuf` → `&Path`, `io_other_error`, unnecessary `.to_string()` on `t!()` results)

## [0.1.0] - 2025-02-28

### Added

- Complete chess engine with full FIDE 2023 rules support
  - Move generation and validation
  - Castling, en passant, pawn promotion
  - Check, checkmate, and stalemate detection
  - Draw conditions: 50-move rule, threefold repetition, insufficient material
- REST API for AI agents
  - Create, list, get, delete games
  - Submit moves and actions (draw claims, resignation)
  - Get legal moves and ASCII board representation
- WebSocket API at `/ws` with real-time event broadcasting
  - Subscribe to individual games
  - Push notifications for moves, state changes, and deletions
- Swagger/OpenAPI documentation at `/swagger-ui/`
- Terminal interface with colored board display and interactive move input
- Game export in text, PGN, and JSON formats
- Game archiving with zstd compression
- Web UI for browser-based game viewing

[Unreleased]: https://github.com/JosunLP/checkai/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/JosunLP/checkai/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/JosunLP/checkai/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/JosunLP/checkai/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/JosunLP/checkai/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/JosunLP/checkai/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/JosunLP/checkai/releases/tag/v0.1.0
