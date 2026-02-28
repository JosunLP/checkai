/**
 * CheckAI Web UI — Main Application
 * Built with @bquery/bquery (Zero-build, CDN)
 *
 * Reactive chess web interface that communicates via REST API + WebSocket
 * with the CheckAI Rust server.
 */

/* global bQuery */
const { $, $$, signal, computed, effect, batch } = bQuery;

// ============================================================================
// Configuration
// ============================================================================

const API_BASE = `${window.location.origin}/api`;
const WS_URL = `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/ws`;

// ============================================================================
// Unicode Chess Pieces
// ============================================================================

const PIECE_UNICODE = {
  K: '♔',
  Q: '♕',
  R: '♖',
  B: '♗',
  N: '♘',
  P: '♙',
  k: '♚',
  q: '♛',
  r: '♜',
  b: '♝',
  n: '♞',
  p: '♟',
};

/**
 * Returns the translated piece name for the given piece character.
 * @param {string} piece — one of K, Q, R, B, N, P
 * @returns {string}
 */
function pieceName(piece) {
  return t(`piece.${piece}`);
}

const FILES = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
const RANKS = ['1', '2', '3', '4', '5', '6', '7', '8'];

// ============================================================================
// Reactive State (Signals)
// ============================================================================

const state = {
  // Current view
  currentView: signal('dashboard'),

  // Game list
  games: signal([]),

  // Current game data
  currentGameId: signal(null),
  currentGame: signal(null),
  legalMoves: signal([]),
  isCheck: signal(false),

  // Board interaction
  selectedSquare: signal(null),
  legalTargets: signal([]),
  lastMove: signal(null),

  // Promotion pending
  pendingPromotion: signal(null),

  // Archive
  archivedGames: signal([]),
  replayData: signal(null),
  replayMoveNum: signal(0),
  replayTotalMoves: signal(0),

  // WebSocket
  wsConnected: signal(false),

  // Storage stats
  storageStats: signal(null),
};

// ============================================================================
// API Client
// ============================================================================

const api = {
  async request(method, path, body = null) {
    const opts = {
      method,
      headers: { 'Content-Type': 'application/json' },
    };
    if (body) opts.body = JSON.stringify(body);
    const res = await fetch(`${API_BASE}${path}`, opts);
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: res.statusText }));
      throw new Error(err.error || res.statusText);
    }
    const ct = res.headers.get('Content-Type') || '';
    if (ct.includes('application/json')) return res.json();
    return res.text();
  },

  createGame() {
    return this.request('POST', '/games');
  },
  listGames() {
    return this.request('GET', '/games');
  },
  getGame(id) {
    return this.request('GET', `/games/${id}`);
  },
  deleteGame(id) {
    return this.request('DELETE', `/games/${id}`);
  },
  submitMove(id, from, to, promotion) {
    const body = { from, to };
    if (promotion) body.promotion = promotion;
    return this.request('POST', `/games/${id}/move`, body);
  },
  submitAction(id, action, reason) {
    const body = { action };
    if (reason) body.reason = reason;
    return this.request('POST', `/games/${id}/action`, body);
  },
  getLegalMoves(id) {
    return this.request('GET', `/games/${id}/moves`);
  },
  listArchived() {
    return this.request('GET', '/archive');
  },
  getArchived(id) {
    return this.request('GET', `/archive/${id}`);
  },
  replayArchived(id, moveNum) {
    const q = moveNum !== undefined ? `?move_number=${moveNum}` : '';
    return this.request('GET', `/archive/${id}/replay${q}`);
  },
  getStorageStats() {
    return this.request('GET', '/archive/stats');
  },
};

// ============================================================================
// WebSocket Manager
// ============================================================================

let ws = null;
let wsReconnectTimer = null;

function connectWebSocket() {
  if (
    ws &&
    (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)
  )
    return;

  try {
    ws = new WebSocket(WS_URL);
  } catch (e) {
    console.warn('WebSocket connection failed:', e);
    scheduleReconnect();
    return;
  }

  ws.onopen = () => {
    state.wsConnected.value = true;
    if (wsReconnectTimer) {
      clearTimeout(wsReconnectTimer);
      wsReconnectTimer = null;
    }
    // Subscribe to current game if any
    if (state.currentGameId.value) {
      wsSubscribe(state.currentGameId.value);
    }
  };

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      handleWsMessage(msg);
    } catch (e) {
      console.warn('WS message parse error:', e);
    }
  };

  ws.onclose = () => {
    state.wsConnected.value = false;
    scheduleReconnect();
  };

  ws.onerror = () => {
    state.wsConnected.value = false;
  };
}

