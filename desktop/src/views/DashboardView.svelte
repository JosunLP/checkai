<script lang="ts">
  import {
    analysisJobs,
    backendStatus,
    desktopState,
    gamesList,
    storageStats,
    updateStatus,
  } from '../stores.js';
  import {
    createNewGame,
    importFenFromFile,
    navigateTo,
    openGame,
    startBackendProcess,
    stopBackendProcess,
  } from '../workspace.js';
</script>

<div class="view-grid">
  <div class="card hero-card">
    <div class="card-head">
      <h2>Welcome to CheckAI Desktop</h2>
    </div>
    <p class="dim">
      A powerful chess analysis engine with comprehensive game tracking, analysis,
      and replay capabilities.
    </p>

    <div class="quick-strip">
      <button class="qbtn" on:click={createNewGame}>
        <strong>♟ New Game</strong>
        <span>Start a new local game and jump straight into the board view</span>
      </button>
      <button class="qbtn" on:click={importFenFromFile}>
        <strong>📂 Import FEN</strong>
        <span>Use the native file dialog to load a saved position from disk</span>
      </button>
      <button class="qbtn" on:click={() => navigateTo('games')}>
        <strong>🗂 Manage Games</strong>
        <span>Open active games, delete stale sessions, and resume work quickly</span>
      </button>
      <button class="qbtn" on:click={() => navigateTo('analysis')}>
        <strong>📊 Analysis Workspace</strong>
        <span>Review running jobs, finished annotations, and engine verdicts</span>
      </button>
      {#if $backendStatus.running}
        <button class="qbtn" on:click={stopBackendProcess}>
          <strong>■ Stop Backend</strong>
          <span>Shut down the local engine process safely</span>
        </button>
      {:else}
        <button class="qbtn" on:click={startBackendProcess}>
          <strong>▶ Start Backend</strong>
          <span>Launch the local CheckAI backend and sync the workspace</span>
        </button>
      {/if}
      {#if $gamesList[0]}
        <button class="qbtn" on:click={() => openGame($gamesList[0].game_id)}>
          <strong>♞ Resume Latest</strong>
          <span>Continue with {$gamesList[0].game_id.slice(0, 8)}…</span>
        </button>
      {/if}
    </div>
  </div>

  <div class="card">
    <div class="card-head">
      <h3>Active Games</h3>
    </div>
    <p class="dim">{$gamesList.length} game(s)</p>
    {#if $gamesList[0]}
      <p class="mono">Latest: {$gamesList[0].game_id.slice(0, 8)}…</p>
    {/if}
  </div>

  <div class="card">
    <div class="card-head">
      <h3>Analysis Jobs</h3>
    </div>
    <p class="dim">{$analysisJobs.length} job(s)</p>
    <p class="dim">
      {#if $analysisJobs.some((job) => typeof job.status === 'object' && 'InProgress' in job.status)}
        Engine is currently evaluating at least one job.
      {:else}
        Queue is idle right now.
      {/if}
    </p>
  </div>

  {#if $storageStats}
    <div class="card">
      <div class="card-head">
        <h3>Storage</h3>
      </div>
      <div class="stat-grid">
        <div class="stat">
          <span class="stat-label">Active</span>
          <strong>{$storageStats.active_count}</strong>
        </div>
        <div class="stat">
          <span class="stat-label">Archived</span>
          <strong>{$storageStats.archived_count}</strong>
        </div>
      </div>
    </div>
  {/if}

  <div class="card">
    <div class="card-head">
      <h3>Workspace</h3>
    </div>
    <div class="stat-grid">
      <div class="stat">
        <span class="stat-label">Backend URL</span>
        <strong class="mono">{$desktopState.backendUrl}</strong>
      </div>
      <div class="stat">
        <span class="stat-label">Status</span>
        <strong>{$backendStatus.running ? 'Running' : 'Stopped'}</strong>
      </div>
      <div class="stat">
        <span class="stat-label">Update</span>
        <strong>{$updateStatus.state}</strong>
      </div>
      <div class="stat">
        <span class="stat-label">Working Dir</span>
        <strong class="mono">
          {$desktopState.backendWorkingDirectory || 'Project default'}
        </strong>
      </div>
    </div>
  </div>
</div>
