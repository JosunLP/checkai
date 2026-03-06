# @josunlp/checkai

**CheckAI chess engine compiled to WebAssembly** — use as a Node.js CLI tool or JavaScript library.

Implements complete FIDE 2023 chess rules with move generation, position evaluation, deep search, full game management, and export (PGN/JSON/text).

## Installation

This package is published on **GitHub Packages**. Configure the registry first (`bun` reads `.npmrc`):

```bash
echo "@josunlp:registry=https://npm.pkg.github.com" >> ~/.npmrc
```

Then install:

```bash
bun add --global @josunlp/checkai  # CLI tool
# or
bun add @josunlp/checkai           # library
```

The published package includes both the generated Node.js glue code and the compiled WebAssembly binary under `pkg/`. `@josunlp/checkai/raw` exposes the generated JS glue module, and `@josunlp/checkai/wasm` exposes a small JS helper that returns the resolved filesystem path / file URL for `checkai_bg.wasm`.

## CLI Usage

### CLI: Position analysis

```bash
# Show the starting position FEN
checkai fen

# List all legal moves
checkai moves "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"

# Evaluate a position (centipawns)
checkai eval "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"

# Search for the best move
checkai search "..." --depth 15

# Apply a single move
checkai move "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1" e2e4

# Print ASCII board
checkai board "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
```

### CLI: Game management

```bash
# Create a new game (returns game ID)
checkai game new
checkai game new "custom FEN"

# Show game state
checkai game state <ID>

# Submit a move
checkai game move <ID> e2 e4
checkai game move <ID> e7 e8 Q   # with promotion

# Process actions
checkai game action <ID> resign
checkai game action <ID> offer_draw
checkai game action <ID> accept_draw
checkai game action <ID> claim_draw threefold_repetition

# List / delete games
checkai game list
checkai game delete <ID>
```

### CLI: Export

```bash
checkai export <ID> pgn
checkai export <ID> json
checkai export <ID> text
```

### Interactive play

```bash
checkai play
```

### Other

```bash
checkai version
checkai search "..." --depth 10 --json   # JSON output for any command
```

## Library Usage

```javascript
import { engine } from "@josunlp/checkai";

// --- Position analysis ---
const fen = engine.startingFen();
const moves = engine.legalMoves(fen);
const score = engine.evaluate(fen);
const result = engine.bestMove(fen, 10);
const after = engine.makeMove(fen, "e2e4");

// Position checks
engine.isCheckmate(fen);
engine.isStalemate(fen);
engine.isCheck(fen);
engine.isInsufficientMaterial(fen);

// Board display
console.log(engine.boardToAscii(fen));

// --- Game management ---
const gameId = engine.createGame();
//const gameId = engine.createGameFromFen("custom FEN");

let state = engine.gameSubmitMove(gameId, "e2", "e4");
state = engine.gameSubmitMove(gameId, "e7", "e5");
console.log(state.fen, state.turn, state.isCheck);

state = engine.gameState(gameId);
const history = engine.gameMoveHistory(gameId);
const currentFen = engine.gameFen(gameId);

// Actions
engine.gameProcessAction(gameId, "offer_draw");
engine.gameProcessAction(gameId, "resign");

// --- Export ---
const pgn = engine.gameToPgn(gameId);
const json = engine.gameToJson(gameId);
const text = engine.gameToText(gameId);

// Cleanup
engine.deleteGame(gameId);
const allGames = engine.listGames();
```

## WASM Path Helper

```javascript
import wasmPath, {
  getWasmFileUrl,
  getWasmPath,
  wasmFileUrl,
  wasmUrl,
} from "@josunlp/checkai/wasm";

console.log(wasmPath);
console.log(getWasmPath());
console.log(wasmUrl);
console.log(wasmFileUrl.href);
console.log(getWasmFileUrl().href);
```

Use this helper if you need to pass the packaged `.wasm` file to another loader. Importing `@josunlp/checkai/wasm` as a WebAssembly module directly is intentionally avoided because plain `.wasm` subpath imports are not portable across Node.js and Bun.

## API Reference

### Position analysis

| Function                      | Parameters              | Returns                                                              |
| ----------------------------- | ----------------------- | -------------------------------------------------------------------- |
| `startingFen()`               | —                       | `string`                                                             |
| `legalMoves(fen)`             | FEN string              | `Array<{ from, to, promotion?, notation }>`                          |
| `evaluate(fen)`               | FEN string              | `number` (centipawns)                                                |
| `bestMove(fen, depth)`        | FEN string, depth 1-30  | `{ bestMove, score, depth, pv, nodes, timeMs }`                      |
| `makeMove(fen, move)`         | FEN, move (e.g. "e2e4") | `{ fen, isCheck, isCheckmate, isStalemate, isInsufficientMaterial }` |
| `isCheckmate(fen)`            | FEN string              | `boolean`                                                            |
| `isStalemate(fen)`            | FEN string              | `boolean`                                                            |
| `isCheck(fen)`                | FEN string              | `boolean`                                                            |
| `isInsufficientMaterial(fen)` | FEN string              | `boolean`                                                            |
| `boardToAscii(fen)`           | FEN string              | `string`                                                             |

### Game management

| Function                                   | Parameters                       | Returns       |
| ------------------------------------------ | -------------------------------- | ------------- |
| `createGame()`                             | —                                | `string` (ID) |
| `createGameFromFen(fen)`                   | FEN string                       | `string` (ID) |
| `gameState(id)`                            | game ID                          | Game state    |
| `gameSubmitMove(id, from, to, promotion?)` | game ID, squares, optional promo | Game state    |
| `gameProcessAction(id, action, reason?)`   | game ID, action string           | Game state    |
| `gameMoveHistory(id)`                      | game ID                          | `Array`       |
| `gameFen(id)`                              | game ID                          | `string`      |
| `deleteGame(id)`                           | game ID                          | `void`        |
| `listGames()`                              | —                                | `string[]`    |

### Export

| Function         | Parameters | Returns  |
| ---------------- | ---------- | -------- |
| `gameToPgn(id)`  | game ID    | `string` |
| `gameToJson(id)` | game ID    | `string` |
| `gameToText(id)` | game ID    | `string` |

## How it works

The engine is written in Rust and compiled to WebAssembly using `wasm-pack`. The core chess engine runs at near-native speed in any JavaScript runtime that supports WASM.

The search uses iterative deepening with aspiration windows, Principal Variation Search (PVS), transposition table (16 MB), null-move pruning, Late Move Reductions (LMR), killer/history/counter-move heuristics, quiescence search, and SEE-based pruning.

## Requirements

- Node.js >= 18 (WASM support required)

## License

MIT — see [LICENSE.md](https://github.com/JosunLP/checkai/blob/main/LICENSE.md)