function scheduleReconnect() {
  if (wsReconnectTimer) return;
  wsReconnectTimer = setTimeout(() => {
    wsReconnectTimer = null;
    connectWebSocket();
  }, 3000);
}

function wsSend(payload) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(payload));
  }
}

function wsSubscribe(gameId) {
  wsSend({ action: 'subscribe', game_id: gameId });
}

function wsUnsubscribe(gameId) {
  wsSend({ action: 'unsubscribe', game_id: gameId });
}

function handleWsMessage(msg) {
  if (msg.type === 'event') {
    switch (msg.event) {
      case 'game_updated':
        if (state.currentGameId.value === msg.game_id) {
          // Refresh the game from the event data
          refreshCurrentGame();
        }
        // Refresh game list
        refreshGameList();
        break;
      case 'game_created':
        refreshGameList();
        showToast(t('toast.new_game_created'), 'info');
        break;
      case 'game_deleted':
        refreshGameList();
        if (state.currentGameId.value === msg.game_id) {
          state.currentGameId.value = null;
          state.currentGame.value = null;
          navigateTo('dashboard');
          showToast(t('toast.current_game_deleted'), 'warning');
        }
        break;
    }
  }
}

// ============================================================================
// Navigation
// ============================================================================

function navigateTo(view) {
  state.currentView.value = view;
}

// Effect: Update DOM when view changes
effect(() => {
  const view = state.currentView.value;
  document
    .querySelectorAll('.view')
    .forEach((el) => el.classList.remove('active'));
  const target = document.getElementById(`view-${view}`);
  if (target) target.classList.add('active');

  document.querySelectorAll('.nav-btn').forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.view === view);
  });

  // Re-render game view when switching to it so stale content is cleared
  if (view === 'game') {
    renderGameView();
  }
});

// ============================================================================
// WebSocket Status Effect
// ============================================================================

effect(() => {
  const connected = state.wsConnected.value;
  const indicator = document.getElementById('ws-status');
  const label = document.getElementById('ws-label');
  if (indicator) {
    indicator.classList.toggle('connected', connected);
    indicator.classList.toggle('disconnected', !connected);
  }
  if (label) {
    label.textContent = connected ? t('ws.connected') : t('ws.disconnected');
  }
});

// ============================================================================
// Board Rendering
// ============================================================================

function renderBoard(containerId, boardMap, options = {}) {
  const container = document.getElementById(containerId);
  if (!container) return;

  const {
    interactive = false,
    selectedSq = null,
    legalTargetSqs = [],
    lastMoveSqs = [],
    checkSquare = null,
  } = options;

  container.innerHTML = '';

  // Render from rank 8 (top) to rank 1 (bottom)
  for (let rank = 7; rank >= 0; rank--) {
    for (let file = 0; file < 8; file++) {
      const sq = FILES[file] + RANKS[rank];
      const isLight = (file + rank) % 2 === 1;
      const piece = boardMap ? boardMap[sq] : null;

      const div = document.createElement('div');
      div.className = `square ${isLight ? 'light' : 'dark'}`;
      div.dataset.square = sq;

      // Highlights
      if (selectedSq === sq) div.classList.add('selected');
      if (legalTargetSqs.includes(sq)) {
        if (piece) {
          div.classList.add('legal-capture');
        } else {
          div.classList.add('legal-move');
        }
      }
      if (lastMoveSqs.includes(sq)) div.classList.add('last-move');
      if (checkSquare === sq) div.classList.add('in-check');

      // Labels
      if (rank === 0) {
        const fl = document.createElement('span');
        fl.className = 'file-label';
        fl.textContent = FILES[file];
        div.appendChild(fl);
      }
      if (file === 0) {
        const rl = document.createElement('span');
        rl.className = 'rank-label';
        rl.textContent = RANKS[rank];
        div.appendChild(rl);
      }

      // Piece
      if (piece) {
        const span = document.createElement('span');
        const unicode = PIECE_UNICODE[piece];
        span.className = `piece ${piece === piece.toUpperCase() ? 'white-piece' : 'black-piece'}`;
        span.textContent = unicode || piece;
        div.appendChild(span);
      }

      // Interactive click handler
      if (interactive) {
        div.addEventListener('click', () => handleSquareClick(sq, piece));
      }

      container.appendChild(div);
    }
  }
}

