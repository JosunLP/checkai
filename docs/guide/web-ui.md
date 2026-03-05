# Web UI

CheckAI includes a modern browser-based UI built with TypeScript, [@bquery/bquery](https://www.npmjs.com/package/@bquery/bquery), and Tailwind CSS v4. The UI is embedded in the binary and served automatically.

## Access

Navigate to the server root after starting:

```bash
http://localhost:8080/
```

The Web UI is compiled into the binary via `rust-embed`, so no external files are needed.

## Features

- **Dashboard** — Overview of all active games with storage statistics
- **Game Board** — Interactive SVG chess board with click selection and legal move indicators
- **Move Input** — Click-to-select squares or type moves in coordinate notation
- **Legal Move Highlights** — Visual dots and rings for valid target squares
- **Promotion Dialog** — Piece-picker popup when a pawn reaches the 8th rank
- **Real-time Updates** — WebSocket-powered live game state with auto-reconnect
- **FEN/PGN Tools** — Copy current position as FEN, export PGN, import from FEN string
- **Board Flip** — Toggle board orientation
- **Analysis Panel** — Start deep analysis and view real-time results (score, depth, best move, PV)
- **Game Archive** — Browse and replay completed games move by move
- **Storage Statistics** — View data usage and game counts
- **WebSocket Indicator** — Connection status dot in the header

## Language Selection

The Web UI supports all 8 languages:

- English, German, French, Spanish
- Chinese (Simplified), Japanese, Portuguese, Russian

The language is auto-detected from your browser settings and can be changed at any time via the language selector in the header. Your preference is saved in `localStorage`.

## Technology Stack

| Layer      | Technology                                                             |
| ---------- | ---------------------------------------------------------------------- |
| DOM        | [@bquery/bquery](https://www.npmjs.com/package/@bquery/bquery) v1.4    |
| Reactivity | bQuery signals (`signal`, `computed`, `effect`, `batch`)               |
| Styling    | [Tailwind CSS v4](https://tailwindcss.com) with custom `@theme` tokens |
| Bundler    | [Vite](https://vite.dev) v7 with TypeScript, path aliases              |
| Language   | TypeScript 5.9 with strict mode                                        |

### Module Architecture

The UI follows a unidirectional data flow:

```
User action → API call → Signal update → Effect re-renders DOM
```

All state lives in reactive signals defined in `store.ts`. Effects in `main.ts` subscribe to signal changes and update the DOM. Components are plain functions that read/write signals and manipulate the DOM via bQuery's `$` helper.

## Development

Start the Vite dev server with HMR:

```bash
cd web
bun install
bun run dev
```

The dev server runs on `http://localhost:5173` and proxies all `/api` and `/ws` requests to the Rust backend on port 8080.

## Production Build

```bash
cd web
bun run build     # Outputs to web/dist/
cd ..
cargo build --release  # Embeds dist/ into the binary
```
