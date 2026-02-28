# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- CI/CD pipelines for GitHub Actions (build, test, release)
- Cross-platform install and uninstall scripts (Linux, macOS, Windows)
- Automatic update check on startup (notifies when a new version is available)
- `checkai update` command for in-place self-updating on all platforms
- CHANGELOG.md following Keep a Changelog format
- Semantic versioning (SemVer) for all releases

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

[Unreleased]: https://github.com/JosunLP/checkai/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/JosunLP/checkai/releases/tag/v0.1.0