function renderMiniBoard(boardMap) {
  let html = '';
  for (let rank = 7; rank >= 0; rank--) {
    for (let file = 0; file < 8; file++) {
      const sq = FILES[file] + RANKS[rank];
      const isLight = (file + rank) % 2 === 1;
      const piece = boardMap ? boardMap[sq] : null;
      const pieceChar = piece ? PIECE_UNICODE[piece] || '' : '';
      html += `<div class="mini-sq ${isLight ? 'light' : 'dark'}">${pieceChar}</div>`;
    }
  }
  return html;
}

// ============================================================================
// Board Interaction
// ============================================================================

function handleSquareClick(sq, piece) {
  const game = state.currentGame.value;
  if (!game || game.is_over) return;

  const selected = state.selectedSquare.value;
  const legalTargets = state.legalTargets.value;

  if (selected && legalTargets.includes(sq)) {
    // Make a move
    attemptMove(selected, sq);
    return;
  }

  if (piece && isPieceOfCurrentTurn(piece)) {
    // Select this piece
    selectSquare(sq);
  } else {
    // Deselect
    state.selectedSquare.value = null;
    state.legalTargets.value = [];
    renderCurrentBoard();
  }
}

function isPieceOfCurrentTurn(fenChar) {
  const game = state.currentGame.value;
  if (!game) return false;
  const isWhite = fenChar === fenChar.toUpperCase();
  return (
    (game.state.turn === 'white' && isWhite) ||
    (game.state.turn === 'black' && !isWhite)
  );
}

function selectSquare(sq) {
  state.selectedSquare.value = sq;

  // Find legal moves from this square
  const moves = state.legalMoves.value || [];
  const targets = moves.filter((m) => m.from === sq).map((m) => m.to);
  state.legalTargets.value = [...new Set(targets)];

  renderCurrentBoard();
}

async function attemptMove(from, to) {
  const moves = state.legalMoves.value || [];

  // Check if this is a promotion move
  const promoMoves = moves.filter(
    (m) => m.from === from && m.to === to && m.promotion
  );
  if (promoMoves.length > 0) {
    // Show promotion dialog
    state.pendingPromotion.value = { from, to };
    showPromotionDialog();
    return;
  }

  await executeMove(from, to);
}

async function executeMove(from, to, promotion = null) {
  const gameId = state.currentGameId.value;
  if (!gameId) return;

  try {
    const res = await api.submitMove(gameId, from, to, promotion);

    batch(() => {
      state.selectedSquare.value = null;
      state.legalTargets.value = [];
      state.lastMove.value = { from, to };
      state.pendingPromotion.value = null;
    });

    hidePromotionDialog();
    showGameMessage(res.message, res.is_over ? 'warning' : 'success');
    await refreshCurrentGame();
  } catch (err) {
    showGameMessage(err.message, 'error');
    showToast(t('toast.invalid_move', { error: err.message }), 'error');
  }
}

function showPromotionDialog() {
  const dialog = document.getElementById('promotion-dialog');
  if (!dialog) return;

  const game = state.currentGame.value;
  const isWhite = game && game.state.turn === 'white';

  // Set correct piece symbols
  const pieces = dialog.querySelectorAll('.promotion-btn');
  const promoMap = isWhite
    ? { Q: '♕', R: '♖', B: '♗', N: '♘' }
    : { Q: '♛', R: '♜', B: '♝', N: '♞' };

  pieces.forEach((btn) => {
    const p = btn.dataset.piece;
    btn.querySelector('.promo-piece').textContent = promoMap[p] || '';
  });

  dialog.style.display = 'flex';
}

function hidePromotionDialog() {
  const dialog = document.getElementById('promotion-dialog');
  if (dialog) dialog.style.display = 'none';
}

// ============================================================================
// Game Data Management
// ============================================================================

async function refreshGameList() {
  try {
    const data = await api.listGames();
    state.games.value = data.games || [];
    renderGameList();
  } catch (err) {
    console.error('Failed to load games:', err);
  }
}

