# Web UI

CheckAI includes an embedded browser-based UI that is served automatically when the server is running.

## Access

Navigate to the server root after starting:

```bash
http://localhost:8080/
```

The Web UI is compiled into the binary via `rust-embed`, so no external files are needed.

## Features

- **Dashboard** — Overview of all active games
- **Game Board** — Interactive chess board with piece drag-and-drop
- **Move Input** — Click-to-select or type moves
- **Legal Move Highlights** — Visual indicators for valid target squares
- **Real-time Updates** — WebSocket-powered live game state
- **Game Archive** — Browse and replay completed games
- **Move Replay** — Step through archived games move by move
- **Storage Statistics** — View data usage and game counts

## Language Selection

The Web UI supports all 8 languages:

- English, German, French, Spanish
- Chinese (Simplified), Japanese, Portuguese, Russian

The language is auto-detected from your browser settings and can be changed at any time via the language selector in the header. Your preference is saved in `localStorage`.

## Technology

The frontend uses [bQuery](https://github.com/nicokimmel/bquery) — a lightweight reactive framework — with zero build step. All assets (HTML, CSS, JS) are served from the embedded binary.
