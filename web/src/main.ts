// ============================================================================
// CheckAI Web UI — Main Entry Point
// ============================================================================

import './styles.css';

import { effect } from '@bquery/bquery/reactive';
import { bindAnalysisEvents } from './analysis';
import { bindArchiveEvents, refreshArchiveList } from './archive';
import { renderCurrentBoard } from './board';
import {
  claimDraw,
  copyFen,
  copyPgn,
  createNewGame,
  deleteCurrentGame,
  importFen,
  loadGame,
  offerDraw,
  refreshGameList,
  refreshStorageStats,
  renderGameList,
  resign,
  submitMoveFromInput,
} from './game';
import { getLocale, initI18n, setLocale, translateDom } from './i18n';
import { store } from './store';
import { showToast } from './ui';
import { connectWebSocket } from './ws';

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------
function navigateView(view: 'dashboard' | 'game' | 'archive'): void {
  store.currentView.value = view;
}

function bindNavigation(): void {
  document.querySelectorAll<HTMLElement>('[data-nav]').forEach((el) => {
    el.addEventListener('click', () => {
      const target = el.dataset.nav as 'dashboard' | 'game' | 'archive';
      navigateView(target);
    });
  });
}

// View switching effect
function setupViewEffect(): void {
  effect(() => {
    const view = store.currentView.value;
    document.querySelectorAll<HTMLElement>('.view').forEach((el) => {
      el.classList.toggle('hidden', el.id !== `view-${view}`);
    });
    document.querySelectorAll<HTMLElement>('[data-nav]').forEach((el) => {
      el.classList.toggle('active', el.dataset.nav === view);
    });

    if (view === 'dashboard') {
      refreshGameList().then(renderGameList);
      refreshStorageStats();
    } else if (view === 'archive') {
      refreshArchiveList();
    }
  });
}

// ---------------------------------------------------------------------------
// WebSocket status indicator
// ---------------------------------------------------------------------------
function setupWsIndicator(): void {
  effect(() => {
    const connected = store.wsConnected.value;
    const dot = document.getElementById('ws-indicator');
    if (dot) {
      dot.classList.toggle('connected', connected);
      dot.classList.toggle('disconnected', !connected);
      dot.title = connected ? 'Connected' : 'Disconnected';
    }
  });
}

// ---------------------------------------------------------------------------
// Board reactivity
// ---------------------------------------------------------------------------
function setupBoardEffect(): void {
  effect(() => {
    // Re-render whenever game, flipping, selection, or legal targets change
    void store.currentGame.value;
    void store.boardFlipped.value;
    void store.selectedSquare.value;
    void store.legalTargets.value;
    void store.lastMove.value;
    void store.isCheck.value;
    renderCurrentBoard();
  });
}

// ---------------------------------------------------------------------------
// Game action bindings
// ---------------------------------------------------------------------------
function bindGameActions(): void {
  document
    .getElementById('btn-new-game')
    ?.addEventListener('click', async () => {
      await createNewGame();
    });

  document.getElementById('btn-resign')?.addEventListener('click', resign);
  document.getElementById('btn-draw')?.addEventListener('click', offerDraw);
  document
    .getElementById('btn-claim-draw')
    ?.addEventListener('click', claimDraw);
  document
    .getElementById('btn-delete-game')
    ?.addEventListener('click', deleteCurrentGame);

  document.getElementById('btn-flip-board')?.addEventListener('click', () => {
    store.boardFlipped.value = !store.boardFlipped.value;
  });

  document.getElementById('btn-copy-fen')?.addEventListener('click', copyFen);
  document.getElementById('btn-copy-pgn')?.addEventListener('click', copyPgn);
  document
    .getElementById('btn-import-fen')
    ?.addEventListener('click', importFen);

  // Move input form
  document.getElementById('move-form')?.addEventListener('submit', (e) => {
    e.preventDefault();
    submitMoveFromInput();
  });

  // Game list delegation
  document.getElementById('game-list')?.addEventListener('click', (e) => {
    const target = (e.target as HTMLElement).closest<HTMLElement>(
      '[data-game-id]'
    );
    if (target) {
      loadGame(target.dataset.gameId!);
      navigateView('game');
    }
  });
}

// ---------------------------------------------------------------------------
// Language selector
// ---------------------------------------------------------------------------
function bindLanguageSelector(): void {
  const select = document.getElementById(
    'lang-select'
  ) as HTMLSelectElement | null;
  if (!select) return;
  select.value = getLocale();
  select.addEventListener('change', () => {
    setLocale(select.value);
    translateDom();
  });
}

// ---------------------------------------------------------------------------
// Promotion dialog
// ---------------------------------------------------------------------------
function bindPromotionDialog(): void {
  document.querySelectorAll<HTMLElement>('[data-promote]').forEach((el) => {
    el.addEventListener('click', () => {
      const piece = el.dataset.promote;
      if (piece && store.pendingPromotion.value) {
        const { from, to } = store.pendingPromotion.value;
        store.pendingPromotion.value = null;
        document.getElementById('promotion-dialog')?.classList.add('hidden');
        // Dynamically import to avoid circular dependency
        import('./game').then(({ executeMove }) => {
          executeMove(from, to, piece);
        });
      }
    });
  });
}

// ---------------------------------------------------------------------------
// Initialize
// ---------------------------------------------------------------------------
async function init(): Promise<void> {
  initI18n();
  translateDom();
  bindNavigation();
  bindGameActions();
  bindLanguageSelector();
  bindPromotionDialog();
  bindArchiveEvents();
  bindAnalysisEvents();

  setupViewEffect();
  setupWsIndicator();
  setupBoardEffect();

  connectWebSocket();

  // Initial data load
  await Promise.all([refreshGameList(), refreshStorageStats()]);
  renderGameList();
}

init().catch((err) => {
  console.error('CheckAI init failed:', err);
  showToast('Initialization failed', 'error');
});
