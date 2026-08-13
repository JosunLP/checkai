# Changelog

<!-- markdownlint-disable MD024 -->

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.1] - 2026-08-13

A follow-up to 1.0.0. The Linux desktop artifacts never made it into the 1.0.0
release because the packaging step failed, which also stopped the checksum file
and the Docker image from being published. This release fixes that and clears
the live-engine-panel defects found while reviewing both UIs.

### Fixed

- **Linux desktop packaging** — `electron-builder` derived the executable name
  from the package name (`@checkai/desktop` → `@checkaidesktop`) and refused to
  build the AppImage, because `@` is not safe in a file path. The Linux target
  now sets `executableName: checkai-desktop` explicitly. This is what broke the
  1.0.0 release: with the desktop job red, `Generate checksums` and
  `Docker image` never ran, so neither `checksums-sha256.txt` nor the
  `ghcr.io/josunlp/checkai` image was published for 1.0.0
- **Checksums could miss the WASM tarball** — the `checksums` job did not depend
  on the `wasm` job, yet its download glob (`checkai-*`) matches
  `checkai-wasm-<version>.tgz`. Whether the npm tarball was listed in
  `checksums-sha256.txt` came down to which job finished first
- **Web UI — engine panel could hang on "Thinking…"** — a search whose answer
  arrived after the game was deleted or switched was discarded as stale without
  clearing the running flag, leaving the panel spinning and the Evaluate button
  disabled until a different game was loaded
- **Web UI — the Evaluate button lied while a search was in flight** — switching
  games re-enabled it even though the previous request was still out, and
  pressing it only queued a re-run, so the click looked like it did nothing
- **Web UI — stale best-move hint** — with auto-analysis on, the previous
  position's evaluation and its best-move marker stayed on the board for the
  whole of the next search
- **Web UI — engine hint collided with the legal-move dot** — both were drawn on
  the same `::after` pseudo-element, so a suggested destination that was also a
  legal target rendered as a shrunken dashed box in the corner of the square.
  The hint moved to `::before`
- **Web UI — engine panel ignored a language switch** — the rendered idle text
  replaced the translated markup with an untagged paragraph, so it stayed in the
  language the app started in
- **Desktop — engine panel stayed blank after opening a game** — with Auto on,
  opening or re-selecting a game cleared the panel and never started a search;
  it filled in only once the opponent moved
- **Desktop — a poll tick could restart the search it had just triggered** — the
  refresh compared the new game state against a snapshot taken before its own
  request, so a move made while the poll was in flight counted twice and cost
  two full search budgets before a verdict appeared. A poll answering for a game
  the user has since left is now discarded instead of overwriting the new one
- **Desktop — duplicate opening-book moves crashed the board view** — the book
  list was keyed on the move notation, and a polyglot file can hold several
  entries for the same move; a duplicate key is a hard runtime error in Svelte 5
- **Desktop — out-of-range engine settings stuck on screen** — entering a value
  above the maximum twice left the box showing the rejected number while the
  engine used the clamped one, and clearing a field snapped the setting to its
  minimum (a 10 ms search budget) instead of keeping the current value
- **Docs** — the Docker guide still pinned its pull example to `0.3.1`

## [1.0.0] - 2026-08-13

The first stable release. The engine, the CLI, the web and desktop UIs and the
npm package now all run the same search and expose the same features.

### Added

