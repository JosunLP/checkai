#!/usr/bin/env node

// CheckAI — Node.js CLI powered by WebAssembly
// Usage:
//   checkai fen                         Print the starting position FEN
//   checkai moves <FEN>                 List legal moves
//   checkai eval <FEN>                  Evaluate a position
//   checkai search <FEN> [--depth N]    Search for the best move
//   checkai move <FEN> <MOVE>           Apply a move and print new FEN
//   checkai board <FEN>                 Print an ASCII board
//   checkai play                        Interactive game in the terminal
//   checkai game new [FEN]              Create a new managed game
//   checkai game state <ID>             Show game state
//   checkai game move <ID> <FROM> <TO>  Submit a move
//   checkai game action <ID> <ACTION>   Resign, offer/accept/claim draw
//   checkai game list                   List active games
//   checkai game delete <ID>            Delete a game
//   checkai export <ID> <FORMAT>        Export game (pgn|json|text)
//   checkai version                      Print version
//   checkai --help                      Show help

import { createRequire } from 'node:module';
import * as readline from 'node:readline';

const require = createRequire(import.meta.url);
const wasm = require('../pkg/checkai.js');

wasm.init();

const args = process.argv.slice(2);
const command = args[0];

function usage() {
  console.log(`CheckAI v0.5.0 — Chess engine (WebAssembly)

Usage:
  checkai <command> [options]

Commands:
  fen                            Print the starting position FEN
  moves <FEN>                    List all legal moves for a position
  eval <FEN>                     Static evaluation (centipawns)
  search <FEN> [--depth N]       Find the best move (default depth: 10)
  move <FEN> <MOVE>              Apply a move (e.g. e2e4) and print result
  board <FEN>                    Print an ASCII board diagram
  play                           Interactive game in the terminal
  game new [FEN]                 Create a new managed game (optional FEN)
  game state <ID>                Show full game state
  game move <ID> <FROM> <TO> [P] Submit a move (optional promotion Q/R/B/N)
  game action <ID> <ACTION> [R]  Process action (resign|offer_draw|accept_draw|claim_draw)
  game list                      List active game IDs
  game delete <ID>               Delete a game
  export <ID> <FORMAT>           Export game (pgn|json|text)
  version                        Print version
  help                           Show this help message

Options:
  --depth N                      Search depth (1-30, default: 10)
  --json                         Output results as JSON

Examples:
  checkai fen
  checkai moves "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
  checkai search "..." --depth 15
  checkai board "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
  checkai game new
  checkai game move <ID> e2 e4
  checkai export <ID> pgn
  checkai play`);
}

const jsonOutput = args.includes('--json');
const depthIdx = args.indexOf('--depth');
const depth = depthIdx !== -1 ? parseInt(args[depthIdx + 1], 10) || 10 : 10;

function output(data) {
  if (jsonOutput) {
    console.log(JSON.stringify(data, null, 2));
  } else if (typeof data === 'object') {
    for (const [key, val] of Object.entries(data)) {
      console.log(
        `${key}: ${typeof val === 'object' ? JSON.stringify(val) : val}`
      );
    }
  } else {
    console.log(data);
  }
}

function requireFen() {
  const fen = args[1];
  if (!fen) {
    console.error('Error: FEN string required. Example:');
    console.error(
      '  checkai moves "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"'
    );
    process.exit(1);
  }
  return fen;
}

