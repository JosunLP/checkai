<script lang="ts">
  import {
    activeAnalysis,
    activeGame,
    analysisJobs,
    archivedList,
    backendStatus,
    currentView,
    desktopState,
    gamesList,
  } from '../stores.js';
  import { navigateTo, openGame, viewAnalysisJob } from '../workspace.js';

  $: statusDot = $backendStatus.running ? 'online' : 'offline';
  $: runningAnalyses = $analysisJobs.filter(
    (job) => typeof job.status === 'object' && 'InProgress' in job.status
  ).length;
</script>

<aside class="sidebar">
  <div class="sidebar-top">
    <div class="brand">
      <div class="brand-icon">✓</div>
      <div>
        <h1>CheckAI</h1>
        <p>Chess Analysis</p>
      </div>
    </div>

    {#if $desktopState.backendUrl}
      <div class="sidebar-workspace-card">
        <strong>Workspace</strong>
        <p class="dim">{$desktopState.backendUrl}</p>
      </div>
    {/if}
  </div>

  <nav class="sidebar-nav">
    <span class="sidebar-kicker">Main</span>

    <button
      class="nav-btn"
      class:active={$currentView === 'dashboard'}
      on:click={() => navigateTo('dashboard')}
    >
      <span class="nav-icon">📊</span>
      <span>Dashboard</span>
      <span class="badge badge-dim">Home</span>
    </button>

    <button
      class="nav-btn"
      class:active={$currentView === 'games'}
      on:click={() => navigateTo('games')}
    >
      <span class="nav-icon">♟️</span>
      <span>Games</span>
      <span class="badge badge-dim">{$gamesList.length}</span>
    </button>

    <button
      class="nav-btn"
      class:active={$currentView === 'board'}
      on:click={() => navigateTo('board')}
    >
      <span class="nav-icon">🎮</span>
      <span>Board</span>
      {#if $activeGame}
        <span class="badge badge-active">Live</span>
      {/if}
    </button>

    <span class="sidebar-section-label">Analysis</span>

    <button
      class="nav-btn"
      class:active={$currentView === 'analysis'}
      on:click={() => navigateTo('analysis')}
    >
      <span class="nav-icon">🔍</span>
      <span>Analysis</span>
      {#if runningAnalyses > 0}
        <span class="badge badge-active">{runningAnalyses}</span>
      {/if}
    </button>

    <button
      class="nav-btn"
      class:active={$currentView === 'archive'}
      on:click={() => navigateTo('archive')}
    >
      <span class="nav-icon">📦</span>
      <span>Archive</span>
      <span class="badge badge-dim">{$archivedList.length}</span>
    </button>

    <span class="sidebar-section-label">System</span>

    <button
      class="nav-btn"
      class:active={$currentView === 'engine'}
      on:click={() => navigateTo('engine')}
    >
      <span class="nav-icon">⚙️</span>
      <span>Engine</span>
      {#if $desktopState.backendPresets.length > 0}
        <span class="badge badge-dim">{$desktopState.backendPresets.length}</span>
      {/if}
    </button>

    <button
      class="nav-btn"
      class:active={$currentView === 'logs'}
      on:click={() => navigateTo('logs')}
    >
      <span class="nav-icon">📝</span>
      <span>Logs</span>
      {#if $backendStatus.lastError}
        <span class="badge badge-danger">!</span>
      {/if}
    </button>

    <button
      class="nav-btn"
      class:active={$currentView === 'settings'}
      on:click={() => navigateTo('settings')}
    >
      <span class="nav-icon">🔧</span>
      <span>Settings</span>
    </button>
  </nav>

  {#if $activeGame || $activeAnalysis}
    <div class="sidebar-workspace-card">
      <strong>Quick resume</strong>
      {#if $activeGame}
        <button class="nav-btn nav-btn-inline" on:click={() => openGame($activeGame.game_id)}>
          <span class="nav-icon">♞</span>
          <span class="mono">{$activeGame.game_id.slice(0, 8)}…</span>
        </button>
      {/if}
      {#if $activeAnalysis}
        <button
          class="nav-btn nav-btn-inline"
          on:click={() => viewAnalysisJob($activeAnalysis.id)}
        >
          <span class="nav-icon">📈</span>
          <span class="mono">{$activeAnalysis.id.slice(0, 8)}…</span>
        </button>
      {/if}
    </div>
  {/if}

  <div class="sidebar-footer">
    <div class="status-dot {statusDot}"></div>
    <div>
      <strong>Backend</strong>
      <p class="dim">
        {$backendStatus.running ? 'Running' : 'Offline'}
        {#if $backendStatus.pid}
          (PID: {$backendStatus.pid})
        {/if}
      </p>
    </div>
  </div>
</aside>
