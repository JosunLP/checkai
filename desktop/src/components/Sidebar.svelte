<script lang="ts">
  import { desktopState, currentView, backendStatus } from '../stores.js';
  import { saveDesktopState } from '../desktop-api.js';
  import type { DesktopView } from '../shared-types.js';

  function navigateTo(view: DesktopView) {
    $currentView = view;
    desktopState.update((state) => ({ ...state, lastView: view }));
    saveDesktopState();
  }

  $: statusDot = $backendStatus.running ? 'online' : 'offline';
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
    </button>

    <button
      class="nav-btn"
      class:active={$currentView === 'games'}
      on:click={() => navigateTo('games')}
    >
      <span class="nav-icon">♟️</span>
      <span>Games</span>
    </button>

    <button
      class="nav-btn"
      class:active={$currentView === 'board'}
      on:click={() => navigateTo('board')}
    >
      <span class="nav-icon">🎮</span>
      <span>Board</span>
    </button>

    <span class="sidebar-section-label">Analysis</span>

    <button
      class="nav-btn"
      class:active={$currentView === 'analysis'}
      on:click={() => navigateTo('analysis')}
    >
      <span class="nav-icon">🔍</span>
      <span>Analysis</span>
    </button>

    <button
      class="nav-btn"
      class:active={$currentView === 'archive'}
      on:click={() => navigateTo('archive')}
    >
      <span class="nav-icon">📦</span>
      <span>Archive</span>
    </button>

    <span class="sidebar-section-label">System</span>

    <button
      class="nav-btn"
      class:active={$currentView === 'engine'}
      on:click={() => navigateTo('engine')}
    >
      <span class="nav-icon">⚙️</span>
      <span>Engine</span>
    </button>

    <button
      class="nav-btn"
      class:active={$currentView === 'logs'}
      on:click={() => navigateTo('logs')}
    >
      <span class="nav-icon">📝</span>
      <span>Logs</span>
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
