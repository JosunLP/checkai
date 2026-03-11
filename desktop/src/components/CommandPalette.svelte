<script lang="ts">
  import { onMount } from 'svelte';
  import { errorMsg, paletteOpen, paletteQuery } from '../stores.js';
  import type { DesktopView } from '../shared-types.js';
  import {
    checkForDesktopUpdates,
    createNewGame,
    importFenFromFile,
    navigateTo,
    startBackendProcess,
    stopBackendProcess,
  } from '../workspace.js';

  type Command = {
    id: string;
    name: string;
    description: string;
    action: () => void | Promise<void>;
  };

  function navigate(view: DesktopView): void {
    navigateTo(view);
    closePalette();
  }

  function closePalette(): void {
    $paletteOpen = false;
    $paletteQuery = '';
  }

  const commands: Command[] = [
    {
      id: 'new-game',
      name: 'New game',
      description: 'Create a fresh game and jump into the board view.',
      action: async () => {
        await createNewGame();
        closePalette();
      },
    },
    {
      id: 'import-fen',
      name: 'Import FEN file',
      description: 'Open the native file picker and import a position from disk.',
      action: async () => {
        await importFenFromFile();
        closePalette();
      },
    },
    {
      id: 'start-backend',
      name: 'Start backend',
      description: 'Launch the local CheckAI backend process.',
      action: async () => {
        await startBackendProcess();
        closePalette();
      },
    },
    {
      id: 'stop-backend',
      name: 'Stop backend',
      description: 'Stop the running backend process.',
      action: async () => {
        await stopBackendProcess();
        closePalette();
      },
    },
    {
      id: 'check-updates',
      name: 'Check for updates',
      description: 'Ask Electron to query desktop releases.',
      action: async () => {
        await checkForDesktopUpdates();
        closePalette();
      },
    },
    { id: 'dashboard', name: 'Dashboard', description: 'Open the dashboard.', action: () => navigate('dashboard') },
    { id: 'games', name: 'Games', description: 'Browse active games.', action: () => navigate('games') },
    { id: 'board', name: 'Board', description: 'Open the board view.', action: () => navigate('board') },
    { id: 'analysis', name: 'Analysis', description: 'Review engine jobs and results.', action: () => navigate('analysis') },
    { id: 'archive', name: 'Archive', description: 'Open archived games and replay positions.', action: () => navigate('archive') },
    { id: 'engine', name: 'Engine', description: 'Edit backend settings and presets.', action: () => navigate('engine') },
    { id: 'logs', name: 'Logs', description: 'Inspect native backend logs.', action: () => navigate('logs') },
    { id: 'settings', name: 'Settings', description: 'Adjust theme, notifications, and update behavior.', action: () => navigate('settings') },
  ];

  let searchInput: HTMLInputElement | null = null;

  onMount(() => {
    searchInput?.focus();
  });

  function handleOverlayClick(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      closePalette();
    }
  }

  async function executeCommand(command: Command): Promise<void> {
    try {
      await command.action();
    } catch (error) {
      errorMsg.set(error instanceof Error ? error.message : String(error));
      setTimeout(() => errorMsg.set(null), 5000);
    }
  }

  $: normalizedQuery = $paletteQuery.trim().toLowerCase();
  $: filteredCommands = commands.filter((command) => {
    if (!normalizedQuery) {
      return true;
    }

    return `${command.name} ${command.description}`.toLowerCase().includes(normalizedQuery);
  });
</script>

<div
  class="overlay"
  role="button"
  tabindex="0"
  aria-label="Close command palette"
  on:click={handleOverlayClick}
  on:keydown={(event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      closePalette();
    }
  }}
>
  <div class="palette">
    <div class="palette-head">
      <div>
        <h3>Command Palette</h3>
        <p class="dim">Jump between views and launch desktop actions.</p>
      </div>
      <button class="btn btn-ghost btn-sm" on:click={closePalette}>✕</button>
    </div>

    <label class="field-inline palette-search">
      <span>Search</span>
      <input
        bind:this={searchInput}
        bind:value={$paletteQuery}
        type="text"
        placeholder="Type to filter commands"
        on:keydown={(event) => {
          if (event.key === 'Enter' && filteredCommands[0]) {
            event.preventDefault();
            void executeCommand(filteredCommands[0]);
          }
        }}
      />
    </label>

    <div class="palette-grid">
      {#each filteredCommands as command (command.id)}
        <button class="palette-btn" on:click={() => executeCommand(command)}>
          <strong>{command.name}</strong>
          <span>{command.description}</span>
        </button>
      {/each}
    </div>
  </div>
</div>
