<script lang="ts">
  import { activeAnalysis, activeGame, analysisDepth, analysisJobs } from '../stores.js';
  import {
    cancelAnalysis,
    refreshAnalysisJobs,
    setAnalysisDepth,
    submitAnalysisForGame,
    viewAnalysisJob,
  } from '../workspace.js';
  import type {
    AnalysisJob,
    AnalysisMoveAnnotation,
    AnalysisMoveQuality,
    AnalysisStatus,
  } from '../shared-types.js';

  function isInProgress(
    status: AnalysisStatus
  ): status is { InProgress: { moves_analyzed: number; total_moves: number } } {
    return typeof status === 'object' && status !== null && 'InProgress' in status;
  }

  function isFailed(
    status: AnalysisStatus
  ): status is { Failed: { error: string } } {
    return typeof status === 'object' && status !== null && 'Failed' in status;
  }

  function statusLabel(job: AnalysisJob): string {
    if (isInProgress(job.status)) {
      return `${job.status.InProgress.moves_analyzed}/${job.status.InProgress.total_moves}`;
    }

    if (isFailed(job.status)) {
      return `Failed: ${job.status.Failed.error}`;
    }

    return String(job.status);
  }

  function qualityColor(quality: AnalysisMoveQuality): string {
    switch (quality) {
      case 'Best':
      case 'Excellent':
        return 'var(--success)';
      case 'Good':
      case 'Book':
        return 'var(--primary)';
      case 'Inaccuracy':
        return 'var(--warning)';
      case 'Mistake':
      case 'Blunder':
        return 'var(--danger)';
    }
  }

  function isTerminal(status: AnalysisStatus): boolean {
    return status === 'Completed' || status === 'Cancelled' || isFailed(status);
  }

  function annotationMove(annotation: AnalysisMoveAnnotation): string {
    return `${annotation.played_move.from}${annotation.played_move.to}${annotation.played_move.promotion ?? ''}`;
  }
</script>

<div class="view-grid">
  <div class="card hero-card">
    <div class="card-head">
      <div>
        <h2>Analysis Workspace</h2>
        <p class="dim">
          Queue local engine analysis jobs, track progress, and inspect move-by-move verdicts.
        </p>
      </div>
    </div>

    <div class="btn-row">
      <label class="field-inline">
        <span>Depth</span>
        <input
          type="number"
          min="30"
          max="99"
          value={$analysisDepth}
          on:input={(event) =>
            setAnalysisDepth(Number.parseInt((event.currentTarget as HTMLInputElement).value, 10))}
        />
      </label>

      <button
        class="btn btn-primary btn-sm"
        disabled={!$activeGame}
        on:click={() => submitAnalysisForGame()}
      >
        ▶ Analyze current game
      </button>

      <button class="btn btn-ghost btn-sm" on:click={() => refreshAnalysisJobs()}>
        ↻ Refresh jobs
      </button>
    </div>

    {#if $activeGame}
      <p class="dim">
        Current target: <span class="mono">{$activeGame.game_id}</span>
      </p>
    {:else}
      <p class="dim">Open a game first to launch a new analysis run.</p>
    {/if}
  </div>

  {#if $activeAnalysis}
    <div class="card">
      <div class="card-head">
        <div>
          <h3>Focused analysis</h3>
          <p class="dim">
            Job <span class="mono">{$activeAnalysis.id}</span> · {statusLabel($activeAnalysis)}
          </p>
        </div>
        {#if !isTerminal($activeAnalysis.status)}
          <span class="badge badge-active">Running</span>
        {:else if $activeAnalysis.status === 'Completed'}
          <span class="badge badge-ok">Completed</span>
        {:else}
          <span class="badge badge-danger">Finished</span>
        {/if}
      </div>

      {#if isInProgress($activeAnalysis.status)}
        <progress
          max={$activeAnalysis.status.InProgress.total_moves}
          value={$activeAnalysis.status.InProgress.moves_analyzed}
          style="width: 100%; margin-bottom: 1rem"
        ></progress>
      {/if}

      {#if $activeAnalysis.result}
        <div class="stat-grid">
          <div class="stat">
            <span class="stat-label">Depth</span>
            <strong>{$activeAnalysis.result.depth}</strong>
          </div>
          <div class="stat">
            <span class="stat-label">Average CP loss</span>
            <strong>{$activeAnalysis.result.summary.average_centipawn_loss}</strong>
          </div>
          <div class="stat">
            <span class="stat-label">White accuracy</span>
            <strong>{$activeAnalysis.result.summary.white_accuracy.toFixed(1)}%</strong>
          </div>
          <div class="stat">
            <span class="stat-label">Black accuracy</span>
            <strong>{$activeAnalysis.result.summary.black_accuracy.toFixed(1)}%</strong>
          </div>
        </div>

        <div class="table-wrap" style="margin-top: 1rem">
          <table class="data-table compact">
            <thead>
              <tr>
                <th>#</th>
                <th>Move</th>
                <th>Best</th>
                <th>CP loss</th>
                <th>Quality</th>
                <th>PV</th>
              </tr>
            </thead>
            <tbody>
              {#each $activeAnalysis.result.annotations as annotation (annotation.move_number + annotation.side)}
                <tr>
                  <td>{annotation.move_number}{annotation.side === 'white' ? '.' : '…'}</td>
                  <td class="mono">{annotationMove(annotation)}</td>
                  <td class="mono">
                    {annotation.best_move.from}{annotation.best_move.to}{annotation.best_move.promotion ??
                      ''}
                  </td>
                  <td>{annotation.centipawn_loss}</td>
                  <td>
                    <span class="quality-dot" style={`color: ${qualityColor(annotation.quality)}`}>
                      {annotation.quality}
                    </span>
                  </td>
                  <td class="mono dim">{annotation.principal_variation.slice(0, 3).join(' ')}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {/if}

  <div class="card">
    <div class="card-head">
      <div>
        <h3>Analysis jobs</h3>
        <p class="dim">
          {$analysisJobs.length} queued or completed job{$analysisJobs.length === 1 ? '' : 's'}.
        </p>
      </div>
    </div>

    {#if $analysisJobs.length === 0}
      <div class="empty-card">
        <p class="empty-text">No analysis jobs yet.</p>
      </div>
    {:else}
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>Job</th>
              <th>Game</th>
              <th>Status</th>
              <th>Created</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each $analysisJobs as job (job.id)}
              <tr>
                <td class="mono">{job.id.slice(0, 8)}…</td>
                <td class="mono">{job.game_id ? `${job.game_id.slice(0, 8)}…` : '—'}</td>
                <td>{statusLabel(job)}</td>
                <td>{new Date(job.created_at).toLocaleString()}</td>
                <td class="btn-row">
                  <button class="btn btn-sm" on:click={() => viewAnalysisJob(job.id)}>
                    View
                  </button>
                  {#if !isTerminal(job.status)}
                    <button class="btn btn-sm btn-danger" on:click={() => cancelAnalysis(job.id)}>
                      Cancel
                    </button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>
