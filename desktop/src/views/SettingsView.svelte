<script lang="ts">
  import { desktopState, updateStatus } from '../stores.js';
  import type { DesktopState } from '../shared-types.js';
  import {
    checkForDesktopUpdates,
    downloadDesktopUpdate,
    installDesktopUpdate,
    updateDesktopState,
  } from '../workspace.js';

  function updateState(updater: (state: DesktopState) => DesktopState) {
    updateDesktopState(updater);
  }

  function toggleTheme() {
    const themes: Array<'dark' | 'light' | 'system'> = ['dark', 'light', 'system'];
    const currentIndex = themes.indexOf($desktopState.theme);
    const nextIndex = (currentIndex + 1) % themes.length;
    updateState((state) => ({ ...state, theme: themes[nextIndex] }));
  }
</script>

<div class="card">
  <div class="card-head">
    <h2>Settings</h2>
  </div>

  <div class="field">
    <span>Theme</span>
    <button class="btn" on:click={toggleTheme}>
      Current: {$desktopState.theme}
    </button>
  </div>

  <div class="checkbox-field">
    <input
      type="checkbox"
      checked={$desktopState.compactMode}
      on:change={(event) =>
        updateState((state) => ({
          ...state,
          compactMode: (event.currentTarget as HTMLInputElement).checked,
        }))}
    />
    <span>Compact Mode</span>
  </div>

  <div class="checkbox-field">
    <input
      type="checkbox"
      checked={$desktopState.notificationsEnabled}
      on:change={(event) =>
        updateState((state) => ({
          ...state,
          notificationsEnabled: (event.currentTarget as HTMLInputElement).checked,
        }))}
    />
    <span>Enable Notifications</span>
  </div>

  <div class="checkbox-field">
    <input
      type="checkbox"
      checked={$desktopState.developerMode}
      on:change={(event) =>
        updateState((state) => ({
          ...state,
          developerMode: (event.currentTarget as HTMLInputElement).checked,
        }))}
    />
    <span>Developer Mode</span>
  </div>

  <div class="checkbox-field">
    <input
      type="checkbox"
      checked={$desktopState.boardFlipped}
      on:change={(event) =>
        updateState((state) => ({
          ...state,
          boardFlipped: (event.currentTarget as HTMLInputElement).checked,
        }))}
    />
    <span>Flip Board</span>
  </div>

  <div class="card-head" style="margin-top: 1rem">
    <h3>Desktop updates</h3>
  </div>
  <p class="dim">{$updateStatus.message ?? 'No update information yet.'}</p>
  <div class="btn-row">
    <button class="btn btn-sm" on:click={checkForDesktopUpdates}>Check now</button>
    <button
      class="btn btn-sm"
      disabled={$updateStatus.state !== 'available'}
      on:click={downloadDesktopUpdate}
    >
      Download
    </button>
    <button
      class="btn btn-sm btn-primary"
      disabled={$updateStatus.state !== 'downloaded'}
      on:click={installDesktopUpdate}
    >
      Install
    </button>
  </div>

  {#if $desktopState.recentWorkspaces.length > 0}
    <div class="card-head" style="margin-top: 1rem">
      <h3>Recent workspaces</h3>
    </div>
    <div class="mini-list">
      {#each $desktopState.recentWorkspaces as workspace (workspace)}
        <div class="mini-item">
          <span class="mono">{workspace}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