async function refreshCurrentGame() {
  const gameId = state.currentGameId.value;
  if (!gameId) return;

  try {
    const game = await api.getGame(gameId);
    state.currentGame.value = game;
    state.isCheck.value = game.is_check;

    // Fetch legal moves if game is not over
    if (!game.is_over) {
      const movesData = await api.getLegalMoves(gameId);
      state.legalMoves.value = movesData.moves || [];
    } else {
      state.legalMoves.value = [];
    }

    renderGameView();
  } catch (err) {
    console.error('Failed to load game:', err);
    showToast(t('toast.error', { error: err.message }), 'error');
  }
}

async function loadGame(gameId) {
  // Unsubscribe from previous game
  if (state.currentGameId.value && state.currentGameId.value !== gameId) {
    wsUnsubscribe(state.currentGameId.value);
  }

  batch(() => {
    state.currentGameId.value = gameId;
    state.currentGame.value = null;
    state.selectedSquare.value = null;
    state.legalTargets.value = [];
    state.lastMove.value = null;
  });

  navigateTo('game');
  wsSubscribe(gameId);
  await refreshCurrentGame();
}

async function createNewGame() {
  try {
    const res = await api.createGame();
    showToast(t('toast.new_game_created'), 'success');
    await refreshGameList();
    await loadGame(res.game_id);
  } catch (err) {
    showToast(t('toast.error', { error: err.message }), 'error');
  }
}

async function deleteCurrentGame() {
  const gameId = state.currentGameId.value;
  if (!gameId) return;
  if (!confirm(t('confirm.delete'))) return;

  try {
    await api.deleteGame(gameId);
    batch(() => {
      state.currentGameId.value = null;
      state.currentGame.value = null;
    });
    showToast(t('toast.game_deleted'), 'info');
    navigateTo('dashboard');
    await refreshGameList();
  } catch (err) {
    showToast(t('toast.error', { error: err.message }), 'error');
  }
}

// ============================================================================
// DOM Rendering
// ============================================================================

function renderGameList() {
  const container = document.getElementById('games-list');
  const noGames = document.getElementById('no-games');
  const games = state.games.value;

  if (!container) return;

  // Clear existing game cards (keep empty state)
  container.querySelectorAll('.game-card').forEach((el) => el.remove());

  if (games.length === 0) {
    if (noGames) noGames.style.display = 'block';
    return;
  }

  if (noGames) noGames.style.display = 'none';

  games.forEach((game) => {
    const card = document.createElement('div');
    card.className = 'game-card';
    card.addEventListener('click', () => loadGame(game.game_id));

    const turnLabel =
      game.turn === 'white' ? t('game.turn_white') : t('game.turn_black');
    const badge = game.is_over
      ? `<span class="game-card-badge badge-over">${t('game.badge_over')}</span>`
      : `<span class="game-card-badge badge-active">${t('game.badge_active')}</span>`;

    let resultText = '';
    if (game.result) {
      const resultMap = {
        WhiteWins: t('result.white_wins'),
        BlackWins: t('result.black_wins'),
        Draw: t('result.draw'),
      };
      resultText = `<div class="game-card-result">${resultMap[game.result] || game.result}</div>`;
    }

    card.innerHTML = `
      <div class="game-card-header">
        <span class="game-card-id" title="${game.game_id}">${game.game_id.substring(0, 8)}…</span>
        ${badge}
      </div>
      <div class="game-card-info">
        <span>${t('game.turn')}: ${turnLabel}</span>
        <span>${t('game.move_number')} ${game.fullmove_number}</span>
      </div>
      ${resultText}
    `;

    container.appendChild(card);
  });
}

