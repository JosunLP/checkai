# Changelog

<!-- markdownlint-disable MD024 -->

All notable changes to CheckAI are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [1.0.1] — 2026-08-13

A follow-up to 1.0.0. The Linux desktop artifacts never made it into the 1.0.0 release because the packaging step failed, which also stopped the checksum file and the Docker image from being published. This release fixes that and clears the live-engine-panel defects found while reviewing both UIs.

### Fixed

- **Linux desktop packaging** — `electron-builder` derived the executable name from the package name (`@checkai/desktop` → `@checkaidesktop`) and refused to build the AppImage, because `@` is not safe in a file path. The Linux target now sets `executableName: checkai-desktop` explicitly. This is what broke the 1.0.0 release: with the desktop job red, `Generate checksums` and `Docker image` never ran, so neither `checksums-sha256.txt` nor the `ghcr.io/josunlp/checkai` image was published for 1.0.0
- **Checksums could miss the WASM tarball** — the `checksums` job did not depend on the `wasm` job, yet its download glob (`checkai-*`) matches `checkai-wasm-<version>.tgz`. Whether the npm tarball was listed in `checksums-sha256.txt` came down to which job finished first
- **Web UI — engine panel could hang on "Thinking…"** — a search whose answer arrived after the game was deleted or switched was discarded as stale without clearing the running flag, leaving the panel spinning and the Evaluate button disabled until a different game was loaded
- **Web UI — the Evaluate button lied while a search was in flight** — switching games re-enabled it even though the previous request was still out, and pressing it only queued a re-run, so the click looked like it did nothing
- **Web UI — stale best-move hint** — with auto-analysis on, the previous position's evaluation and its best-move marker stayed on the board for the whole of the next search
- **Web UI — engine hint collided with the legal-move dot** — both were drawn on the same `::after` pseudo-element, so a suggested destination that was also a legal target rendered as a shrunken dashed box in the corner of the square. The hint moved to `::before`
- **Web UI — engine panel ignored a language switch** — the rendered idle text replaced the translated markup with an untagged paragraph, so it stayed in the language the app started in
- **Desktop — engine panel stayed blank after opening a game** — with Auto on, opening or re-selecting a game cleared the panel and never started a search; it filled in only once the opponent moved
- **Desktop — a poll tick could restart the search it had just triggered** — the refresh compared the new game state against a snapshot taken before its own request, so a move made while the poll was in flight counted twice and cost two full search budgets before a verdict appeared. A poll answering for a game the user has since left is now discarded instead of overwriting the new one
- **Desktop — duplicate opening-book moves crashed the board view** — the book list was keyed on the move notation, and a polyglot file can hold several entries for the same move; a duplicate key is a hard runtime error in Svelte 5
- **Desktop — out-of-range engine settings stuck on screen** — entering a value above the maximum twice left the box showing the rejected number while the engine used the clamped one, and clearing a field snapped the setting to its minimum (a 10 ms search budget) instead of keeping the current value
- **Docs** — the Docker guide still pinned its pull example to `0.3.1`

## [1.0.0] — 2026-08-13

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

## [0.8.0] — 2026-06-12

### Added

- **Animated terminal CLI** — A new terminal experience built on `crossterm` + `indicatif` (animated boards, evaluation bar, live search spinners/progress bars) that is TTY-gated and degrades to clean plain text when piped, with `--no-color`, or with `NO_COLOR`
- **`play` vs the built-in engine** — `checkai play` now plays against the engine, streaming a live animated search and announcing each move with its evaluation, via `--vs <engine|human>`, `--color <white|black|random>`, `--level <1-10>`, `--movetime`, `--depth`, `--fen`, `--ascii`, and `--flip`; in-game commands gain `hint`, `undo`, and `fen`
- **`watch` command** — Engine-vs-engine showcase, with `--level` / `--level-white` / `--level-black`, plus `--delay`, `--max-moves`, `--movetime`, and `--ascii`
- **`analyze` command** — Analyze a FEN (`--fen`) or annotate a whole game (`--moves`) with a live animated iterative-deepening display, then print the best move, evaluation, mate distance, depth/nodes/time, and principal variation (`--depth`, `--movetime`)
- **`bench` command** — Run the fixed engine benchmark suite, reporting nodes, time, and nodes-per-second (`--depth`, `--movetime`)
- **`perft` command** — Verify move generation with perft node counts (`DEPTH`, `--fen`, `--divide`)
- **`uci` command** — Run as a UCI engine on stdin/stdout for chess GUIs and match runners; UCI output is intentionally not internationalized
- **Global `--no-color` flag** — Added to every command (alongside `--lang`); honors `NO_COLOR` too
- **FEN position loading** — `Game::from_fen`, `Board::from_piece_placement`, and `CastlingRights::from_fen` parse and validate full FEN strings for `play`, `analyze`, and `perft`
- **Completed 8-language i18n** — Localized all CLI strings, including the engine labels and the `play`, `watch`, `analyze`, `bench`, and `perft` flows, across all eight bundled languages
- **Community health files** — `CONTRIBUTING.md`, `SECURITY.md`, a pull-request template, and structured GitHub issue forms replacing the previous Markdown templates

