<script lang="ts">
  import { desktopState, backendStatus } from '../stores.js';
  import { startBackend, stopBackend, saveDesktopState } from '../desktop-api.js';
  import type { DesktopState } from '../shared-types.js';

  function updateDesktopField<K extends keyof DesktopState>(
    key: K,
    value: DesktopState[K]
  ) {
    desktopState.update((state) => ({ ...state, [key]: value }));
    saveDesktopState();
  }
</script>

<div class="card">
  <div class="card-head">
    <h2>Engine Configuration</h2>
  </div>

  <div class="field">
    <span>Backend URL</span>
    <input
      type="text"
      value={$desktopState.backendUrl}
      on:input={(event) =>
        updateDesktopField(
          'backendUrl',
          (event.currentTarget as HTMLInputElement).value
        )}
    />
  </div>

  <div class="field">
    <span>Backend Executable</span>
    <input
      type="text"
      value={$desktopState.backendExecutable}
      on:input={(event) =>
        updateDesktopField(
          'backendExecutable',
          (event.currentTarget as HTMLInputElement).value
        )}
    />
  </div>

  <div class="field">
    <span>Backend Arguments</span>
    <input
      type="text"
      value={$desktopState.backendArgs}
      on:input={(event) =>
        updateDesktopField(
          'backendArgs',
          (event.currentTarget as HTMLInputElement).value
        )}
    />
  </div>

  <div class="checkbox-field">
    <input
      type="checkbox"
      checked={$desktopState.autoStartBackend}
      on:change={(event) =>
        updateDesktopField(
          'autoStartBackend',
          (event.currentTarget as HTMLInputElement).checked
        )}
    />
    <span>Auto-start backend</span>
  </div>

  <div class="btn-row">
    <button class="btn btn-primary" on:click={startBackend}>Start Backend</button>
    <button class="btn btn-danger" on:click={stopBackend}>Stop Backend</button>
  </div>

  <div class="callout">
    <strong>Status:</strong>
    {$backendStatus.running ? 'Running' : 'Stopped'}
    {#if $backendStatus.pid}
      (PID: {$backendStatus.pid})
    {/if}
  </div>
</div>