function renderGameView() {
  const game = state.currentGame.value;
  const placeholder = document.getElementById('game-placeholder');
  const gameContainer = document.getElementById('game-container');

  if (!game) {
    if (placeholder) placeholder.style.display = 'block';
    if (gameContainer) gameContainer.style.display = 'none';
    return;
  }

  if (placeholder) placeholder.style.display = 'none';
  if (gameContainer) gameContainer.style.display = 'grid';

  // Update info panel
  updateElement('info-game-id', game.game_id.substring(0, 12) + '…');
  updateElement(
    'info-turn',
    game.state.turn === 'white' ? t('game.turn_white') : t('game.turn_black')
  );
  updateElement('info-move-num', game.state.fullmove_number);
  updateElement(
    'info-status',
    game.is_over ? t('game.status_over') : t('game.status_active')
  );
  updateElement(
    'info-check',
    game.is_check ? t('game.check_yes') : t('game.check_no')
  );
  updateElement('info-legal-moves', game.legal_move_count);

  // Castling rights
  const c = game.state.castling;
  updateElement('castle-wk', c.white.kingside ? '✓' : '✗');
  updateElement('castle-wq', c.white.queenside ? '✓' : '✗');
  updateElement('castle-bk', c.black.kingside ? '✓' : '✗');
  updateElement('castle-bq', c.black.queenside ? '✓' : '✗');

  // Player bars
  const whiteBar = document.querySelector('.white-bar');
  const blackBar = document.querySelector('.black-bar');
  if (whiteBar)
    whiteBar.classList.toggle('active-turn', game.state.turn === 'white');
  if (blackBar)
    blackBar.classList.toggle('active-turn', game.state.turn === 'black');

  // Player status
  const ws = document.getElementById('white-status');
  const bs = document.getElementById('black-status');
  if (ws)
    ws.textContent = game.state.turn === 'white' ? t('game.your_turn') : '';
  if (bs)
    bs.textContent = game.state.turn === 'black' ? t('game.your_turn') : '';

  // Disable actions if game is over
  const actions = document.getElementById('game-actions');
  if (actions) {
    const btns = actions.querySelectorAll('.btn');
    btns.forEach((btn) => {
      if (btn.id !== 'btn-delete-game') {
        btn.disabled = game.is_over;
      }
    });
  }

  // Render move history
  renderMoveHistory(game.move_history);

  // Render board
  renderCurrentBoard();
}

function renderCurrentBoard() {
  const game = state.currentGame.value;
  if (!game) return;

  const lastMoveData = state.lastMove.value;
  const lastMoveSqs = lastMoveData ? [lastMoveData.from, lastMoveData.to] : [];

  // Also get last move from history if no interaction yet
  const mh = game.move_history;
  if (lastMoveSqs.length === 0 && mh && mh.length > 0) {
    const last = mh[mh.length - 1];
    if (last.move_json) {
      lastMoveSqs.push(last.move_json.from, last.move_json.to);
    }
  }

  // Find king square for check highlight
  let checkSquare = null;
  if (game.is_check) {
    const board = game.state.board;
    const kingChar = game.state.turn === 'white' ? 'K' : 'k';
    for (const [sq, p] of Object.entries(board)) {
      if (p === kingChar) {
        checkSquare = sq;
        break;
      }
    }
  }

  renderBoard('chess-board', game.state.board, {
    interactive: true,
    selectedSq: state.selectedSquare.value,
    legalTargetSqs: state.legalTargets.value,
    lastMoveSqs,
    checkSquare,
  });
}

function renderMoveHistory(history) {
  const container = document.getElementById('move-history');
  if (!container) return;

  if (!history || history.length === 0) {
    container.innerHTML = `<div class="empty-hint">${t('game.no_moves')}</div>`;
    return;
  }

  // Group by move number
  const grouped = {};
  history.forEach((m) => {
    if (!grouped[m.move_number]) grouped[m.move_number] = {};
    if (m.side === 'white') {
      grouped[m.move_number].white = m.notation;
    } else {
      grouped[m.move_number].black = m.notation;
    }
  });

  let html = '';
  for (const [num, moves] of Object.entries(grouped)) {
    html += `<div class="move-row">
      <span class="move-num">${num}.</span>
      <span class="move-white">${moves.white || ''}</span>
      <span class="move-black">${moves.black || ''}</span>
    </div>`;
  }

  container.innerHTML = html;
  container.scrollTop = container.scrollHeight;
}

// ============================================================================
// Archive
// ============================================================================

async function refreshArchiveList() {
  try {
    const data = await api.listArchived();
    state.archivedGames.value = data.games || [];
    renderArchiveList();

    // Also update stats
    if (data.storage) {
      state.storageStats.value = data.storage;
      renderStorageStats(data.storage);
    }
  } catch (err) {
    console.error('Failed to load archive:', err);
    showToast(t('toast.load_archive_failed', { error: err.message }), 'error');
  }
}

