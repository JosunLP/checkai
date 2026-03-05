# npm Package (WebAssembly)

The CheckAI chess engine is available as a Node.js package compiled to WebAssembly. It runs at near-native speed and provides the full engine feature set — position analysis, game management, export, and board display.

## Installation

The package is published on **GitHub Packages** under the `@josunlp` scope.

### 1. Configure the registry

```bash
echo "@josunlp:registry=https://npm.pkg.github.com" >> ~/.npmrc
```

### 2. Install

```bash
# As a global CLI tool
npm install -g @josunlp/checkai

# As a project dependency
npm install @josunlp/checkai
```

## CLI Usage

Once installed globally, the `checkai` command is available:

```bash
# Position analysis
checkai fen                          # Starting position FEN
checkai moves "<FEN>"                # List legal moves
checkai eval "<FEN>"                 # Static evaluation (centipawns)
checkai search "<FEN>" --depth 15    # Best move search
checkai move "<FEN>" e2e4            # Apply a move
checkai board "<FEN>"                # ASCII board diagram

# Game management
checkai game new                     # Create game (returns ID)
checkai game new "<FEN>"             # Create from custom position
checkai game state <ID>              # Full game state
checkai game move <ID> e2 e4         # Submit move
checkai game move <ID> e7 e8 Q       # Submit with promotion
checkai game action <ID> resign      # Resign
checkai game action <ID> offer_draw  # Offer draw
checkai game action <ID> claim_draw threefold_repetition
checkai game list                    # List active games
checkai game delete <ID>             # Delete game

# Export
checkai export <ID> pgn              # PGN format
checkai export <ID> json             # JSON format
checkai export <ID> text             # Human-readable text

# Interactive play
checkai play

# Other
checkai version
checkai help
```

Add `--json` to any command for JSON output.

## Library API

```javascript
import { engine } from "@josunlp/checkai";

// --- Position analysis ---
const fen = engine.startingFen();
const moves = engine.legalMoves(fen);       // Array of legal moves
const score = engine.evaluate(fen);          // Centipawns
const result = engine.bestMove(fen, 10);     // { bestMove, score, depth, pv, nodes, timeMs }
const after = engine.makeMove(fen, "e2e4");  // { fen, isCheck, isCheckmate, ... }

engine.isCheckmate(fen);            // boolean
engine.isStalemate(fen);            // boolean
engine.isCheck(fen);                // boolean
engine.isInsufficientMaterial(fen); // boolean

// --- Board display ---
console.log(engine.boardToAscii(fen));

// --- Game management ---
const gameId = engine.createGame();
// const gameId = engine.createGameFromFen("custom FEN");

let state = engine.gameSubmitMove(gameId, "e2", "e4");
state = engine.gameSubmitMove(gameId, "e7", "e5");
console.log(state.fen, state.turn, state.isCheck);

state = engine.gameState(gameId);
const history = engine.gameMoveHistory(gameId);
const currentFen = engine.gameFen(gameId);

engine.gameProcessAction(gameId, "resign");
engine.listGames();
engine.deleteGame(gameId);

// --- Export ---
const pgn = engine.gameToPgn(gameId);
const json = engine.gameToJson(gameId);
const text = engine.gameToText(gameId);
```

## API Reference

### Position Analysis

| Function                      | Parameters              | Returns                                                              |
| ----------------------------- | ----------------------- | -------------------------------------------------------------------- |
| `startingFen()`               | —                       | `string`                                                             |
| `legalMoves(fen)`             | FEN string              | `Array<{ from, to, promotion?, notation }>`                          |
| `evaluate(fen)`               | FEN string              | `number` (centipawns)                                                |
| `bestMove(fen, depth)`        | FEN, depth 1–30         | `{ bestMove, score, depth, pv, nodes, timeMs }`                      |
| `makeMove(fen, move)`         | FEN, move (e.g. "e2e4") | `{ fen, isCheck, isCheckmate, isStalemate, isInsufficientMaterial }` |
| `isCheckmate(fen)`            | FEN string              | `boolean`                                                            |
| `isStalemate(fen)`            | FEN string              | `boolean`                                                            |
| `isCheck(fen)`                | FEN string              | `boolean`                                                            |
| `isInsufficientMaterial(fen)` | FEN string              | `boolean`                                                            |
| `boardToAscii(fen)`           | FEN string              | `string`                                                             |

### Game Management

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

## How It Works

The engine is written in Rust and compiled to WebAssembly using `wasm-pack`. The WASM crate (`wasm/`) re-uses the core Rust source files via `#[path]` directives — there is zero code duplication between the native and WASM builds.

The search uses iterative deepening with aspiration windows, Principal Variation Search (PVS), a 16 MB transposition table, null-move pruning, Late Move Reductions, killer/history/counter-move heuristics, quiescence search, and SEE-based pruning.

## Requirements

- Node.js ≥ 18 (WASM support required)
- GitHub Packages authentication (for `npm install`)
