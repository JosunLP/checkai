<script lang="ts">
  import { desktopState } from '../stores.js';
  import { saveDesktopState } from '../desktop-api.js';

  function toggleTheme() {
    const themes: Array<'dark' | 'light' | 'system'> = ['dark', 'light', 'system'];
    const currentIndex = themes.indexOf($desktopState.theme);
    const nextIndex = (currentIndex + 1) % themes.length;
    $desktopState.theme = themes[nextIndex];
    document.documentElement.setAttribute('data-theme', $desktopState.theme);
    saveDesktopState();
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
      bind:checked={$desktopState.compactMode}
      on:change={saveDesktopState}
    />
    <span>Compact Mode</span>
  </div>

  <div class="checkbox-field">
    <input
      type="checkbox"
      bind:checked={$desktopState.notificationsEnabled}
      on:change={saveDesktopState}
    />
    <span>Enable Notifications</span>
  </div>

  <div class="checkbox-field">
    <input
      type="checkbox"
      bind:checked={$desktopState.developerMode}
      on:change={saveDesktopState}
    />
    <span>Developer Mode</span>
  </div>

  <div class="checkbox-field">
    <input
      type="checkbox"
      bind:checked={$desktopState.boardFlipped}
      on:change={saveDesktopState}
    />
    <span>Flip Board</span>
  </div>
</div>