- **Lazy SMP multi-threaded search** — `--threads N` (or `0` for one per core) runs the search across up to 64 threads sharing a single transposition table. Available on every engine-backed command and over UCI as the `Threads` option; WebAssembly builds stay single-threaded
- **MultiPV** — The search reports up to 16 principal variations. Exposed as `--multipv` on `analyze`/`eval`/`play`, the UCI `MultiPV` option, the `multi_pv` field of the position API, and the live engine panel in both UIs
- **Opening book and endgame tablebase in the engine** — The Polyglot book and Syzygy tablebase are now consulted by the search itself, not just by the game-review API. `--book`, `--book-best` and `--tablebase` on every engine command; `OwnBook`, `BookFile` and `SyzygyPath` over UCI. Book moves are chosen by weight-proportional sampling so opening play is not perfectly repetitive
- **`checkai eval`** — A new command that shows what the engine actually thinks: static evaluation, material balance, a ranked move list, search statistics, the opening-book entries for the position, and the tablebase verdict
- **Synchronous position analysis endpoint** — `POST /api/analysis/position` runs one bounded search and answers in the same request, returning the evaluation, best move, MultiPV lines, book and tablebase information. This is what interactive clients need; the existing job API remains for full-game review
- **Live engine panel in the web and desktop UIs** — An evaluation bar, the best-move hint on the board, the top candidate lines, opening-book statistics and the tablebase verdict, with configurable time, MultiPV width and thread count, plus an auto-analyse toggle
- **Chess clocks** — `--time 5+3` (also `90+30`, `30s`, `1m+2s`) gives `play` and `watch` a real two-sided clock; the engine paces itself from the remaining time exactly as it would under a UCI GUI
- **PGN import and export with real SAN** — A new PGN module renders and parses standard algebraic notation with correct disambiguation, and reads/writes complete PGN files including the Seven Tag Roster, `FEN`/`SetUp` start positions, comments, NAGs and variations. `play --pgn` resumes a game, `save`/`load` work in-game, `analyze --pgn` annotates a file, and `watch --pgn-out` saves the finished game
- **SAN input** — Moves can be typed as `Nf3`, `exd5`, `O-O` or `Qh4#` anywhere coordinate notation was accepted
- **Board themes and colour rendering** — `--board wood|ice|club|mono|ascii` draws the board with solid coloured squares on truecolor terminals, falling back to the classic ASCII grid otherwise
- **In-place animation** — Boards and the search readout repaint in place instead of scrolling past, pieces slide across the board square by square, the evaluation bar is colour-graded, and checkmate is punctuated with a flash. Everything stays TTY-gated
- **New in-game commands** — `pgn`, `eval`, `analyze`, `book`, `tb`, `redo`, `flip`, `new`, `level N`, `save`, `load`
- **Accuracy and evaluation curves** — `analyze` reports per-side accuracy derived from average centipawn loss, and both `analyze` and `watch` print a sparkline of the evaluation over the game
- **Parallel perft** — `checkai perft --threads N` splits the root moves across worker threads
- **Full UCI option set** — `Threads`, `MultiPV`, `Move Overhead`, `Ponder`, `OwnBook`, `BookFile`, `SyzygyPath`, `UCI_LimitStrength`, `UCI_Elo`, `Skill Level` and `Clear Hash`, plus `go searchmoves`/`mate`/`ponder`, `ponderhit`, and the conventional `d` and `eval` debug commands. `info` lines now carry `seldepth`, `multipv`, `hashfull` and `tbhits`, and `bestmove` suggests a ponder move
- **WASM engine API** — `analyze(fen, options)` exposes depth, movetime, nodes, MultiPV, hash size and skill limiting to JavaScript; `engineInfo()` reports the engine's version, limits and feature list. The npm CLI gains `analyze` and `info` commands
- **`checkai bench` signature** — Single-threaded runs print a deterministic total node count, so a change to the search is visible at a glance between builds. The suite grew from six to twelve positions

### Changed

- **Transposition table rebuilt as a lock-free structure** — Entries are packed into two atomic words guarded by the classic XOR checksum, so one table can be shared by every search thread without locking. At 16 bytes per slot instead of ~40, the same `--hash` budget now holds four times as many entries
- **Search strengthened** — Singular extensions, one-ply continuation history, an improving heuristic feeding reverse futility pruning / LMP / LMR, history-based pruning of repeatedly failing quiets, and an explicit root search with per-root-move aspiration windows
- **Two-tier time management** — A hard in-tree limit plus a soft limit that gates the start of each iteration, stretching by 65% while the best move is still changing and shrinking once it has been stable for five iterations
- **Skill-based difficulty levels** — Levels 1–6 now combine a depth cap with a skill limit, so a weak engine plays the occasional human-looking inaccuracy instead of being uniformly short-sighted. Levels 7–10 play at full strength
- **The WebAssembly crate shares the engine source** — `wasm/src/search.rs` was a 1 400-line copy that had drifted far behind the native engine. It is gone; the WASM build now compiles the same `src/search.rs` through a small clock shim, so the npm package gets the full-strength engine and cannot fall behind again
- **Move history and legal-move lists are shown in SAN** rather than coordinate notation
- **`analyze` never answers from the opening book** — Analysis is expected to search, so the book is disabled for it; book information is reported separately

