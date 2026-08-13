---
layout: home

hero:
  name: CheckAI
  text: Chess Server for AI Agents
  tagline: A Rust-powered chess server and CLI with REST, WebSocket, and deep analysis APIs — following FIDE 2023 rules.
  image:
    src: /logo.svg
    alt: CheckAI
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: API Reference
      link: /api/rest
    - theme: alt
      text: View on GitHub
      link: https://github.com/JosunLP/checkai

features:
  - icon: ♟️
    title: Complete Chess Engine
    details: Full FIDE 2023 move generation with castling, en passant, promotion, check/checkmate/stalemate, and all draw conditions.
  - icon: 🔌
    title: REST & WebSocket API
    details: JSON-based API for AI agents and the web UI. FEN/PGN import/export, real-time WebSocket events, and interactive Swagger docs.
  - icon: 🔬
    title: Analysis Engine
    details: Iterative-deepening PVS with singular extensions, continuation history, null-move pruning, LMR, SEE and quiescence search, plus a PeSTO evaluation with pawn structure, king safety and mobility.
  - icon: ⚡
    title: Lazy SMP & MultiPV
    details: Up to 64 search threads sharing one lock-free transposition table, and up to 16 principal variations from the CLI, UCI, REST API and both UIs.
  - icon: ⌨️
    title: Animated Terminal CLI
    details: Play vs the engine (10-level ladder) or a human on a real chess clock, watch engine-vs-engine games, analyze PGN files, inspect the evaluation, bench, perft and speak UCI — with coloured boards and animated moves that fall back to plain text when piped.
  - icon: 🖥️
    title: Modern Web UI
    details: TypeScript web app with bQuery signals and Tailwind CSS v4 — a live engine panel with an evaluation bar, best-move hint and candidate lines, plus game review, FEN/PGN tools and a promotion dialog.
  - icon: 📖
    title: Opening Book & Tablebases
    details: Polyglot .bin opening book support plus Syzygy file detection and analytical evaluation for select endgames.
  - icon: 🌐
    title: 8 Languages
    details: English, German, French, Spanish, Chinese, Japanese, Portuguese, Russian — auto-detected from the browser with per-request API selection.
---
