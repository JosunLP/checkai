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
    title: Deep Analysis Engine
    details: Async analysis with 30+ ply depth, PVS, SEE, futility pruning, king safety, mobility scoring, and PeSTO evaluation.
  - icon: 🖥️
    title: Modern Web UI
    details: TypeScript web app with bQuery signals, Tailwind CSS v4, analysis panel, FEN/PGN tools, board flip, and promotion dialog.
  - icon: 📖
    title: Opening Book & Tablebases
    details: Polyglot .bin opening book support plus Syzygy file detection and analytical evaluation for select endgames.
  - icon: 🌐
    title: 8 Languages
    details: English, German, French, Spanish, Chinese, Japanese, Portuguese, Russian — auto-detected from the browser with per-request API selection.
---
