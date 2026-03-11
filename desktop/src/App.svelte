<script lang="ts">
  import { onMount } from 'svelte';
  import type { DesktopView } from './shared-types.js';
  import {
    desktopState,
    currentView,
    backendStatus,
    updateStatus,
    toastMsg,
    errorMsg,
    paletteOpen,
  } from './stores.js';
  import {
    loadDesktopState,
    initializeBackendListener,
    initializeUpdateListener,
  } from './desktop-api.js';

  // View components
  import Sidebar from './components/Sidebar.svelte';
  import Topbar from './components/Topbar.svelte';
  import Toast from './components/Toast.svelte';
  import CommandPalette from './components/CommandPalette.svelte';
  import DashboardView from './views/DashboardView.svelte';
  import GamesView from './views/GamesView.svelte';
  import BoardView from './views/BoardView.svelte';
  import ArchiveView from './views/ArchiveView.svelte';
  import AnalysisView from './views/AnalysisView.svelte';
  import EngineView from './views/EngineView.svelte';
  import LogsView from './views/LogsView.svelte';
  import SettingsView from './views/SettingsView.svelte';

  onMount(async () => {
    await loadDesktopState();
    initializeBackendListener();
    initializeUpdateListener();

    // Apply theme
    document.documentElement.setAttribute('data-theme', $desktopState.theme);

    // Keyboard shortcuts
    const handleKeydown = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key === 'k') {
        event.preventDefault();
        $paletteOpen = true;
      }
    };

    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });

  $: shellClass = $desktopState.compactMode ? 'shell shell-compact' : 'shell';
</script>

<div class={shellClass}>
  <Sidebar />

  <div class="content">
    {#if $toastMsg}
      <Toast message={$toastMsg} type="ok" />
    {/if}

    {#if $errorMsg}
      <Toast message={$errorMsg} type="error" />
    {/if}

    <Topbar />

    <main>
      {#if $currentView === 'dashboard'}
        <DashboardView />
      {:else if $currentView === 'games'}
        <GamesView />
      {:else if $currentView === 'board'}
        <BoardView />
      {:else if $currentView === 'archive'}
        <ArchiveView />
      {:else if $currentView === 'analysis'}
        <AnalysisView />
      {:else if $currentView === 'engine'}
        <EngineView />
      {:else if $currentView === 'logs'}
        <LogsView />
      {:else if $currentView === 'settings'}
        <SettingsView />
      {/if}
    </main>
  </div>
</div>

{#if $paletteOpen}
  <CommandPalette />
{/if}

<style lang="scss">
  @import './styles.scss';
</style>
