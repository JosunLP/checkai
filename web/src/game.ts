// ============================================================================
// CheckAI Web UI — Game Logic Module
// ============================================================================

import { batch } from '@bquery/bquery/reactive';
import * as api from './api';
import { renderCurrentBoard } from './board';
import { t } from './i18n';
import { store } from './store';
import type { MoveHistoryEntry } from './types';
import { formatBytes, setText, showGameMessage, showToast } from './ui';
import { wsSubscribe, wsUnsubscribe } from './ws';

// ============================================================================
// Game Data Management
// ============================================================================

export async function refreshGameList(): Promise<void> {
  try {
    const data = await api.listGames();
    store.games.value = data.games || [];
    renderGameList();
  } catch (err) {
    console.error('Failed to load games:', err);
  }
}

export async function refreshCurrentGame(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId) return;

  try {
    const game = await api.getGame(gameId);
    store.currentGame.value = game;
    store.isCheck.value = game.is_check;

    if (!game.is_over) {
      const movesData = await api.getLegalMoves(gameId);
      store.legalMoves.value = movesData.moves || [];
    } else {
      store.legalMoves.value = [];
    }

    renderGameView();
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

export async function loadGame(gameId: string): Promise<void> {
  const prev = store.currentGameId.value;
  if (prev && prev !== gameId) wsUnsubscribe(prev);

  batch(() => {
    store.currentGameId.value = gameId;
    store.currentGame.value = null;
    store.selectedSquare.value = null;
    store.legalTargets.value = [];
    store.lastMove.value = null;
    store.analysisResult.value = null;
    store.analysisRunning.value = false;
    store.analysisJobId.value = null;
  });

  navigateTo('game');
  wsSubscribe(gameId);
  await refreshCurrentGame();
}

export async function createNewGame(): Promise<void> {
  try {
    const res = await api.createGame();
    showToast(t('toast.new_game_created'), 'success');
    await refreshGameList();
    await loadGame(res.game_id);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

export async function deleteCurrentGame(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId) return;
  if (!confirm(t('confirm.delete'))) return;

  try {
    await api.deleteGame(gameId);
    batch(() => {
      store.currentGameId.value = null;
      store.currentGame.value = null;
    });
    showToast(t('toast.game_deleted'), 'info');
    navigateTo('dashboard');
    await refreshGameList();
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

// ============================================================================
// Move Execution
// ============================================================================

export async function attemptMove(from: string, to: string): Promise<void> {
  const moves = store.legalMoves.value || [];
  const promoMoves = moves.filter((m) => m.from === from && m.to === to && m.promotion);
  if (promoMoves.length > 0) {
    store.pendingPromotion.value = { from, to };
    showPromotionDialog();
    return;
  }
  await executeMove(from, to);
}

export async function executeMove(from: string, to: string, promotion?: string): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId) return;

  try {
    const move = { from, to, promotion };
    const res = await api.submitMove(gameId, move);

    batch(() => {
      store.selectedSquare.value = null;
      store.legalTargets.value = [];
      store.lastMove.value = { from, to };
      store.pendingPromotion.value = null;
    });

    hidePromotionDialog();
    showGameMessage(res.message, res.is_over ? 'warning' : 'success');
    await refreshCurrentGame();
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showGameMessage(msg, 'error');
    showToast(t('toast.invalid_move', { error: msg }), 'error');
  }
}

export function showPromotionDialog(): void {
  const dialog = document.getElementById('promotion-dialog');
  if (!dialog) return;

  const game = store.currentGame.value;
  const isWhite = game?.state.turn === 'white';
  const promoMap: Record<string, string> = isWhite
    ? { Q: '♕', R: '♖', B: '♗', N: '♘' }
    : { Q: '♛', R: '♜', B: '♝', N: '♞' };

  dialog.querySelectorAll<HTMLElement>('.promotion-btn').forEach((btn) => {
    const p = btn.dataset.promote!;
    const span = btn.querySelector('.promo-piece');
    if (span) span.textContent = promoMap[p] || '';
  });

  dialog.classList.remove('hidden');
}

export function hidePromotionDialog(): void {
  const dialog = document.getElementById('promotion-dialog');
  if (dialog) dialog.classList.add('hidden');
}

// ============================================================================
// Game Actions
// ============================================================================

export async function resign(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId || !confirm(t('confirm.resign'))) return;

  try {
    const res = await api.submitAction(gameId, { action: 'resign' });
    showGameMessage(res.message, 'warning');
    await refreshCurrentGame();
    await refreshGameList();
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

export async function offerDraw(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId) return;

  try {
    const res = await api.submitAction(gameId, { action: 'offer_draw' });
    showGameMessage(res.message, 'info');
    await refreshCurrentGame();
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

export async function claimDraw(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId) return;
  const reason = prompt(t('confirm.claim_draw_reason'));
  if (!reason) return;

  try {
    const res = await api.submitAction(gameId, {
      action: 'claim_draw',
      reason,
    });
    showGameMessage(res.message, 'info');
    await refreshCurrentGame();
    await refreshGameList();
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

export async function submitMoveFromInput(): Promise<void> {
  const fromEl = document.getElementById('input-from') as HTMLInputElement | null;
  const toEl = document.getElementById('input-to') as HTMLInputElement | null;
  const from = fromEl?.value?.trim().toLowerCase();
  const to = toEl?.value?.trim().toLowerCase();
  if (!from || !to) {
    showToast(t('toast.enter_from_to'), 'warning');
    return;
  }

  const gameId = store.currentGameId.value;
  if (!gameId) return;

  const moves = store.legalMoves.value || [];
  if (moves.some((m) => m.from === from && m.to === to && m.promotion)) {
    store.pendingPromotion.value = { from, to };
    showPromotionDialog();
    return;
  }

  try {
    const res = await api.submitMove(gameId, { from, to });
    store.lastMove.value = { from, to };
    showGameMessage(res.message, res.is_over ? 'warning' : 'success');
    if (fromEl) fromEl.value = '';
    if (toEl) toEl.value = '';
    await refreshCurrentGame();
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showGameMessage(msg, 'error');
    showToast(t('toast.invalid_move', { error: msg }), 'error');
  }
}

// ============================================================================
// FEN Import / Export
// ============================================================================

export async function copyFen(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId) return;
  try {
    const data = await api.exportFen(gameId);
    await navigator.clipboard.writeText(data.fen);
    showToast(t('toast.fen_copied'), 'success');
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

export async function copyPgn(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId) return;
  try {
    const pgn = await api.exportPgn(gameId);
    await navigator.clipboard.writeText(pgn);
    showToast(t('toast.pgn_copied'), 'success');
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

export async function importFen(): Promise<void> {
  const fen = prompt(t('toast.fen_import_prompt'));
  if (!fen) return;
  try {
    const res = await api.importFen(fen);
    showToast(t('toast.fen_imported'), 'success');
    await refreshGameList();
    await loadGame(res.game_id);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

// ============================================================================
// Navigation
// ============================================================================

export function navigateTo(view: 'dashboard' | 'game' | 'archive'): void {
  store.currentView.value = view;
}

// ============================================================================
// DOM Rendering
// ============================================================================

export function renderGameList(): void {
  const container = document.getElementById('games-list');
  const noGames = document.getElementById('no-games');
  const games = store.games.value;
  if (!container) return;

  container.querySelectorAll('.game-card').forEach((el) => el.remove());

  if (games.length === 0) {
    if (noGames) noGames.style.display = 'block';
    return;
  }
  if (noGames) noGames.style.display = 'none';

  for (const game of games) {
    const card = document.createElement('div');
    card.className = 'game-card';
    card.addEventListener('click', () => loadGame(game.game_id));

    const turnLabel = game.turn === 'white' ? t('game.turn_white') : t('game.turn_black');
    const badge = game.is_over
      ? `<span class="game-card-badge badge-over">${t('game.badge_over')}</span>`
      : `<span class="game-card-badge badge-active">${t('game.badge_active')}</span>`;

    let resultText = '';
    if (game.result) {
      const resultMap: Record<string, string> = {
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
  }
}

export function renderGameView(): void {
  const game = store.currentGame.value;
  const placeholder = document.getElementById('game-placeholder');
  const gameContainer = document.getElementById('game-container');

  if (!game) {
    if (placeholder) placeholder.style.display = 'block';
    if (gameContainer) gameContainer.style.display = 'none';
    return;
  }

  if (placeholder) placeholder.style.display = 'none';
  if (gameContainer) gameContainer.style.display = 'grid';

  // Info panel
  setText('info-game-id', game.game_id.substring(0, 12) + '…');
  setText('info-turn', game.state.turn === 'white' ? t('game.turn_white') : t('game.turn_black'));
  setText('info-move-num', game.state.fullmove_number);
  setText('info-status', game.is_over ? t('game.status_over') : t('game.status_active'));
  setText('info-check', game.is_check ? t('game.check_yes') : t('game.check_no'));
  setText('info-legal-moves', game.legal_move_count);

  // Castling rights
  const c = game.state.castling;
  setText('castle-wk', c.white.kingside ? '✓' : '✗');
  setText('castle-wq', c.white.queenside ? '✓' : '✗');
  setText('castle-bk', c.black.kingside ? '✓' : '✗');
  setText('castle-bq', c.black.queenside ? '✓' : '✗');

  // Player bars
  document
    .querySelector('.white-bar')
    ?.classList.toggle('active-turn', game.state.turn === 'white');
  document
    .querySelector('.black-bar')
    ?.classList.toggle('active-turn', game.state.turn === 'black');

  const ws = document.getElementById('white-status');
  const bs = document.getElementById('black-status');
  if (ws) ws.textContent = game.state.turn === 'white' ? t('game.your_turn') : '';
  if (bs) bs.textContent = game.state.turn === 'black' ? t('game.your_turn') : '';

  // Disable actions if game is over
  document.querySelectorAll<HTMLButtonElement>('#game-actions .btn').forEach((btn) => {
    if (btn.id !== 'btn-delete-game') btn.disabled = game.is_over;
  });

  renderMoveHistory(game.move_history);
  renderCurrentBoard();
}

function renderMoveHistory(history: MoveHistoryEntry[]): void {
  const container = document.getElementById('move-history');
  if (!container) return;

  if (!history || history.length === 0) {
    container.innerHTML = `<div class="empty-hint">${t('game.no_moves')}</div>`;
    return;
  }

  const grouped: Record<number, { white?: string; black?: string }> = {};
  for (const m of history) {
    if (!grouped[m.move_number]) grouped[m.move_number] = {};
    grouped[m.move_number][m.side] = m.notation;
  }

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

export function renderStorageStats(
  stats: {
    active_count: number;
    archived_count: number;
    active_bytes: number;
    archive_bytes: number;
  } | null,
): void {
  if (!stats) return;
  setText('stat-active', stats.active_count);
  setText('stat-archived', stats.archived_count);
  setText('stat-active-bytes', formatBytes(stats.active_bytes));
  setText('stat-archive-bytes', formatBytes(stats.archive_bytes));
}

export async function refreshStorageStats(): Promise<void> {
  try {
    const stats = await api.getStorageStats();
    store.storageStats.value = stats;
    renderStorageStats(stats);
  } catch {
    /* ignore */
  }
}
