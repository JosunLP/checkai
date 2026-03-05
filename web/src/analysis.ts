// ============================================================================
// CheckAI Web UI — Analysis Panel
// ============================================================================

import * as api from './api';
import { t } from './i18n';
import { store } from './store';
import { showToast } from './ui';

let pollTimer: ReturnType<typeof setInterval> | null = null;

/** Start engine analysis for the current game. */
export async function startAnalysis(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId || store.analysisRunning.value) return;

  try {
    const res = await api.startAnalysis(gameId, { depth: 20 });
    store.analysisJobId.value = res.job_id;
    store.analysisRunning.value = true;
    renderAnalysis();
    startPolling(gameId, res.job_id);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

/** Stop running analysis. */
export async function stopAnalysis(): Promise<void> {
  const gameId = store.currentGameId.value;
  const jobId = store.analysisJobId.value;
  if (!gameId || !jobId) return;

  stopPolling();
  try {
    await api.cancelAnalysis(gameId, jobId);
  } catch {
    /* ignore */
  }
  store.analysisRunning.value = false;
  renderAnalysis();
}

function startPolling(gameId: string, jobId: string): void {
  stopPolling();
  pollTimer = setInterval(async () => {
    try {
      const result = await api.getAnalysis(gameId, jobId);
      store.analysisResult.value = {
        depth: result.depth,
        score: result.score,
        bestMove: result.best_move,
        pv: result.pv,
        nodes: result.nodes,
        nps: result.nps,
        timeMs: result.time_ms,
      };
      renderAnalysis();

      if (result.status === 'completed' || result.status === 'cancelled') {
        store.analysisRunning.value = false;
        stopPolling();
        renderAnalysis();
      }
    } catch {
      stopPolling();
      store.analysisRunning.value = false;
      renderAnalysis();
    }
  }, 500);
}

function stopPolling(): void {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

/** Format centipawn score as human-readable evaluation. */
function formatScore(cp: number): string {
  if (Math.abs(cp) > 29000) {
    const mateIn = Math.ceil((30000 - Math.abs(cp)) / 2);
    return cp > 0 ? `M${mateIn}` : `-M${mateIn}`;
  }
  const sign = cp >= 0 ? '+' : '';
  return `${sign}${(cp / 100).toFixed(2)}`;
}

/** Format large numbers with separators. */
function formatNum(n: number): string {
  return n.toLocaleString('en-US');
}

/** Render the analysis panel content. */
export function renderAnalysis(): void {
  const panel = document.getElementById('analysis-content');
  if (!panel) return;

  const running = store.analysisRunning.value;
  const result = store.analysisResult.value;

  // Toggle button states
  const startBtn = document.getElementById('btn-analysis-start') as HTMLButtonElement | null;
  const stopBtn = document.getElementById('btn-analysis-stop') as HTMLButtonElement | null;
  if (startBtn) startBtn.disabled = running;
  if (stopBtn) stopBtn.disabled = !running;

  if (!result) {
    panel.innerHTML = `<p class="analysis-idle">${t('analysis.idle')}</p>`;
    return;
  }

  panel.innerHTML = `
    <div class="analysis-grid">
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.score')}</span>
        <span class="analysis-value analysis-score">${formatScore(result.score)}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.depth')}</span>
        <span class="analysis-value">${result.depth}${running ? '…' : ''}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.best_move')}</span>
        <span class="analysis-value mono">${result.bestMove || '—'}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.nodes')}</span>
        <span class="analysis-value">${formatNum(result.nodes)}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.nps')}</span>
        <span class="analysis-value">${formatNum(result.nps)}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.time')}</span>
        <span class="analysis-value">${(result.timeMs / 1000).toFixed(1)}s</span>
      </div>
    </div>
    ${
      result.pv.length > 0
        ? `
      <div class="analysis-pv">
        <span class="analysis-label">${t('analysis.pv')}</span>
        <span class="analysis-pv-line mono">${result.pv.join(' ')}</span>
      </div>
    `
        : ''
    }
  `;
}

/** Bind analysis panel events. */
export function bindAnalysisEvents(): void {
  document.getElementById('btn-analysis-start')?.addEventListener('click', startAnalysis);
  document.getElementById('btn-analysis-stop')?.addEventListener('click', stopAnalysis);
}