function renderArchiveList() {
  const container = document.getElementById('archive-list');
  const noArchive = document.getElementById('no-archive');
  const games = state.archivedGames.value;

  if (!container) return;

  container.querySelectorAll('.game-card').forEach((el) => el.remove());

  if (games.length === 0) {
    if (noArchive) noArchive.style.display = 'block';
    return;
  }

  if (noArchive) noArchive.style.display = 'none';

  games.forEach((game) => {
    const card = document.createElement('div');
    card.className = 'game-card archive-card';
    card.addEventListener('click', () =>
      openReplay(game.game_id, game.move_count)
    );

    const resultMap = {
      WhiteWins: { text: t('result.white_wins'), cls: 'result-white' },
      BlackWins: { text: t('result.black_wins'), cls: 'result-black' },
      Draw: { text: t('result.draw'), cls: 'result-draw' },
    };
    const result = resultMap[game.result] || {
      text: game.result || '?',
      cls: '',
    };

    const reason = game.end_reason
      ? t(`reason.${game.end_reason}`) || game.end_reason
      : '';

    const date = game.start_timestamp
      ? new Date(game.start_timestamp * 1000).toLocaleString(
          getLocale() === 'zh-CN' ? 'zh-CN' : getLocale()
        )
      : '';

    card.innerHTML = `
      <div class="game-card-header">
        <span class="game-card-id" title="${game.game_id}">${game.game_id.substring(0, 8)}…</span>
        <span class="game-card-badge badge-over">${t('archive.badge')}</span>
      </div>
      <div class="game-card-info">
        <span>${t('archive.half_moves', { n: game.move_count })}</span>
        <span>${formatBytes(game.compressed_bytes)}</span>
      </div>
      <div class="game-card-result ${result.cls}">${result.text}</div>
      <div class="game-card-meta">${reason}${date ? ` — ${date}` : ''}</div>
    `;

    container.appendChild(card);
  });
}

async function openReplay(gameId, totalMoves) {
  const replaySection = document.getElementById('archive-replay');
  if (!replaySection) return;

  state.replayTotalMoves.value = totalMoves || 0;
  state.replayMoveNum.value = totalMoves || 0; // start at end

  const slider = document.getElementById('replay-slider');
  if (slider) {
    slider.max = totalMoves || 0;
    slider.value = totalMoves || 0;
  }

  updateElement('replay-total-moves', totalMoves || 0);
  updateElement(
    'replay-title',
    t('archive.replay_title', { id: gameId.substring(0, 8) + '…' })
  );

  replaySection.style.display = 'block';
  replaySection.dataset.gameId = gameId;

  await loadReplayPosition(gameId, totalMoves || 0);
}

async function loadReplayPosition(gameId, moveNum) {
  try {
    const data = await api.replayArchived(gameId, moveNum);
    state.replayData.value = data;
    state.replayMoveNum.value = data.at_move;
    updateElement('replay-move-num', data.at_move);

    renderBoard('replay-board', data.state.board, { interactive: false });
  } catch (err) {
    showToast(t('toast.replay_failed', { error: err.message }), 'error');
  }
}

// ============================================================================
// Storage Stats
// ============================================================================

async function refreshStorageStats() {
  try {
    const stats = await api.getStorageStats();
    state.storageStats.value = stats;
    renderStorageStats(stats);
  } catch (err) {
    console.error('Failed to load stats:', err);
  }
}

function renderStorageStats(stats) {
  if (!stats) return;
  updateElement('stat-active', stats.active_count);
  updateElement('stat-archived', stats.archived_count);
  updateElement('stat-active-bytes', formatBytes(stats.active_bytes));
  updateElement('stat-archive-bytes', formatBytes(stats.archive_bytes));
}

// ============================================================================
// Game Actions
// ============================================================================

async function resign() {
  const gameId = state.currentGameId.value;
  if (!gameId) return;
  if (!confirm(t('confirm.resign'))) return;

  try {
    const res = await api.submitAction(gameId, 'resign');
    showGameMessage(res.message, 'warning');
    await refreshCurrentGame();
    await refreshGameList();
  } catch (err) {
    showToast(t('toast.error', { error: err.message }), 'error');
  }
}

async function offerDraw() {
  const gameId = state.currentGameId.value;
  if (!gameId) return;

  try {
    const res = await api.submitAction(gameId, 'offer_draw');
    showGameMessage(res.message, 'info');
    await refreshCurrentGame();
  } catch (err) {
    showToast(t('toast.error', { error: err.message }), 'error');
  }
}

