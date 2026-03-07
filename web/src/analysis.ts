// ============================================================================
// CheckAI Web UI — Analysis Panel
// ============================================================================

import { batch } from '@bquery/bquery/reactive';
import * as api from './api';
import { t } from './i18n';
import { store } from './store';
import type { AnalysisJob, AnalysisPanelState, AnalysisStatus } from './types';
import { showToast } from './ui';

let pollTimer: ReturnType<typeof setInterval> | null = null;

/** Start engine analysis for the current game. */
export async function startAnalysis(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId || store.analysisRunning.value) return;

  try {
    const res = await api.startAnalysis(gameId, { depth: 30 });
    batch(() => {
      store.analysisJobId.value = res.job_id;
      store.analysisRunning.value = true;
      store.analysisResult.value = {
        statusText: t('analysis.queued'),
        progressText: null,
        errorMessage: null,
        depth: null,
        totalMoves: null,
        averageCpLoss: null,
        whiteAccuracy: null,
        blackAccuracy: null,
        whiteAverageCpLoss: null,
        blackAverageCpLoss: null,
        bookAvailable: null,
        tablebaseAvailable: null,
        counts: null,
      };
    });
    renderAnalysis();
    startPolling(res.job_id);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast(t('toast.error', { error: msg }), 'error');
  }
}

/** Stop running analysis. */
export async function stopAnalysis(): Promise<void> {
  const jobId = store.analysisJobId.value;
  if (!jobId) return;

  stopPolling();
  try {
    await api.cancelAnalysis(jobId);
  } catch {
    /* ignore */
  }
  batch(() => {
    store.analysisRunning.value = false;
    if (store.analysisResult.value) {
      store.analysisResult.value = {
        ...store.analysisResult.value,
        statusText: t('analysis.cancelled'),
      };
    }
  });
  renderAnalysis();
}

/** Reset analysis UI state and stop background polling. */
export function resetAnalysisState(): void {
  stopPolling();
  batch(() => {
    store.analysisJobId.value = null;
    store.analysisResult.value = null;
    store.analysisRunning.value = false;
  });
}

function startPolling(jobId: string): void {
  stopPolling();
  pollTimer = setInterval(async () => {
    try {
      const job = await api.getAnalysis(jobId);
      store.analysisResult.value = toPanelState(job);
      renderAnalysis();

      if (isTerminalStatus(job.status)) {
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

function isTerminalStatus(status: AnalysisStatus): boolean {
  return status === 'Completed' || status === 'Cancelled' || 'Failed' in Object(status);
}

function getStatusLabel(status: AnalysisStatus): string {
  if (status === 'Queued') return t('analysis.queued');
  if (status === 'Completed') return t('analysis.completed');
  if (status === 'Cancelled') return t('analysis.cancelled');
  if ('Failed' in status) return t('analysis.failed');
  return t('analysis.running');
}

function getProgressLabel(status: AnalysisStatus): string | null {
  if ('InProgress' in Object(status)) {
    const { moves_analyzed, total_moves } = (
      status as Extract<
        AnalysisStatus,
        { InProgress: { moves_analyzed: number; total_moves: number } }
      >
    ).InProgress;
    return `${moves_analyzed}/${total_moves}`;
  }
  return null;
}

function getFailureMessage(status: AnalysisStatus): string | null {
  if ('Failed' in Object(status)) {
    return (status as Extract<AnalysisStatus, { Failed: { error: string } }>).Failed.error;
  }
  return null;
}

function toPanelState(job: AnalysisJob): AnalysisPanelState {
  const summary = job.result?.summary;

  return {
    statusText: getStatusLabel(job.status),
    progressText: getProgressLabel(job.status),
    errorMessage: getFailureMessage(job.status),
    depth: job.result?.depth ?? null,
    totalMoves: summary?.total_moves ?? null,
    averageCpLoss: summary?.average_centipawn_loss ?? null,
    whiteAccuracy: summary?.white_accuracy ?? null,
    blackAccuracy: summary?.black_accuracy ?? null,
    whiteAverageCpLoss: summary?.white_avg_cp_loss ?? null,
    blackAverageCpLoss: summary?.black_avg_cp_loss ?? null,
    bookAvailable: job.result?.book_available ?? null,
    tablebaseAvailable: job.result?.tablebase_available ?? null,
    counts: summary
      ? {
          best: summary.best_moves,
          excellent: summary.excellent_moves,
          good: summary.good_moves,
          inaccuracies: summary.inaccuracies,
          mistakes: summary.mistakes,
          blunders: summary.blunders,
          book: summary.book_moves,
        }
      : null,
  };
}

function formatDecimal(value: number | null): string {
  if (value === null) return '—';
  return value.toFixed(1);
}

function formatPercent(value: number | null): string {
  if (value === null) return '—';
  return `${value.toFixed(1)}%`;
}

function formatBool(value: boolean | null): string {
  if (value === null) return '—';
  return value ? t('analysis.available') : t('analysis.unavailable');
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
        <span class="analysis-label">${t('analysis.status')}</span>
        <span class="analysis-value">${result.statusText}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.progress')}</span>
        <span class="analysis-value">${result.progressText ?? '—'}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.depth')}</span>
        <span class="analysis-value">${result.depth ?? '—'}${running ? '…' : ''}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.total_moves')}</span>
        <span class="analysis-value">${result.totalMoves ?? '—'}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.avg_cp_loss')}</span>
        <span class="analysis-value">${formatDecimal(result.averageCpLoss)}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.white_accuracy')}</span>
        <span class="analysis-value">${formatPercent(result.whiteAccuracy)}</span>
      </div>
    </div>
    ${
      result.errorMessage
        ? `
      <div class="analysis-pv">
        <span class="analysis-label">${t('analysis.failed')}</span>
        <span class="analysis-pv-line mono">${result.errorMessage}</span>
      </div>
    `
        : ''
    }
    <div class="analysis-grid">
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.black_accuracy')}</span>
        <span class="analysis-value">${formatPercent(result.blackAccuracy)}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.white_avg_cp_loss')}</span>
        <span class="analysis-value">${formatDecimal(result.whiteAverageCpLoss)}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.black_avg_cp_loss')}</span>
        <span class="analysis-value">${formatDecimal(result.blackAverageCpLoss)}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.book_available')}</span>
        <span class="analysis-value">${formatBool(result.bookAvailable)}</span>
      </div>
      <div class="analysis-item">
        <span class="analysis-label">${t('analysis.tablebase_available')}</span>
        <span class="analysis-value">${formatBool(result.tablebaseAvailable)}</span>
      </div>
    </div>
    ${
      result.counts
        ? `
      <div class="analysis-pv">
        <span class="analysis-label">${t('analysis.summary')}</span>
        <span class="analysis-pv-line mono">Best ${result.counts.best} · Excellent ${result.counts.excellent} · Good ${result.counts.good} · Inaccuracies ${result.counts.inaccuracies} · Mistakes ${result.counts.mistakes} · Blunders ${result.counts.blunders} · Book ${result.counts.book}</span>
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
