<script lang="ts">
  import { desktopState, backendStatus } from '../stores.js';
  import { startBackend, stopBackend, saveDesktopState } from '../desktop-api.js';
</script>

<div class="card">
  <div class="card-head">
    <h2>Engine Configuration</h2>
  </div>

  <div class="field">
    <span>Backend URL</span>
    <input
      type="text"
      bind:value={$desktopState.backendUrl}
      on:change={saveDesktopState}
    />
  </div>

  <div class="field">
    <span>Backend Executable</span>
    <input
      type="text"
      bind:value={$desktopState.backendExecutable}
      on:change={saveDesktopState}
    />
  </div>

  <div class="field">
    <span>Backend Arguments</span>
    <input
      type="text"
      bind:value={$desktopState.backendArgs}
      on:change={saveDesktopState}
    />
  </div>

  <div class="checkbox-field">
    <input
      type="checkbox"
      bind:checked={$desktopState.autoStartBackend}
      on:change={saveDesktopState}
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