async function claimDraw() {
  const gameId = state.currentGameId.value;
  if (!gameId) return;

  // Ask for reason
  const reason = prompt(t('confirm.claim_draw_reason'));
  if (!reason) return;

  try {
    const res = await api.submitAction(gameId, 'claim_draw', reason);
    showGameMessage(res.message, 'info');
    await refreshCurrentGame();
    await refreshGameList();
  } catch (err) {
    showToast(t('toast.error', { error: err.message }), 'error');
  }
}

async function submitMoveFromInput() {
  const from = document
    .getElementById('input-from')
    ?.value?.trim()
    .toLowerCase();
  const to = document.getElementById('input-to')?.value?.trim().toLowerCase();
  if (!from || !to) {
    showToast(t('toast.enter_from_to'), 'warning');
    return;
  }

  const gameId = state.currentGameId.value;
  if (!gameId) return;

  // Check if promotion is needed
  const moves = state.legalMoves.value || [];
  const promoMoves = moves.filter(
    (m) => m.from === from && m.to === to && m.promotion
  );
  if (promoMoves.length > 0) {
    state.pendingPromotion.value = { from, to };
    showPromotionDialog();
    return;
  }

  try {
    const res = await api.submitMove(gameId, from, to);
    state.lastMove.value = { from, to };
    showGameMessage(res.message, res.is_over ? 'warning' : 'success');
    document.getElementById('input-from').value = '';
    document.getElementById('input-to').value = '';
    await refreshCurrentGame();
  } catch (err) {
    showGameMessage(err.message, 'error');
    showToast(t('toast.invalid_move', { error: err.message }), 'error');
  }
}

// ============================================================================
// UI Helpers
// ============================================================================

function updateElement(id, value) {
  const el = document.getElementById(id);
  if (el) el.textContent = String(value);
}

