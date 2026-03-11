<script lang="ts">
  import { paletteOpen, paletteQuery, currentView } from '../stores.js';
  import { desktopState } from '../stores.js';
  import { saveDesktopState } from '../desktop-api.js';
  import type { DesktopView } from '../shared-types.js';

  type Command = {
    id: string;
    name: string;
    description: string;
    action: () => void;
  };

  const commands: Command[] = [
    {
      id: 'dashboard',
      name: 'Dashboard',
      description: 'View dashboard',
      action: () => navigateTo('dashboard'),
    },
    {
      id: 'games',
      name: 'Games',
      description: 'View games',
      action: () => navigateTo('games'),
    },
    {
      id: 'board',
      name: 'Board',
      description: 'View board',
      action: () => navigateTo('board'),
    },
    {
      id: 'analysis',
      name: 'Analysis',
      description: 'View analysis',
      action: () => navigateTo('analysis'),
    },
    {
      id: 'archive',
      name: 'Archive',
      description: 'View archive',
      action: () => navigateTo('archive'),
    },
    {
      id: 'engine',
      name: 'Engine',
      description: 'View engine settings',
      action: () => navigateTo('engine'),
    },
    {
      id: 'logs',
      name: 'Logs',
      description: 'View logs',
      action: () => navigateTo('logs'),
    },
    {
      id: 'settings',
      name: 'Settings',
      description: 'View settings',
      action: () => navigateTo('settings'),
    },
  ];

  function navigateTo(view: DesktopView) {
    $currentView = view;
    desktopState.update((state) => ({ ...state, lastView: view }));
    saveDesktopState();
    closePalette();
  }

  function closePalette() {
    $paletteOpen = false;
    $paletteQuery = '';
  }

  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      closePalette();
    }
  }

  function handleOverlayKeydown(event: KeyboardEvent) {
    if (event.target !== event.currentTarget) {
      return;
    }

    if (event.key === 'Escape' || event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      closePalette();
    }
  }

  function handleCommand(command: Command) {
    command.action();
  }

  $: filteredCommands = commands.filter((cmd) =>
    cmd.name.toLowerCase().includes($paletteQuery.toLowerCase())
  );
</script>

<div
  class="overlay"
  data-close-palette
  role="button"
  tabindex="0"
  aria-label="Close command palette"
  on:click={handleOverlayClick}
  on:keydown={handleOverlayKeydown}
>
  <div class="palette">
    <div class="palette-head">
      <h3>Command Palette</h3>
      <button class="btn btn-ghost btn-sm" on:click={closePalette}>
        ✕
      </button>
    </div>

    <div class="palette-grid">
      {#each filteredCommands as command (command.id)}
        <button class="palette-btn" on:click={() => handleCommand(command)}>
          <strong>{command.name}</strong>
          <span>{command.description}</span>
        </button>
      {/each}
    </div>
  </div>
</div>
