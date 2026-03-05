# Changelog

All notable changes to CheckAI are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [0.4.0] — 2026-03-05

### Added

- **Modern TypeScript Web UI** — Complete modular rewrite with @bquery/bquery v1.4, Tailwind CSS v4, and Vite v7
  - 12 modular TypeScript source files with reactive signal-driven architecture
  - SVG chess board with click selection, legal move indicators, check highlight, board flip
  - Analysis panel with real-time polling and score formatting (including mate detection)
  - Promotion dialog, FEN/PGN toolbar tools, WebSocket indicator with auto-reconnect
  - Vite-built SPA embedded into the Rust binary via dual `rust-embed` (DistAssets + WebAssets fallback)
- **FEN/PGN API endpoints**
  - `GET /api/games/{id}/fen` — Export full 6-field FEN
  - `POST /api/games/fen` — Create game from FEN string
  - `GET /api/games/{id}/pgn` — Export PGN with Seven Tag Roster
- **King safety evaluation** — Pawn shield, open file penalties, enemy piece tropism
- **Piece mobility evaluation** — Per-phase square counts for knights, bishops, rooks, queens
- **Static Exchange Evaluation (SEE)** — Filters bad captures at low depth
- **Futility pruning** — Skips quiet moves when static eval is far below alpha
- **Build script** (`build.rs`) — Ensures `web/dist/` exists at compile time so `rust-embed` works without a prior web build
- **Bun** as frontend package manager (replaces Node.js/npm)

### Changed

- `rust-embed` uses `include-exclude` feature to exclude TS source from legacy embed
- VitePress documentation updated for all new features

### Fixed

- Promotion dialog not showing piece symbols (read wrong `data-` attribute)
- CI compile error when `web/dist/` missing — added `build.rs` to ensure the directory exists
- Clippy warnings: collapsed nested ifs, `RangeInclusive::contains`

## [0.3.1] — 2026-03-02

### Added

- **VitePress documentation site** — Complete project documentation built with VitePress and deployed to GitHub Pages
  - Guide: Getting Started, CLI Commands, Docker, Configuration, Web UI, Analysis Engine, Opening Book, Tablebases, Architecture, i18n
  - API Reference: REST API, WebSocket API, Analysis API with full endpoint docs and code samples
  - Agent Protocol: Overview, Game State, Move Output, Chess Rules (FIDE 2023), Special Actions, Examples
  - Local search, edit-on-GitHub links, dark mode
- **GitHub Actions workflow** (`docs.yml`) — Auto-deploys docs to GitHub Pages on every release

## [0.3.0] — 2026-03-02

### Added

- **Deep game analysis engine** — Asynchronous analysis of complete games with a minimum search depth of 30 plies
  - Alpha-beta search with PVS/Negascout, transposition table (configurable, default 64 MB), null-move pruning, late move reductions, killer/history heuristics, quiescence search
  - PeSTO-style position evaluation with midgame/endgame piece-square tables, pawn structure analysis, bishop pair bonus
  - Move quality classification: Best, Excellent (≤10 cp), Good (≤25 cp), Inaccuracy (≤50 cp), Mistake (≤100 cp), Blunder (>100 cp)
  - Per-move centipawn loss, principal variation, accuracy percentages per side
  - Zobrist hashing with compile-time key generation
- **Opening book support** — Polyglot `.bin` format reader with binary search lookups
- **Endgame tablebase support** — Syzygy tablebase interface with analytical probing for common endgames (KvK, KRvK, KQvK, etc.)
- **Analysis REST API** at `/api/analysis/*` — Architecturally isolated from player-facing endpoints
  - `POST /api/analysis/game/{id}` — Submit game for async analysis
  - `GET /api/analysis/jobs` — List all analysis jobs
  - `GET /api/analysis/jobs/{id}` — Get job status and results
  - `DELETE /api/analysis/jobs/{id}` — Cancel or delete a job
- **Docker support** — Multi-stage Dockerfile, docker-compose.yml with volume mounts
- **Docker image CI** — Release workflow builds and pushes Docker images to GHCR with semver tags
- CLI flags for analysis: `--book-path`, `--tablebase-path`, `--analysis-depth`, `--tt-size-mb`

## [0.2.2] — 2026-03-01

### Fixed

- Draw offer logic: offers now persist correctly after the offerer makes a move
- AGENT.md example 15: corrected `en_passant` field from `null` to `"e3"` after `1. e4`

### Added

- 37 comprehensive unit tests covering all critical chess engine edge cases

## [0.2.1] — 2026-03-01

### Fixed

- Web UI now embedded into the binary via `rust-embed`, eliminating the need for an external `web/` directory
- Removed `actix-files` dependency in favor of `rust-embed`

## [0.2.0] — 2026-03-01

### Added

- Full internationalization (i18n) for all user-facing strings (8 languages)
- CI/CD pipelines for GitHub Actions
- Cross-platform install and uninstall scripts
- Automatic update check on startup
- `checkai update` command for self-updating

### Changed

- All source code comments translated to English
- Web UI default language changed from German to English

## [0.1.0] — 2025-02-28

### Added

- Complete chess engine with full FIDE 2023 rules support
- REST API for AI agents
- WebSocket API with real-time event broadcasting
- Swagger/OpenAPI documentation
- Terminal interface with colored board display
- Game export in text, PGN, and JSON formats
- Game archiving with zstd compression
- Web UI for browser-based game viewing