### Fixed

- **The entire analysis API was unreachable** — `/api` was registered as an actix scope before `/api/analysis`, and since the first is a prefix of the second, every analysis request was swallowed by the games scope and answered with `404`. The narrower scope is now registered first, which makes both the job API and the new position endpoint work
- **Castling and en-passant flags survive SAN rendering** — Moves rebuilt from bare coordinates are resolved against the position before being written, so castling prints as `O-O` rather than `Kg1`
- **Interrupted searches report the previous best move** — A search aborted before completing an iteration no longer falls back to the first legal move

### Documentation

- New [The Search Engine](https://josunlp.github.io/checkai/guide/engine) guide covering the search, its options, time management, Lazy SMP, strength limiting and how to verify a build
- Rewritten [CLI Commands](https://josunlp.github.io/checkai/guide/cli) reference covering all eleven commands, the shared engine option group, every in-game command and the full UCI option table
- The Analysis API reference documents both analysis modes and when to use which

## [0.8.0] - 2026-06-12

### Added

- **Animated terminal CLI** — A new terminal experience built on `crossterm` and `indicatif`: animated boards, an evaluation bar, and live search spinners/progress bars. Every effect is TTY-gated and degrades gracefully to clean, parseable plain text when stdout is not a terminal, when `--no-color` is passed, or when `NO_COLOR` is set
- **`play` vs the built-in engine** — `checkai play` can now play against the built-in engine, streaming a live animated search and announcing each move with its evaluation. Flags: `--vs <engine|human>`, `--color <white|black|random>`, `--level <1-10>`, `--movetime`, `--depth`, `--fen`, `--ascii`, `--flip`. In-game commands now include `hint`, `undo`, and `fen`
- **`watch` command** — Watch an engine-vs-engine showcase. Set both sides with `--level`, or asymmetrically with `--level-white` / `--level-black`; control pacing with `--delay` and length with `--max-moves` (`--movetime`, `--ascii` also supported)
- **`analyze` command** — Analyze a position (`--fen`) or annotate a whole game (`--moves`) with a live, animated iterative-deepening display, then print the best move, evaluation, forced-mate distance, depth/nodes/time, and principal variation (`--depth`, `--movetime`)
- **`bench` command** — Run the fixed engine benchmark suite over a set of positions, reporting nodes, time, and nodes-per-second (`--depth`, default 12; or `--movetime`)
- **`perft` command** — Verify move generation with perft node counts to a given depth (positional `DEPTH`, default 5; `--fen`, `--divide`)
- **`uci` command** — Run as a UCI engine on stdin/stdout for chess GUIs and match runners (e.g. via cutechess-cli). UCI output is a machine protocol and is intentionally not internationalized
- **Global `--no-color` flag** — Added to every command alongside the existing `--lang`; disables ANSI styling (the `NO_COLOR` env var is honored too)
- **FEN position loading** — `Game::from_fen`, `Board::from_piece_placement`, and `CastlingRights::from_fen` parse and validate a full FEN so `play`, `analyze`, and `perft` can start from arbitrary positions, with accompanying tests
- **Completed 8-language i18n** — Localized all CLI strings, including the engine labels and the `play`, `watch`, `analyze`, `bench`, and `perft` flows, across all eight bundled languages (EN, DE, FR, ES, ZH, JA, PT, RU); English remains the source of truth and fallback
- **Community health files** — Added `CONTRIBUTING.md`, `SECURITY.md`, a pull-request template, and structured GitHub issue forms (bug report, feature request) replacing the previous Markdown issue templates
- **Engine test coverage** — Added tests for skill-level scaling, node-limit bounding, and a forced mate-in-two, plus opt-in nodes-per-second and node-count benchmark diagnostics

### Changed

- **`play` now defaults to playing vs the engine** — Running `checkai play` with no flags starts a game against the built-in engine (level 5) instead of a local two-player game; pass `--vs human` for the previous two-player behavior
- **Search engine overhaul** — Substantially strengthened the alpha-beta / PVS search:
  - Full Static Exchange Evaluation (swap algorithm with x-ray and en-passant handling) replaces the previous optimistic capture heuristic, driving both capture ordering and pruning
  - The transposition table gains generation-based aging, depth-preferred replacement, and a cached static evaluation, and is now probed and stored inside quiescence search
  - Hard time and node limits are enforced inside the tree via the `SearchLimits` / `IterationInfo` / `search_limited` contract, discarding partial iterations, with per-iteration progress reported to live CLI displays and UCI `info` output through a callback
  - Added mate-distance pruning, reverse futility pruning, adaptive null-move pruning with a high-depth verification search, table-driven Late Move Reductions, Late Move Pruning, Internal Iterative Reduction, check extensions, and a corrected counter-move heuristic
  - Quiescence search now resolves check evasions and applies per-capture delta pruning, SEE pruning, and transposition-table cutoffs
  - History scores use a gravity-style update with maluses for quiet moves that failed before a cutoff
- **Evaluation** — The PeSTO-style evaluation adds pawn-structure terms (passed, doubled, isolated, backward, and connected pawns), king-safety penalties (open files and a weakened pawn shield), per-piece mobility, and a tempo bonus on top of the tapered piece-square tables and bishop-pair / rook-file bonuses
- **Animated CLI welcome screen** — The welcome screen and terminal banner now reveal with a subtle animation and list the new commands
- **Version metadata** — Bumped the Rust crate, WASM crate, npm package, web UI, desktop app, OpenAPI metadata, and VitePress version label to 0.8.0

## [0.7.0] - 2026-05-13

### Added

- **Engine test coverage** — Added perft suites for the standard starting position (depths 1–3, with depth-4 guarded by `#[ignore]`) and the Kiwipete benchmark (depths 1–2, depth-3 ignored); new mate-in-one verification through the full search; transposition-table reuse test across consecutive iterative-deepening runs
- **Evaluation test coverage** — Added colour-mirror symmetry tests (starting position and asymmetric material imbalance), tapered-evaluation phase verification (full midgame phase at startpos, pure endgame phase in K+P vs K), and bishop-pair bonus delta test
- **REST API documentation for the archive** — Added an "Archive & Storage" section to `docs/api/rest.md` covering `GET /api/archive`, `GET /api/archive/stats`, `GET /api/archive/{game_id}`, and `GET /api/archive/{game_id}/replay`, including request/response shapes and error codes
- **Desktop packaging smoke test in CI** — The desktop CI job now runs `bun run pack` on Ubuntu to validate the full electron-builder pipeline end-to-end on every push

### Changed

- **Version metadata** — Bumped Rust crate, WASM crate, npm package, web UI, desktop app, and VitePress version label to 0.7.0
- **Released previously unreleased 0.6.0 desktop work** — Promoted the prior `[Unreleased]` section to a proper `[0.6.0] - 2026-03-09` entry

## [0.6.0] - 2026-03-09

### Added

- **Electron desktop app** — Added a dedicated Svelte-based Electron renderer alongside the web UI
  - Includes persistent desktop sessions, native file/folder pickers, local backend launch controls, inline logs, and a multi-view workspace shell
  - Packaged desktop builds can check GitHub Releases for updates, download them, and install on restart
- **Native desktop installers** — Release automation now publishes platform-native Electron installers in addition to updater-compatible artifacts
  - Linux releases include `.deb` alongside AppImage
  - macOS releases include `.dmg` alongside updater-compatible `.zip`
  - Windows releases include `.msi` alongside NSIS for in-app update compatibility
- **Desktop CI and release automation** — GitHub Actions now validate the Electron app on Ubuntu, macOS, and Windows and publish desktop release assets with dependency review coverage

### Changed

- **Version metadata** — Updated project/package version references, install snippets, OpenAPI metadata, and documentation to align with the 0.6.0 desktop release

## [0.5.2] - 2026-03-07

### Fixed

- **Web analysis UI contract drift** — Fixed the TypeScript analysis client so it now submits jobs to `/api/analysis/game/{game_id}` and polls `/api/analysis/jobs/{job_id}` instead of outdated `/api/games/*` paths
  - Reworked the analysis panel to render the actual job-based backend payload (status, progress, completed summary, failure state) instead of assuming live search telemetry fields that were never returned by the API
  - Reset analysis polling cleanly when switching games so stale background polling does not leak across views
- **Frontend API typing alignment** — Synced the web UI's TypeScript models with the Rust API contract
  - Added `position_history` to the web `GameState` type
  - Expanded move/action response typing to match the real server payloads
  - Replaced stale analysis result typing with explicit job/result summary types
- **Documentation and version metadata** — Updated OpenAPI metadata, README installation snippets, changelog entries, and analysis/tablebase docs so published docs match current behavior
  - Clarified that the analysis API is job-based game review, not a live score/nodes stream
  - Clarified current Syzygy support as analytical / heuristic scaffolding rather than full binary probing

### Added

- **Regression coverage** — Added tests for move-quality threshold boundaries and `GameStateJson` position-history export consistency

## [0.5.1] - 2026-03-06

### Fixed

- **Bun/WASM package contents** — Fixed the published `@josunlp/checkai` package so the compiled WebAssembly binary is included in release tarballs instead of only the generated glue JavaScript
  - Added a `prepack` packaging guard so Bun packaging and publishing always verify the generated `pkg/` artifacts before release
  - Removed the generated `pkg/.gitignore` during packaging, which previously caused package tarballs to omit `pkg/checkai.js` and `pkg/checkai_bg.wasm`
  - Added explicit npm subpath exports for the raw generated artifacts (`@josunlp/checkai/raw` and `@josunlp/checkai/wasm`)

## [0.5.0] - 2026-03-05

### Added

- **WebAssembly (WASM) build** — The core chess engine is now compiled to WebAssembly via `wasm-pack`, enabling use from JavaScript/Node.js environments
  - New `wasm/` crate with `#[path]` re-exports of core engine modules (types, movegen, eval, search, zobrist) — zero code duplication
  - WASM-compatible search using `web-time` crate instead of `std::time::Instant`
  - `js-sys` integration for timestamps and random ID generation
- **npm package** (`@josunlp/checkai`) published to **GitHub Packages**
  - Node.js CLI tool (`checkai`) installable via `npm install -g @josunlp/checkai`
  - JavaScript/ESM library API (`import { engine } from "@josunlp/checkai"`)
- **Full feature parity in WASM** — All major features available in the npm package:
  - Position analysis: `legalMoves`, `evaluate`, `bestMove`, `isCheckmate`, `isStalemate`, `isCheck`, `isInsufficientMaterial`, `makeMove`
  - Game management: `createGame`, `createGameFromFen`, `gameState`, `gameSubmitMove`, `gameProcessAction`, `gameMoveHistory`, `gameFen`, `deleteGame`, `listGames`
  - Export: `gameToPgn`, `gameToJson`, `gameToText`
  - Board display: `boardToAscii`
- **Node.js CLI commands** — `fen`, `moves`, `eval`, `search`, `move`, `board`, `play`, `game new/state/move/action/list/delete`, `export`, `version`
- **Release workflow** — New `wasm` job in `release.yml` builds the WASM package, creates a tarball release asset, and publishes to GitHub Packages using `GITHUB_TOKEN`

## [0.4.0] - 2026-03-05

### Added

- **Modern TypeScript Web UI** — Complete modular rewrite of the browser frontend
  - Built with [@bquery/bquery](https://www.npmjs.com/package/@bquery/bquery) v1.4 (TypeScript-first DOM library with signals)
  - Tailwind CSS v4 with custom `@theme` tokens for consistent design
  - Vite v7 build system with HMR, path aliases, and production bundling
  - 12 modular TypeScript source files: `types`, `store`, `api`, `ws`, `i18n`, `ui`, `board`, `game`, `archive`, `analysis`, `main`, `styles`
  - Reactive signal-driven architecture with unidirectional data flow
  - SVG chess board with click selection, legal move indicators, check highlight, and board flip
  - Analysis panel with start/stop, real-time polling, score formatting (including mate detection)
  - Promotion dialog with piece picker
  - FEN copy, PGN copy, and FEN import directly from the toolbar
  - WebSocket connection indicator with auto-reconnect
  - Vite-built SPA embedded into the Rust binary via `rust-embed` (dual `DistAssets` + `WebAssets` with priority fallback)
- **FEN/PGN API endpoints** — Three new REST endpoints for position interchange
  - `GET /api/games/{id}/fen` — Export full 6-field FEN notation
  - `POST /api/games/fen` — Create a new game from a FEN string with full validation
  - `GET /api/games/{id}/pgn` — Export PGN with Seven Tag Roster headers
  - Complete `parse_fen()` parser and `game_to_pgn()` generator
  - OpenAPI/Swagger annotations for all new endpoints
- **King safety evaluation** — Pawn shield analysis, open file penalties near the king, enemy piece tropism within Chebyshev distance 2
- **Piece mobility evaluation** — Pseudo-legal square counts for knights, bishops, rooks, and queens with separate midgame/endgame scoring
- **Static Exchange Evaluation (SEE)** — Filters bad captures at low depth (≤ 3) to reduce search explosion
- **Futility pruning** — Skips quiet moves when static evaluation plus margin is far below alpha at depth ≤ 3
- **Bun** as the frontend package manager and script runner (replaces Node.js/npm)
- **Build script** (`build.rs`) — Ensures `web/dist/` exists at compile time so `rust-embed` compiles without a prior web build (fixes CI for clippy/test jobs)

### Changed

- `rust-embed` now uses `include-exclude` feature to exclude TypeScript source, `node_modules`, and build config from the legacy `WebAssets` embed
- Evaluation module description updated from "PeSTO position evaluation" to "PeSTO evaluation + king safety + mobility"
- Search module description updated to include SEE and futility pruning
- VitePress documentation updated for all new features: architecture, analysis engine, web UI, REST API, landing page

### Fixed

- Promotion dialog: piece symbols were not displayed because `dataset.piece` was read instead of `dataset.promote` — now correctly reads the `data-promote` attribute from the HTML buttons
- CI build: `#[derive(RustEmbed)]` failed when `web/dist/` did not exist; added `build.rs` to auto-create the directory so `cargo clippy` and `cargo test` work without a prior web build
- Collapsed nested `if` statements in king tropism evaluation (clippy `collapsible_if`)
- Replaced manual range check with `RangeInclusive::contains` in futility pruning (clippy `manual_range_contains`)

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
- Analysis locale strings for English, German, French, Spanish, Chinese (Simplified), Japanese, Portuguese, and Russian
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

[Unreleased]: https://github.com/JosunLP/checkai/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/JosunLP/checkai/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/JosunLP/checkai/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/JosunLP/checkai/compare/v0.5.2...v0.6.0
[0.5.2]: https://github.com/JosunLP/checkai/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/JosunLP/checkai/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/JosunLP/checkai/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/JosunLP/checkai/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/JosunLP/checkai/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/JosunLP/checkai/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/JosunLP/checkai/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/JosunLP/checkai/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/JosunLP/checkai/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/JosunLP/checkai/releases/tag/v0.1.0
