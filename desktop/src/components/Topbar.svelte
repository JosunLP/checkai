<script lang="ts">
  import {
    activeGame,
    analysisJobs,
    backendStatus,
    currentView,
    updateStatus,
  } from '../stores.js';
  import {
    checkForDesktopUpdates,
    copyActivePgn,
    createNewGame,
    importFenFromFile,
    openBackendInBrowser,
    refreshCurrentView,
    startBackendProcess,
    stopBackendProcess,
    submitAnalysisForGame,
  } from '../workspace.js';

  const viewTitles: Record<string, string> = {
    dashboard: 'Dashboard',
    games: 'Games',
    board: 'Board',
    archive: 'Archive',
    analysis: 'Analysis',
    engine: 'Engine',
    logs: 'Logs',
    settings: 'Settings',
  };

  $: title = viewTitles[$currentView] || 'CheckAI Desktop';
  $: subtitle = $activeGame
    ? `${$activeGame.game_id.slice(0, 8)}… · ${$activeGame.state.turn} to move`
    : $backendStatus.running
      ? 'Local backend online'
      : 'Desktop shell ready';
  $: runningAnalyses = $analysisJobs.filter(
    (job) => typeof job.status === 'object' && 'InProgress' in job.status
  ).length;
</script>

<div class="topbar">
  <div class="topbar-copy">
    <span class="topbar-kicker">CheckAI Desktop</span>
    <h1>{title}</h1>
    <div class="topbar-meta">
      <span>{subtitle}</span>
      <span>•</span>
      <span>
        {$updateStatus.state === 'available'
          ? `Update ${$updateStatus.availableVersion ?? ''} available`
          : $updateStatus.message ?? 'No pending desktop updates'}
      </span>
      {#if runningAnalyses > 0}
        <span>•</span>
        <span>{runningAnalyses} analysis job(s) running</span>
      {/if}
    </div>
  </div>

  <div class="topbar-actions">
    <button class="btn btn-ghost btn-sm" on:click={refreshCurrentView}>↻ Refresh</button>

    {#if $currentView === 'dashboard' || $currentView === 'games'}
      <button class="btn btn-primary btn-sm" on:click={createNewGame}>♟ New game</button>
      <button class="btn btn-ghost btn-sm" on:click={importFenFromFile}>📂 Import</button>
    {:else if $currentView === 'board' && $activeGame}
      <button class="btn btn-primary btn-sm" on:click={() => submitAnalysisForGame()}>
        📊 Analyze
      </button>
      <button class="btn btn-ghost btn-sm" on:click={copyActivePgn}>📋 Copy PGN</button>
      <button class="btn btn-ghost btn-sm" on:click={openBackendInBrowser}>
        🌐 Live UI
      </button>
    {:else if $currentView === 'analysis'}
      <button class="btn btn-primary btn-sm" on:click={() => submitAnalysisForGame()}>
        ▶ Analyze current game
      </button>
      <button class="btn btn-ghost btn-sm" on:click={checkForDesktopUpdates}>
        ⬇ Updates
      </button>
    {:else}
      <button class="btn btn-ghost btn-sm" on:click={checkForDesktopUpdates}>
        ⬇ Updates
      </button>
    {/if}

    {#if $backendStatus.running}
      <button class="btn btn-danger btn-sm" on:click={stopBackendProcess}>■ Stop backend</button>
    {:else}
      <button class="btn btn-primary btn-sm" on:click={startBackendProcess}>
        ▶ Start backend
      </button>
    {/if}
  </div>
</div>
