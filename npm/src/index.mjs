// CheckAI — JavaScript/Node.js library API
//
// Usage:
//   import { engine } from "@josunlp/checkai";
//   engine.init();
//   const moves = engine.legalMoves("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");

import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const wasm = require('../pkg/checkai.js');

// Auto-init on first import
wasm.init();

export const engine = {
  // --- Position analysis ---

  /** Standard starting position FEN. */
  startingFen: wasm.startingFen,

  /** List all legal moves for a FEN position. */
  legalMoves: wasm.legalMoves,

  /** Static evaluation in centipawns (positive = side to move is better). */
  evaluate: wasm.evaluate,

  /** Search for the best move at the given depth (1-30). */
  bestMove: wasm.bestMove,

  /** Check if the position is checkmate. */
  isCheckmate: wasm.isCheckmate,

  /** Check if the position is stalemate. */
  isStalemate: wasm.isStalemate,

  /** Check if there is insufficient material for checkmate. */
  isInsufficientMaterial: wasm.isInsufficientMaterial,

  /** Check if the side to move is in check. */
  isCheck: wasm.isCheck,

  /**
   * Apply a move (coordinate notation, e.g. "e2e4") to a FEN position.
   * Returns { fen, is_check, is_checkmate, is_stalemate, is_insufficient_material }.
   */
  makeMove: wasm.makeMove,

  // --- Board display ---

  /** Render an ASCII board diagram from a FEN string. */
  boardToAscii: wasm.boardToAscii,

  // --- Game management ---

  /** Create a new game from the starting position. Returns the game ID. */
  createGame: wasm.createGame,

  /** Create a new game from a custom FEN. Returns the game ID. */
  createGameFromFen: wasm.createGameFromFen,

  /** Get the full state of a game (JSON object). */
  gameState: wasm.gameState,

  /** Submit a move to a game. Returns updated state. */
  gameSubmitMove: wasm.gameSubmitMove,

  /** Process a game action (resign, offer_draw, accept_draw, claim_draw). */
  gameProcessAction: wasm.gameProcessAction,

  /** Get the move history of a game. */
  gameMoveHistory: wasm.gameMoveHistory,

  /** Get the current FEN of a game. */
  gameFen: wasm.gameFen,

  /** Delete a game from the in-memory store. */
  deleteGame: wasm.deleteGame,

  /** List all active game IDs. */
  listGames: wasm.listGames,

  // --- Export ---

  /** Export a game as PGN string. */
  gameToPgn: wasm.gameToPgn,

  /** Export a game as JSON string. */
  gameToJson: wasm.gameToJson,

  /** Export a game as human-readable text. */
  gameToText: wasm.gameToText,
};

export default engine;