function formatBytes(bytes) {
  if (bytes === 0 || bytes == null) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function showGameMessage(text, type = 'info') {
  const el = document.getElementById('game-message');
  if (!el) return;
  el.textContent = text;
  el.className = `game-message msg-${type}`;
  el.style.display = 'block';
  setTimeout(() => {
    el.style.display = 'none';
  }, 6000);
}

function showToast(message, type = 'info') {
  const container = document.getElementById('toast-container');
  if (!container) return;

  const toast = document.createElement('div');
  toast.className = `toast toast-${type}`;
  toast.textContent = message;
  container.appendChild(toast);

  setTimeout(() => {
    toast.classList.add('toast-out');
    setTimeout(() => toast.remove(), 250);
  }, 4000);
}

// ============================================================================
// Event Binding
// ============================================================================

function bindEvents() {
  // Navigation
  document.querySelectorAll('.nav-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const view = btn.dataset.view;
      navigateTo(view);

      if (view === 'dashboard') {
        refreshGameList();
        refreshStorageStats();
      } else if (view === 'archive') {
        refreshArchiveList();
      }
    });
  });

  // New game
  const newGameBtn = document.getElementById('btn-new-game');
  if (newGameBtn) newGameBtn.addEventListener('click', createNewGame);

  // Game actions
  const resignBtn = document.getElementById('btn-resign');
  if (resignBtn) resignBtn.addEventListener('click', resign);

  const drawOfferBtn = document.getElementById('btn-offer-draw');
  if (drawOfferBtn) drawOfferBtn.addEventListener('click', offerDraw);

  const drawClaimBtn = document.getElementById('btn-claim-draw');
  if (drawClaimBtn) drawClaimBtn.addEventListener('click', claimDraw);

  const deleteBtn = document.getElementById('btn-delete-game');
  if (deleteBtn) deleteBtn.addEventListener('click', deleteCurrentGame);

  // Submit move from input
  const submitMoveBtn = document.getElementById('btn-submit-move');
  if (submitMoveBtn)
    submitMoveBtn.addEventListener('click', submitMoveFromInput);

  // Enter key on move input
  const inputTo = document.getElementById('input-to');
  if (inputTo) {
    inputTo.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') submitMoveFromInput();
    });
  }

  const inputFrom = document.getElementById('input-from');
  if (inputFrom) {
    inputFrom.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') document.getElementById('input-to')?.focus();
    });
  }

  // Promotion buttons
  document.querySelectorAll('.promotion-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const piece = btn.dataset.piece;
      const pending = state.pendingPromotion.value;
      if (pending) {
        executeMove(pending.from, pending.to, piece);
      }
    });
  });

  // Refresh stats
  const refreshStatsBtn = document.getElementById('btn-refresh-stats');
  if (refreshStatsBtn)
    refreshStatsBtn.addEventListener('click', refreshStorageStats);

  // Archive
  const refreshArchiveBtn = document.getElementById('btn-refresh-archive');
  if (refreshArchiveBtn)
    refreshArchiveBtn.addEventListener('click', refreshArchiveList);

  // Close replay
  const closeReplayBtn = document.getElementById('btn-close-replay');
  if (closeReplayBtn) {
    closeReplayBtn.addEventListener('click', () => {
      const section = document.getElementById('archive-replay');
      if (section) section.style.display = 'none';
    });
  }

  // Replay controls
  const slider = document.getElementById('replay-slider');
  if (slider) {
    slider.addEventListener('input', async () => {
      const section = document.getElementById('archive-replay');
      const gameId = section?.dataset.gameId;
      if (gameId) {
        await loadReplayPosition(gameId, parseInt(slider.value));
      }
    });
  }

  const replayStart = document.getElementById('replay-start');
  if (replayStart) {
    replayStart.addEventListener('click', async () => {
      const section = document.getElementById('archive-replay');
      const gameId = section?.dataset.gameId;
      if (gameId) {
        const slider = document.getElementById('replay-slider');
        if (slider) slider.value = 0;
        await loadReplayPosition(gameId, 0);
      }
    });
  }

  const replayEnd = document.getElementById('replay-end');
  if (replayEnd) {
    replayEnd.addEventListener('click', async () => {
      const section = document.getElementById('archive-replay');
      const gameId = section?.dataset.gameId;
      if (gameId) {
        const total = state.replayTotalMoves.value;
        const slider = document.getElementById('replay-slider');
        if (slider) slider.value = total;
        await loadReplayPosition(gameId, total);
      }
    });
  }

  const replayPrev = document.getElementById('replay-prev');
  if (replayPrev) {
    replayPrev.addEventListener('click', async () => {
      const section = document.getElementById('archive-replay');
      const gameId = section?.dataset.gameId;
      if (gameId) {
        const curr = state.replayMoveNum.value;
        const next = Math.max(0, curr - 1);
        const slider = document.getElementById('replay-slider');
        if (slider) slider.value = next;
        await loadReplayPosition(gameId, next);
      }
    });
  }

  const replayNext = document.getElementById('replay-next');
  if (replayNext) {
    replayNext.addEventListener('click', async () => {
      const section = document.getElementById('archive-replay');
      const gameId = section?.dataset.gameId;
      if (gameId) {
        const curr = state.replayMoveNum.value;
        const total = state.replayTotalMoves.value;
        const next = Math.min(total, curr + 1);
        const slider = document.getElementById('replay-slider');
        if (slider) slider.value = next;
        await loadReplayPosition(gameId, next);
      }
    });
  }
}

// ============================================================================
// Initialization
// ============================================================================

async function init() {
  console.log('CheckAI Web UI initializing...');

  // Initialize i18n (detect locale, translate static DOM)
  initI18n();

  // Populate language selector
  const langSelect = document.getElementById('lang-select');
  if (langSelect) {
    SUPPORTED_LOCALES.forEach((loc) => {
      const opt = document.createElement('option');
      opt.value = loc.code;
      opt.textContent = loc.name;
      langSelect.appendChild(opt);
    });
    langSelect.value = getLocale();
    langSelect.addEventListener('change', () => {
      setLocale(langSelect.value);
      // Re-render dynamic content with new locale
      renderGameList();
      if (state.currentGame.value) renderGameView();
      renderArchiveList();
      const stats = state.storageStats.value;
      if (stats) renderStorageStats(stats);
      // Update WS label based on actual connection state (not data-i18n)
      const wsLabel = document.getElementById('ws-label');
      if (wsLabel) {
        wsLabel.textContent = state.wsConnected.value
          ? t('ws.connected')
          : t('ws.disconnected');
      }
    });
  }

  // Bind DOM events
  bindEvents();

  // Connect WebSocket
  connectWebSocket();

  // Initial data load
  await Promise.all([refreshGameList(), refreshStorageStats()]);

  console.log('CheckAI Web UI ready.');
}

// Start when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
