<script lang="ts">
  import { backendLogs, backendStatus } from '../stores.js';
  import {
    openWorkingDirectory,
    refreshLogs,
    startBackendProcess,
    stopBackendProcess,
  } from '../workspace.js';
</script>

<div class="view-grid">
  <div class="card">
    <div class="card-head">
      <div>
        <h2>Backend Logs</h2>
        <p class="dim">
          Inspect the native backend process output and jump to the current workspace folder.
        </p>
      </div>
      <div class="btn-row">
        <button class="btn btn-ghost btn-sm" on:click={() => refreshLogs()}>↻ Refresh</button>
        <button class="btn btn-ghost btn-sm" on:click={openWorkingDirectory}>
          📂 Open folder
        </button>
        {#if $backendStatus.running}
          <button class="btn btn-danger btn-sm" on:click={stopBackendProcess}>■ Stop</button>
        {:else}
          <button class="btn btn-primary btn-sm" on:click={startBackendProcess}>▶ Start</button>
        {/if}
      </div>
    </div>

    <pre class="log-panel">{$backendLogs || 'No logs available yet.'}</pre>
  </div>
</div>