### Changed

- **`play` now defaults to playing vs the engine** — Running `checkai play` with no flags starts a game against the engine (level 5) instead of a two-player game; use `--vs human` for the previous behavior
- **Search engine overhaul** — Full Static Exchange Evaluation, transposition-table aging with depth-preferred replacement and quiescence integration, in-tree hard time/node limits via the `SearchLimits` / `IterationInfo` / `search_limited` contract with per-iteration progress reporting (consumed by the live CLI displays and UCI `info`), mate-distance pruning, reverse futility pruning, adaptive null-move pruning with verification, table-driven Late Move Reductions, Late Move Pruning, Internal Iterative Reduction, check extensions, a corrected counter-move heuristic, gravity-style history with maluses, and a stronger quiescence search (check evasions, delta and SEE pruning)
- **Evaluation** — Added pawn-structure terms (passed, doubled, isolated, backward, connected pawns), king-safety penalties (open files, weakened pawn shield), per-piece mobility, and a tempo bonus on top of the tapered piece-square tables and bishop-pair / rook-file bonuses
- **Animated CLI welcome screen** — Animated welcome screen and terminal banner, now listing the new commands
- **Version metadata** — Bumped Rust crate, WASM crate, npm package, web UI, desktop app, OpenAPI metadata, and VitePress version label to 0.8.0

## [0.7.0] — 2026-05-13

### Added

- **Engine test coverage** — perft suites for the starting position (depths 1–3, depth-4 `#[ignore]`-gated) and the Kiwipete benchmark (depths 1–2, depth-3 `#[ignore]`-gated); mate-in-one verification through the full search; transposition-table reuse test across consecutive iterative-deepening runs
- **Evaluation test coverage** — colour-mirror symmetry (starting position and asymmetric material imbalance), tapered-evaluation phase verification, and bishop-pair bonus delta
- **REST archive documentation** — Documented `GET /api/archive`, `GET /api/archive/stats`, `GET /api/archive/{game_id}`, and `GET /api/archive/{game_id}/replay` in `docs/api/rest.md` with request/response shapes and error codes
- **Desktop packaging smoke test in CI** — The desktop CI job now runs `bun run pack` on Ubuntu to validate the full electron-builder pipeline end-to-end on every push

### Changed

- **Version metadata** — Bumped Rust crate, WASM crate, npm package, web UI, desktop app, and VitePress version label to 0.7.0

## [0.6.0] — 2026-03-09

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

## [0.5.2] — 2026-03-07

### Fixed

- **Web analysis UI contract drift** — Fixed the TypeScript analysis client so it now uses the real `/api/analysis/*` endpoints and renders job status / summary data instead of assuming a live search telemetry payload
- **Frontend API typing alignment** — Synced the web UI's TypeScript models with the Rust API contract, including `position_history`, move/action responses, and explicit analysis job/result types
- **Documentation and version metadata** — Updated OpenAPI metadata, installation snippets, and docs so the published documentation matches current server behavior

### Added

- **Regression coverage** — Added tests for move-quality threshold boundaries and `GameStateJson` position-history export consistency

## [0.5.1] — 2026-03-06

### Fixed

- **Bun/WASM package contents** — Fixed the published `@josunlp/checkai` package so release tarballs include the compiled WebAssembly binary instead of only the generated glue JavaScript
  - Added a `prepack` safeguard to verify generated `pkg/` artifacts before Bun packaging / publishing
  - Removed the generated `pkg/.gitignore` during packaging so `pkg/checkai.js` and `pkg/checkai_bg.wasm` are no longer filtered out
  - Added explicit npm subpath exports for the raw generated artifacts: `@josunlp/checkai/raw` and `@josunlp/checkai/wasm`

## [0.5.0] — 2026-03-05

### Added

- **WebAssembly (WASM) build** — The core chess engine compiled to WebAssembly via `wasm-pack`
  - New `wasm/` crate with `#[path]` re-exports of core engine modules — zero code duplication
  - WASM-compatible search using `web-time` crate, `js-sys` for timestamps and IDs
- **npm package** (`@josunlp/checkai`) published to **GitHub Packages**
  - Node.js CLI tool installable via `npm install -g @josunlp/checkai`
  - JavaScript/ESM library API via `import { engine } from "@josunlp/checkai"`
- **Full feature parity in WASM**:
  - Position analysis: `legalMoves`, `evaluate`, `bestMove`, `makeMove`, check/mate/stalemate detection
  - Game management: `createGame`, `gameSubmitMove`, `gameProcessAction`, history, FEN export
  - Export: PGN, JSON, text formatting
  - Board display: `boardToAscii`
- **Node.js CLI commands**: `fen`, `moves`, `eval`, `search`, `move`, `board`, `play`, `game`, `export`, `version`
- **Release workflow**: New `wasm` job builds WASM and publishes to GitHub Packages

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