try {
  switch (command) {
    case 'fen': {
      console.log(wasm.startingFen());
      break;
    }

    case 'moves': {
      const fen = requireFen();
      const moves = wasm.legalMoves(fen);
      if (jsonOutput) {
        console.log(JSON.stringify(moves, null, 2));
      } else {
        for (const m of moves) {
          console.log(m.notation);
        }
        console.log(`\n${moves.length} legal move(s)`);
      }
      break;
    }

    case 'eval': {
      const fen = requireFen();
      const score = wasm.evaluate(fen);
      output({ evaluation: score, unit: 'centipawns' });
      break;
    }

    case 'search': {
      const fen = requireFen();
      const result = wasm.bestMove(fen, depth);
      if (jsonOutput) {
        console.log(JSON.stringify(result, null, 2));
      } else {
        console.log(`Best move : ${result.bestMove?.notation ?? '(none)'}`);
        console.log(`Score     : ${result.score} cp`);
        console.log(`Depth     : ${result.depth}`);
        console.log(`PV        : ${result.pv.join(' ')}`);
        console.log(`Nodes     : ${result.nodes}`);
        console.log(`Time      : ${result.timeMs} ms`);
      }
      break;
    }

    case 'move': {
      const fen = requireFen();
      const moveStr = args[2];
      if (!moveStr) {
        console.error('Error: Move string required (e.g. e2e4, e7e8q)');
        process.exit(1);
      }
      const result = wasm.makeMove(fen, moveStr);
      if (jsonOutput) {
        console.log(JSON.stringify(result, null, 2));
      } else {
        console.log(result.fen);
        if (result.isCheckmate) console.log('Checkmate!');
        else if (result.isStalemate) console.log('Stalemate!');
        else if (result.isCheck) console.log('Check!');
        if (result.isInsufficientMaterial) console.log('Insufficient material');
      }
      break;
    }

    case 'play': {
      runInteractiveGame();
      break;
    }

    case 'board': {
      const fen = requireFen();
      console.log(wasm.boardToAscii(fen));
      break;
    }

    case 'version': {
      console.log('checkai 0.5.1 (wasm)');
      break;
    }

    case 'game': {
      handleGameCommand();
      break;
    }

    case 'export': {
      handleExportCommand();
      break;
    }

    case 'help':
    case '--help':
    case '-h':
    case undefined: {
      usage();
      break;
    }

    default: {
      console.error(`Unknown command: ${command}`);
      console.error('Run "checkai help" for usage information.');
      process.exit(1);
    }
  }
} catch (err) {
  console.error(`Error: ${err.message || err}`);
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Game management commands
// ---------------------------------------------------------------------------

function handleGameCommand() {
  const sub = args[1];
  switch (sub) {
    case 'new': {
      const fen = args[2];
      const id = fen ? wasm.createGameFromFen(fen) : wasm.createGame();
      if (jsonOutput) {
        console.log(JSON.stringify({ gameId: id }));
      } else {
        console.log(`Game created: ${id}`);
      }
      break;
    }
    case 'state': {
      const id = args[2];
      if (!id) {
        console.error('Error: game ID required');
        process.exit(1);
      }
      const state = wasm.gameState(id);
      if (jsonOutput) {
        console.log(JSON.stringify(state, null, 2));
      } else {
        console.log(`Game:    ${state.gameId}`);
        console.log(`FEN:     ${state.fen}`);
        console.log(`Turn:    ${state.turn}`);
        console.log(`Check:   ${state.isCheck}`);
        console.log(`Over:    ${state.isOver}`);
        if (state.result) console.log(`Result:  ${state.result}`);
        if (state.endReason) console.log(`Reason:  ${state.endReason}`);
        console.log(`Moves:   ${state.moveHistory.length}`);
        console.log(`Legal:   ${state.legalMoveCount} move(s)`);
      }
      break;
    }
    case 'move': {
      const id = args[2];
      const from = args[3];
      const to = args[4];
      const promo = args[5] || undefined;
      if (!id || !from || !to) {
        console.error(
          'Error: usage — checkai game move <ID> <FROM> <TO> [PROMOTION]'
        );
        process.exit(1);
      }
      const state = wasm.gameSubmitMove(id, from, to, promo);
      if (jsonOutput) {
        console.log(JSON.stringify(state, null, 2));
      } else {
        console.log(`FEN:   ${state.fen}`);
        if (state.isOver) {
          console.log(`Result: ${state.result} (${state.endReason})`);
        } else if (state.isCheck) {
          console.log('Check!');
        }
      }
      break;
    }
    case 'action': {
      const id = args[2];
      const action = args[3];
      const reason = args[4] || undefined;
      if (!id || !action) {
        console.error(
          'Error: usage — checkai game action <ID> <ACTION> [REASON]'
        );
        process.exit(1);
      }
      const state = wasm.gameProcessAction(id, action, reason);
      if (jsonOutput) {
        console.log(JSON.stringify(state, null, 2));
      } else {
        if (state.isOver) {
          console.log(`Game over: ${state.result} (${state.endReason})`);
        } else {
          console.log(`Action processed. Turn: ${state.turn}`);
        }
      }
      break;
    }
    case 'list': {
      const ids = wasm.listGames();
      if (jsonOutput) {
        console.log(JSON.stringify(ids));
      } else if (ids.length === 0) {
        console.log('No active games.');
      } else {
        for (const id of ids) console.log(id);
      }
      break;
    }
    case 'delete': {
      const id = args[2];
      if (!id) {
        console.error('Error: game ID required');
        process.exit(1);
      }
      wasm.deleteGame(id);
      console.log(`Game ${id} deleted.`);
      break;
    }
    default: {
      console.error(
        'Unknown game sub-command. Use: new, state, move, action, list, delete'
      );
      process.exit(1);
    }
  }
}

// ---------------------------------------------------------------------------
// Export command
// ---------------------------------------------------------------------------

function handleExportCommand() {
  const id = args[1];
  const format = (args[2] || 'pgn').toLowerCase();
  if (!id) {
    console.error('Error: usage — checkai export <ID> <FORMAT>');
    process.exit(1);
  }
  switch (format) {
    case 'pgn':
      console.log(wasm.gameToPgn(id));
      break;
    case 'json':
      console.log(wasm.gameToJson(id));
      break;
    case 'text':
      console.log(wasm.gameToText(id));
      break;
    default:
      console.error(`Unknown format: ${format}. Use: pgn, json, text`);
      process.exit(1);
  }
}

// ---------------------------------------------------------------------------
// Interactive play mode
// ---------------------------------------------------------------------------

function runInteractiveGame() {
  let fen = wasm.startingFen();

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  function printBoard(fen) {
    const placement = fen.split(' ')[0];
    const rows = placement.split('/');
    console.log('\n    a   b   c   d   e   f   g   h');
    console.log('  +---+---+---+---+---+---+---+---+');
    for (let r = 0; r < 8; r++) {
      const rank = 8 - r;
      let line = `${rank} |`;
      for (const ch of rows[r]) {
        if (ch >= '1' && ch <= '8') {
          for (let i = 0; i < parseInt(ch); i++) line += '   |';
        } else {
          line += ` ${ch} |`;
        }
      }
      console.log(line);
      console.log('  +---+---+---+---+---+---+---+---+');
    }
    console.log('    a   b   c   d   e   f   g   h\n');
  }

  function promptMove() {
    const turn = fen.split(' ')[1] === 'w' ? 'White' : 'Black';

    printBoard(fen);

    if (wasm.isCheckmate(fen)) {
      const winner = turn === 'White' ? 'Black' : 'White';
      console.log(`Checkmate! ${winner} wins.`);
      rl.close();
      return;
    }
    if (wasm.isStalemate(fen)) {
      console.log('Stalemate! Game is a draw.');
      rl.close();
      return;
    }
    if (wasm.isInsufficientMaterial(fen)) {
      console.log('Draw by insufficient material.');
      rl.close();
      return;
    }
    if (wasm.isCheck(fen)) {
      console.log('Check!');
    }

    rl.question(`${turn} to move: `, (input) => {
      const move = input.trim().toLowerCase();
      if (move === 'quit' || move === 'q') {
        console.log('Game ended.');
        rl.close();
        return;
      }
      if (move === 'moves') {
        const moves = wasm.legalMoves(fen);
        console.log(moves.map((m) => m.notation).join(', '));
        promptMove();
        return;
      }
      if (move === 'eval') {
        console.log(`Evaluation: ${wasm.evaluate(fen)} cp`);
        promptMove();
        return;
      }
      if (move === 'hint') {
        const result = wasm.bestMove(fen, 10);
        console.log(
          `Hint: ${result.bestMove?.notation ?? '?'} (${result.score} cp)`
        );
        promptMove();
        return;
      }
      if (move === 'fen') {
        console.log(fen);
        promptMove();
        return;
      }
      if (move === 'help') {
        console.log(
          'Commands: <move> (e.g. e2e4), moves, eval, hint, fen, quit'
        );
        promptMove();
        return;
      }

      try {
        const result = wasm.makeMove(fen, move);
        fen = result.fen;
        promptMove();
      } catch (err) {
        console.log(`Invalid move: ${err.message || err}`);
        promptMove();
      }
    });
  }

  console.log('CheckAI — Interactive Chess (WASM)');
  console.log(
    'Type a move (e.g. e2e4), or: moves, eval, hint, fen, help, quit\n'
  );
  promptMove();
}
