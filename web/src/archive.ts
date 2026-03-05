// ============================================================================
// CheckAI Web UI — Archive Module
// ============================================================================

import * as api from './api';
import { renderBoard } from './board';
import { getLocale, t } from './i18n';
import { store } from './store';
import { formatBytes, setText, showToast } from './ui';

export async function refreshArchiveList(): Promise<void> {
  try {
    const data = await api.listArchived();
    store.archivedGames.value = data.games || [];
    renderArchiveList();

    if (data.storage) {
      store.storageStats.value = data.storage;
      const { renderStorageStats } = await import('./game');
      renderStorageStats(data.storage);
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.load_archive_failed', { error: msg }), 'error');
  }
}

function renderArchiveList(): void {
  const container = document.getElementById('archive-list');
  const noArchive = document.getElementById('no-archive');
  const games = store.archivedGames.value;
  if (!container) return;

  container.querySelectorAll('.game-card').forEach((el) => el.remove());

  if (games.length === 0) {
    if (noArchive) noArchive.style.display = 'block';
    return;
  }
  if (noArchive) noArchive.style.display = 'none';

  for (const game of games) {
    const card = document.createElement('div');
    card.className = 'game-card archive-card';
    card.addEventListener('click', () => openReplay(game.game_id, game.move_count));

    const resultMap: Record<string, { text: string; cls: string }> = {
      WhiteWins: { text: t('result.white_wins'), cls: 'result-white' },
      BlackWins: { text: t('result.black_wins'), cls: 'result-black' },
      Draw: { text: t('result.draw'), cls: 'result-draw' },
    };
    const result = (game.result && resultMap[game.result]) || {
      text: game.result || '?',
      cls: '',
    };
    const reason = game.end_reason ? t(`reason.${game.end_reason}`) || game.end_reason : '';
    const date = game.start_timestamp
      ? new Date(game.start_timestamp * 1000).toLocaleString(
          getLocale() === 'zh-CN' ? 'zh-CN' : getLocale(),
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
  }
}

async function openReplay(gameId: string, totalMoves: number): Promise<void> {
  const replaySection = document.getElementById('archive-replay');
  if (!replaySection) return;

  store.replayTotalMoves.value = totalMoves || 0;
  store.replayMoveNum.value = totalMoves || 0;

  const slider = document.getElementById('replay-slider') as HTMLInputElement | null;
  if (slider) {
    slider.max = String(totalMoves || 0);
    slider.value = String(totalMoves || 0);
  }

  setText('replay-total-moves', totalMoves || 0);
  setText('replay-title', t('archive.replay_title', { id: gameId.substring(0, 8) + '…' }));

  replaySection.classList.remove('hidden');
  replaySection.dataset.gameId = gameId;

  await loadReplayPosition(gameId, totalMoves || 0);
}

async function loadReplayPosition(gameId: string, moveNum: number): Promise<void> {
  try {
    const data = await api.replayArchived(gameId, moveNum);
    store.replayData.value = data;
    store.replayMoveNum.value = data.at_move;
    setText('replay-move-num', data.at_move);
    renderBoard('replay-board', data.state.board, { interactive: false });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.replay_failed', { error: msg }), 'error');
  }
}

/** Bind replay control events. */
export function bindArchiveEvents(): void {
  const refreshBtn = document.getElementById('btn-refresh-archive');
  refreshBtn?.addEventListener('click', () => refreshArchiveList());

  const closeBtn = document.getElementById('btn-close-replay');
  closeBtn?.addEventListener('click', () => {
    const section = document.getElementById('archive-replay');
    if (section) section.classList.add('hidden');
  });

  const slider = document.getElementById('replay-slider') as HTMLInputElement | null;
  slider?.addEventListener('input', async () => {
    const section = document.getElementById('archive-replay');
    const gameId = section?.dataset.gameId;
    if (gameId) await loadReplayPosition(gameId, parseInt(slider.value));
  });

  document.getElementById('replay-start')?.addEventListener('click', async () => {
    const section = document.getElementById('archive-replay');
    const gameId = section?.dataset.gameId;
    if (!gameId) return;
    if (slider) slider.value = '0';
    await loadReplayPosition(gameId, 0);
  });

  document.getElementById('replay-end')?.addEventListener('click', async () => {
    const section = document.getElementById('archive-replay');
    const gameId = section?.dataset.gameId;
    if (!gameId) return;
    const total = store.replayTotalMoves.value;
    if (slider) slider.value = String(total);
    await loadReplayPosition(gameId, total);
  });

  document.getElementById('replay-prev')?.addEventListener('click', async () => {
    const section = document.getElementById('archive-replay');
    const gameId = section?.dataset.gameId;
    if (!gameId) return;
    const next = Math.max(0, store.replayMoveNum.value - 1);
    if (slider) slider.value = String(next);
    await loadReplayPosition(gameId, next);
  });

  document.getElementById('replay-next')?.addEventListener('click', async () => {
    const section = document.getElementById('archive-replay');
    const gameId = section?.dataset.gameId;
    if (!gameId) return;
    const total = store.replayTotalMoves.value;
    const next = Math.min(total, store.replayMoveNum.value + 1);
    if (slider) slider.value = String(next);
    await loadReplayPosition(gameId, next);
  });
}
