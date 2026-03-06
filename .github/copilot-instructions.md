# CheckAI Workspace Instructions

## Architecture

- `src/` contains the Rust core: engine, REST/WebSocket API, analysis, export, storage, and CLI.
- `web/` is the TypeScript/Vite SPA. Its build output goes to `web/dist/` and is embedded into the Rust binary via `rust-embed`.
- `wasm/` is a separate WebAssembly crate and shares core logic with the main Rust project via `#[path = "../../src/..."]`. Changes to shared engine files can therefore affect the server, terminal mode, and the npm/WASM package at the same time.
- `npm/` packages the WASM output for Node.js. `docs/` contains the VitePress documentation.
- The external agent/protocol reference lives in `docs/AGENT.md`. Keep API or protocol structure changes consistent with that document.

## Build and Validation

- Rust checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features`
  - `cargo test --all-features`
  - `cargo build --release`
- Frontend (`web/`):
  - `bun install --frozen-lockfile`
  - `bun run check`
  - `bun run build`
- Documentation (`docs/`):
  - `bun install --frozen-lockfile`
  - `bun run docs:build`
- npm/WASM (`npm/`):
  - `bun run build`
- If you change `web/src/` and want to validate the embedded UI, rebuild `web/` before validating the Rust binary or release artifacts.

## Conventions

- Keep changes small and targeted. This repository combines a Rust backend, embedded frontend, WASM, and documentation; always check which areas are affected by a change.
- `src/storage.rs` defines a versioned binary save format. If the format changes intentionally, it needs a clear version bump and migration strategy.
- For file-based scans or optional data sources, prefer logging per-entry errors and continuing with partial results when the overall feature remains useful.
- The codebase often uses `Mutex<...>` with `lock().unwrap()` in API paths. Keep lock durations short and avoid additional work inside critical sections.
- User-facing text belongs to the locale files in `locales/`. If you change visible Rust or web text, check whether translations or at least the English fallback should also be updated.
- The frontend uses strict TypeScript and lint rules. Reuse the existing aliases and patterns from `web/src/` instead of introducing new structural conventions.
- Do not modify generated or build output such as `target/`, `web/dist/`, or `npm/pkg/` unless the task explicitly concerns artifacts or release contents.

## Project-Specific Pitfalls

- `build.rs` ensures that `web/dist/` exists, but without a frontend build the embedded UI can still be empty or outdated.
- Tablebase support must not currently be described as full binary Syzygy probing; according to the repository notes, the implementation currently uses analytical/heuristic fallbacks instead of real `.rtbw`/`.rtbz` probing.
- Changes to shared engine modules such as `src/types.rs`, `src/game.rs`, `src/movegen.rs`, `src/eval.rs`, or `src/search.rs` should always be checked for WASM/npm impact as well.
- If you change protocol, analysis, or evaluation logic, keep the code, `README.md`, and `docs/AGENT.md` in sync so external agents do not integrate against outdated rules.
