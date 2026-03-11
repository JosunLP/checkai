<script lang="ts">
  import { onMount } from 'svelte';
  import {
    desktopState,
    currentView,
    toastMsg,
    errorMsg,
    modalState,
    paletteOpen,
  } from './stores.js';
  import {
    initializeBackendListener,
    initializeUpdateListener,
  } from './desktop-api.js';
  import {
    createNewGame,
    handleDesktopCommand,
    initializeDesktopWorkspace,
    refreshCurrentView,
  } from './workspace.js';

  // View components
  import Sidebar from './components/Sidebar.svelte';
  import Topbar from './components/Topbar.svelte';
  import Toast from './components/Toast.svelte';
  import CommandPalette from './components/CommandPalette.svelte';
  import ModalDialog from './components/ModalDialog.svelte';
  import DashboardView from './views/DashboardView.svelte';
  import GamesView from './views/GamesView.svelte';
  import BoardView from './views/BoardView.svelte';
  import ArchiveView from './views/ArchiveView.svelte';
  import AnalysisView from './views/AnalysisView.svelte';
  import EngineView from './views/EngineView.svelte';
  import LogsView from './views/LogsView.svelte';
  import SettingsView from './views/SettingsView.svelte';

  onMount(() => {
    let cleanupWorkspace: () => void = () => {};
    let cleanupBackendListener: () => void = () => {};
    let cleanupUpdateListener: () => void = () => {};

    const handleKeydown = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      if ((event.ctrlKey || event.metaKey) && key === 'k') {
        event.preventDefault();
        $paletteOpen = true;
        return;
      }

      if ((event.ctrlKey || event.metaKey) && key === 'n') {
        event.preventDefault();
        void createNewGame();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && key === 'o') {
        event.preventDefault();
        void handleDesktopCommand('import-fen-file');
        return;
      }

      if ((event.ctrlKey || event.metaKey) && key === 'r' && !event.shiftKey) {
        event.preventDefault();
        void refreshCurrentView();
      }
    };

    initializeDesktopWorkspace()
      .then((cleanup) => {
        cleanupBackendListener = initializeBackendListener();
        cleanupUpdateListener = initializeUpdateListener();
        return cleanup;
      })
      .then((cleanup) => {
        cleanupWorkspace = cleanup;
      })
      .catch((error) => {
        console.error('Failed to initialize desktop UI:', error);
      });

    window.addEventListener('keydown', handleKeydown);
    return () => {
      cleanupWorkspace();
      cleanupBackendListener();
      cleanupUpdateListener();
      window.removeEventListener('keydown', handleKeydown);
    };
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

{#if $modalState}
  <ModalDialog />
{/if}
