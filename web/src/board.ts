// ============================================================================
// CheckAI Web UI — Board Renderer
// ============================================================================

import { store } from './store';
import type { BoardMap, FenChar, SquareName } from './types';
import { FILES, PIECE_UNICODE, RANKS } from './types';

export interface BoardRenderOptions {
  interactive?: boolean;
  selectedSq?: SquareName | null;
  legalTargetSqs?: SquareName[];
  lastMoveSqs?: SquareName[];
  checkSquare?: SquareName | null;
  flipped?: boolean;
  onClick?: (sq: SquareName, piece: FenChar | null) => void;
}

/**
 * Renders a chess board into the given container element.
 * Supports interactive highlighting, legal-move indicators,
 * last-move highlighting, check indication, and board flipping.
 */
export function renderBoard(
  containerId: string,
  boardMap: BoardMap | null,
  options: BoardRenderOptions = {}
): void {
  const container = document.getElementById(containerId);
  if (!container) return;

  const {
    interactive = false,
    selectedSq = null,
    legalTargetSqs = [],
    lastMoveSqs = [],
    checkSquare = null,
    flipped = false,
    onClick,
  } = options;

  container.innerHTML = '';

  const rankOrder = flipped
    ? [0, 1, 2, 3, 4, 5, 6, 7]
    : [7, 6, 5, 4, 3, 2, 1, 0];
  const fileOrder = flipped
    ? [7, 6, 5, 4, 3, 2, 1, 0]
    : [0, 1, 2, 3, 4, 5, 6, 7];

  for (const rank of rankOrder) {
    for (const file of fileOrder) {
      const sq: SquareName = FILES[file] + RANKS[rank];
      const isLight = (file + rank) % 2 === 1;
      const piece = boardMap ? (boardMap[sq] as FenChar | null) : null;

      const div = document.createElement('div');
      div.className = `square ${isLight ? 'light' : 'dark'}`;
      div.dataset.square = sq;

      // Highlights
      if (selectedSq === sq) div.classList.add('selected');
      if (legalTargetSqs.includes(sq)) {
        div.classList.add(piece ? 'legal-capture' : 'legal-move');
      }
      if (lastMoveSqs.includes(sq)) div.classList.add('last-move');
      if (checkSquare === sq) div.classList.add('in-check');

      // Rank & file labels
      const isBottomRank = rank === (flipped ? 7 : 0);
      const isLeftFile = file === (flipped ? 7 : 0);
      if (isBottomRank) {
        const fl = document.createElement('span');
        fl.className = 'file-label';
        fl.textContent = FILES[file];
        div.appendChild(fl);
      }
      if (isLeftFile) {
        const rl = document.createElement('span');
        rl.className = 'rank-label';
        rl.textContent = RANKS[rank];
        div.appendChild(rl);
      }

      // Piece
      if (piece) {
        const span = document.createElement('span');
        span.className = `piece ${piece === piece.toUpperCase() ? 'white-piece' : 'black-piece'}`;
        span.textContent = PIECE_UNICODE[piece] || piece;
        div.appendChild(span);
      }

      // Click handler
      if (interactive && onClick) {
        div.addEventListener('click', () => onClick(sq, piece));
      }

      container.appendChild(div);
    }
  }
}

/**
 * Renders a tiny uninteractive mini-board and returns the HTML string.
 */
export function renderMiniBoard(boardMap: BoardMap | null): string {
  let html = '';
  for (let rank = 7; rank >= 0; rank--) {
    for (let file = 0; file < 8; file++) {
      const sq: SquareName = FILES[file] + RANKS[rank];
      const isLight = (file + rank) % 2 === 1;
      const piece = boardMap ? (boardMap[sq] as FenChar | null) : null;
      const ch = piece ? PIECE_UNICODE[piece] || '' : '';
      html += `<div class="mini-sq ${isLight ? 'light' : 'dark'}">${ch}</div>`;
    }
  }
  return html;
}

/**
 * Re-renders the current game board with reactive state.
 */
export function renderCurrentBoard(): void {
  const game = store.currentGame.value;
  if (!game) return;

  const lastMoveData = store.lastMove.value;
  const lastMoveSqs: SquareName[] = lastMoveData
    ? [lastMoveData.from, lastMoveData.to]
    : [];

  // Fallback to move history's last move
  if (lastMoveSqs.length === 0 && game.move_history?.length > 0) {
    const last = game.move_history[game.move_history.length - 1];
    if (last.move_json) {
      lastMoveSqs.push(last.move_json.from, last.move_json.to);
    }
  }

  // Find king square for check highlight
  let checkSquare: SquareName | null = null;
  if (game.is_check) {
    const kingChar = game.state.turn === 'white' ? 'K' : 'k';
    for (const [sq, p] of Object.entries(game.state.board)) {
      if (p === kingChar) {
        checkSquare = sq;
        break;
      }
    }
  }

  renderBoard('chess-board', game.state.board, {
    interactive: true,
    selectedSq: store.selectedSquare.value,
    legalTargetSqs: store.legalTargets.value,
    lastMoveSqs,
    checkSquare,
    flipped: store.boardFlipped.value,
    onClick: handleSquareClick,
  });
}

// ── Board Interaction ──────────────────────────────────────────────────────

function handleSquareClick(sq: SquareName, piece: FenChar | null): void {
  const game = store.currentGame.value;
  if (!game || game.is_over) return;

  const selected = store.selectedSquare.value;
  const legalTargets = store.legalTargets.value;

  if (selected && legalTargets.includes(sq)) {
    // Attempt move — imported dynamically to avoid circular dependency
    import('./game').then(({ attemptMove }) => attemptMove(selected, sq));
    return;
  }

  if (piece && isPieceOfCurrentTurn(piece)) {
    selectSquare(sq);
  } else {
    store.selectedSquare.value = null;
    store.legalTargets.value = [];
    renderCurrentBoard();
  }
}

function isPieceOfCurrentTurn(fenChar: string): boolean {
  const game = store.currentGame.value;
  if (!game) return false;
  const isWhite = fenChar === fenChar.toUpperCase();
  return (
    (game.state.turn === 'white' && isWhite) ||
    (game.state.turn === 'black' && !isWhite)
  );
}

function selectSquare(sq: SquareName): void {
  store.selectedSquare.value = sq;

  const moves = store.legalMoves.value || [];
  const targets = moves.filter((m) => m.from === sq).map((m) => m.to);
  store.legalTargets.value = [...new Set(targets)];

  renderCurrentBoard();
}
