# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/JosunLP/checkai/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/JosunLP/checkai/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/JosunLP/checkai/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/JosunLP/checkai/releases/tag/v0.1.0
