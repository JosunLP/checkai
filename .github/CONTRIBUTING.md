# Contributing to CheckAI

Thank you for your interest in contributing to CheckAI — a Rust-powered chess
server and CLI for AI agents, with REST, WebSocket, and deep analysis APIs.
This guide covers everything you need to get from a fresh clone to a merged
pull request.

By participating in this project you agree to follow our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Table of contents

- [Development setup](#development-setup)
- [Project layout](#project-layout)
- [Building and testing](#building-and-testing)
- [Quality gates](#quality-gates)
- [Internationalization (i18n)](#internationalization-i18n)
- [Commit conventions](#commit-conventions)
- [Pull request process](#pull-request-process)
- [Reporting bugs and requesting features](#reporting-bugs-and-requesting-features)

## Development setup

### Prerequisites

| Tool | Version | Used for |
| ---- | ------- | -------- |
| [Rust](https://rustup.rs/) | stable (edition 2024) | Core server, CLI, engine, WASM crate |
| [Bun](https://bun.sh/) | 1.3.x (CI pins 1.3.10) | Web UI, desktop app, docs, npm package |
| [Node.js](https://nodejs.org/) | 22 | Electron tooling for the desktop app |
| [wasm-pack](https://rustwasm.github.io/wasm-pack/) | 0.12.x | Building the WebAssembly package (optional) |

If you use [mise](https://mise.jdx.dev/), the repository's `mise.toml` installs
the Rust toolchain for you.

### Getting started

```bash
git clone https://github.com/JosunLP/checkai.git
cd checkai

# Build the web UI once so it can be embedded into the Rust binary
cd web && bun install --frozen-lockfile && bun run build && cd ..

# Build and test the Rust core
cargo build
cargo test
```

> **Note:** `build.rs` ensures `web/dist/` exists, but without a frontend build
> the embedded web UI will be empty or outdated. Rebuild `web/` whenever you
> change `web/src/` and want to validate the embedded UI.

## Project layout

```text
checkai/
├── src/            Rust core: engine (movegen, eval, search), REST API (api.rs),
│                   WebSocket (ws.rs), analysis, export, storage, terminal UI, CLI
├── locales/        rust-i18n locale files — 8 languages (en is the fallback)
├── web/            TypeScript/Vite SPA; build output (web/dist/) is embedded
│                   into the Rust binary via rust-embed
├── desktop/        Electron desktop app built with Svelte
├── wasm/           WebAssembly crate; shares engine sources with src/ via
│                   #[path = "../../src/..."] includes
├── npm/            Packaging for the @josunlp/checkai npm/Bun package (WASM)
├── docs/           VitePress documentation site, incl. the agent protocol
│                   reference (docs/AGENT.md)
├── scripts/        Install/uninstall and maintenance scripts
└── .github/        CI workflows, issue forms, and community health files
```

Because `wasm/` re-includes engine sources from `src/`, changes to shared
modules such as `src/types.rs`, `src/game.rs`, `src/movegen.rs`, `src/eval.rs`,
or `src/search.rs` can affect the server, terminal mode, **and** the npm/WASM
package at the same time. Always check which targets are affected.

## Building and testing

### Rust core (repository root)

```bash
cargo fmt --all -- --check                                  # formatting
RUSTFLAGS=-Dwarnings cargo clippy --all-targets --all-features  # lints
cargo test --all-features                                   # tests
cargo build --release                                       # release build
```

### Web UI (`web/`)

```bash
cd web
bun install --frozen-lockfile
bun run check          # typecheck + lint + format check
bun run build          # production build into web/dist/
```

### Desktop app (`desktop/`)

```bash
cd desktop
bun install --frozen-lockfile
bun run check          # validate the desktop app
bun run pack           # smoke-test electron-builder packaging
```

### Documentation (`docs/`)

```bash
cd docs
bun install --frozen-lockfile
bun run docs:build     # build the VitePress site
```

### WASM / npm package (`npm/`)

```bash
cd npm
bun run build          # builds the wasm/ crate via wasm-pack and prepares the package
```

## Quality gates

CI must stay green. Every pull request runs:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features` with `RUSTFLAGS=-Dwarnings`
   (every warning is an error)
3. `cargo test --all-features` on Linux, macOS, and Windows
4. Frontend lint/format/typecheck and desktop validation when applicable

Additional expectations:

- Every public item carries a rustdoc comment (`///`); modules start with
  `//!` docs. Code comments are written in English.
- Prefer small, single-responsibility structs and traits; keep the code DRY.
- `src/storage.rs` defines a versioned binary save format — intentional format
  changes need a version bump and a migration strategy.
- Keep `README.md` and `docs/AGENT.md` in sync with protocol, analysis, or
  evaluation changes so external agents do not integrate against stale rules.

## Internationalization (i18n)

All user-facing CLI and server strings go through
[rust-i18n](https://crates.io/crates/rust-i18n). English (`locales/en.yml`) is
the default and fallback language.

CheckAI ships **8 locales**, and every key must exist in all of them:

```text
locales/en.yml  locales/de.yml  locales/es.yml  locales/fr.yml
locales/ja.yml  locales/pt.yml  locales/ru.yml  locales/zh-CN.yml
```

### Adding a new key

1. Pick a namespaced, dotted key, e.g. `terminal.cmd_move`.
2. Add it to `locales/en.yml` first with the English text. Use `%{name}`
   placeholders for interpolated values:

   ```yaml
   terminal.move_status: '  Move %{num}.  %{color} to move.'
   ```

3. Add the translated entry under the **same key** to the other 7 locale
   files. If you cannot provide a confident translation, copy the English text
   and flag it in your PR description so a native speaker can refine it.
4. Use the key from Rust code via the `t!` macro:

   ```rust
   println!("{}", t!("terminal.move_status", num = move_number, color = color_name));
   ```

5. Verify nothing is missing — a quick check is to diff the key sets:

   ```bash
   for f in locales/*.yml; do echo "$f: $(grep -c '^[a-z]' "$f")"; done
   ```

   All 8 files should report the same key count.

Never hard-code user-facing English strings in Rust source; reviewers will ask
you to move them into the locale files.

## Commit conventions

The project uses [Conventional Commits](https://www.conventionalcommits.org/)-style
messages. Use a lowercase type prefix followed by a concise, imperative summary:

```text
feat: add icon generation script and update build process
fix: handle missing checksum entries
docs: clarify install version examples
chore: update dependencies in package.json
```

Common types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`,
`perf`. Keep commits focused — one logical change per commit.

## Pull request process

1. Fork the repository and create a topic branch from `development`
   (feature work targets `development`; `main` tracks releases).
2. Make your changes, following the quality gates and i18n rules above.
3. Run the full local check suite (fmt, clippy, test, plus frontend checks if
   you touched `web/` or `desktop/`).
4. Update documentation and `CHANGELOG.md` for user-visible changes.
5. Open a pull request and fill in the PR template completely.
6. A maintainer (@JosunLP) will review your PR. Address review feedback with
   follow-up commits; avoid force-pushing during an active review.
7. Once CI is green and the review is approved, a maintainer merges the PR.

## Reporting bugs and requesting features

Please use the issue forms:

- [Bug report](https://github.com/JosunLP/checkai/issues/new?template=bug_report.yml)
- [Feature request](https://github.com/JosunLP/checkai/issues/new?template=feature_request.yml)
- [Question](https://github.com/JosunLP/checkai/issues/new?template=question.yml)

For security vulnerabilities, **do not open a public issue** — follow the
process in [SECURITY.md](SECURITY.md) instead.

Thank you for helping make CheckAI better!
