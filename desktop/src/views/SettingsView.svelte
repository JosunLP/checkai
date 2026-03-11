<script lang="ts">
  import { desktopState } from '../stores.js';
  import { saveDesktopState } from '../desktop-api.js';
  import type { DesktopState } from '../shared-types.js';

  function updateDesktopState(updater: (state: DesktopState) => DesktopState) {
    desktopState.update((state) => {
      const nextState = updater(state);
      document.documentElement.setAttribute('data-theme', nextState.theme);
      return nextState;
    });
    saveDesktopState();
  }

  function toggleTheme() {
    const themes: Array<'dark' | 'light' | 'system'> = ['dark', 'light', 'system'];
    const currentIndex = themes.indexOf($desktopState.theme);
    const nextIndex = (currentIndex + 1) % themes.length;
    updateDesktopState((state) => ({ ...state, theme: themes[nextIndex] }));
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
        updateDesktopState((state) => ({
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
        updateDesktopState((state) => ({
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
        updateDesktopState((state) => ({
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
        updateDesktopState((state) => ({
          ...state,
          boardFlipped: (event.currentTarget as HTMLInputElement).checked,
        }))}
    />
    <span>Flip Board</span>
  </div>
</div>
