<script lang="ts">
  import { backendStatus, desktopState } from '../stores.js';
  import type { DesktopState } from '../shared-types.js';
  import {
    deletePreset,
    loadPresetIntoState,
    openWorkingDirectory,
    pickBackendExecutable,
    pickOpeningBook,
    pickTablebaseDirectory,
    pickWorkingDirectory,
    saveCurrentPreset,
    startBackendProcess,
    stopBackendProcess,
    updateDesktopState,
  } from '../workspace.js';

  function updateField<K extends keyof DesktopState>(key: K, value: DesktopState[K]): void {
    updateDesktopState((state) => ({ ...state, [key]: value }));
  }
</script>

<div class="view-grid">
  <div class="card">
    <div class="card-head">
      <div>
        <h2>Engine configuration</h2>
        <p class="dim">
          Control the local backend executable, workspace folders, and analysis assets.
        </p>
      </div>
      <span class="badge {$backendStatus.running ? 'badge-ok' : 'badge-danger'}">
        {$backendStatus.running ? 'Running' : 'Stopped'}
      </span>
    </div>

    <div class="field">
      <span>Backend URL</span>
      <input
        type="text"
        value={$desktopState.backendUrl}
        on:input={(event) =>
          updateField('backendUrl', (event.currentTarget as HTMLInputElement).value)}
      />
    </div>

    <div class="field">
      <span>Backend executable</span>
      <div class="btn-row">
        <input
          type="text"
          value={$desktopState.backendExecutable}
          on:input={(event) =>
            updateField('backendExecutable', (event.currentTarget as HTMLInputElement).value)}
        />
        <button class="btn btn-sm" on:click={pickBackendExecutable}>Browse…</button>
      </div>
    </div>

    <div class="field">
      <span>Backend arguments</span>
      <input
        type="text"
        value={$desktopState.backendArgs}
        on:input={(event) =>
          updateField('backendArgs', (event.currentTarget as HTMLInputElement).value)}
      />
    </div>

    <div class="field">
      <span>Working directory</span>
      <div class="btn-row">
        <input
          type="text"
          value={$desktopState.backendWorkingDirectory}
          on:input={(event) =>
            updateField(
              'backendWorkingDirectory',
              (event.currentTarget as HTMLInputElement).value
            )}
        />
        <button class="btn btn-sm" on:click={pickWorkingDirectory}>Choose…</button>
        <button class="btn btn-ghost btn-sm" on:click={openWorkingDirectory}>Open</button>
      </div>
    </div>

    <div class="field">
      <span>Opening book</span>
      <div class="btn-row">
        <input
          type="text"
          value={$desktopState.openingBookPath}
          on:input={(event) =>
            updateField('openingBookPath', (event.currentTarget as HTMLInputElement).value)}
        />
        <button class="btn btn-sm" on:click={pickOpeningBook}>Choose…</button>
      </div>
    </div>

    <div class="field">
      <span>Tablebase directory</span>
      <div class="btn-row">
        <input
          type="text"
          value={$desktopState.tablebasePath}
          on:input={(event) =>
            updateField('tablebasePath', (event.currentTarget as HTMLInputElement).value)}
        />
        <button class="btn btn-sm" on:click={pickTablebaseDirectory}>Choose…</button>
      </div>
    </div>

    <div class="checkbox-field">
      <input
        type="checkbox"
        checked={$desktopState.autoStartBackend}
        on:change={(event) =>
          updateField('autoStartBackend', (event.currentTarget as HTMLInputElement).checked)}
      />
      <span>Auto-start backend on desktop launch</span>
    </div>

    <div class="btn-row">
      <button class="btn btn-primary" on:click={startBackendProcess}>▶ Start backend</button>
      <button class="btn btn-danger" on:click={stopBackendProcess}>■ Stop backend</button>
      <button class="btn btn-ghost" on:click={saveCurrentPreset}>💾 Save preset</button>
    </div>

    <div class="callout">
      <strong>Status:</strong>
      {$backendStatus.running ? 'Running' : 'Stopped'}
      {#if $backendStatus.pid}
        (PID: {$backendStatus.pid})
      {/if}
      {#if $backendStatus.lastError}
        <div class="dim">{$backendStatus.lastError}</div>
      {/if}
    </div>
  </div>

  <div class="card">
    <div class="card-head">
      <div>
        <h3>Saved backend presets</h3>
        <p class="dim">
          {$desktopState.backendPresets.length} preset{$desktopState.backendPresets.length === 1
            ? ''
            : 's'} available for quick workspace switching.
        </p>
      </div>
    </div>

    {#if $desktopState.backendPresets.length === 0}
      <div class="empty-card">
        <p class="empty-text">No presets saved yet. Save the current backend setup to reuse it.</p>
      </div>
    {:else}
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Executable</th>
              <th>URL</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each $desktopState.backendPresets as preset (preset.id)}
              <tr>
                <td>{preset.name}</td>
                <td class="mono">{preset.backendExecutable}</td>
                <td class="mono">{preset.backendUrl}</td>
                <td class="btn-row">
                  <button class="btn btn-sm" on:click={() => loadPresetIntoState(preset.id)}>
                    Load
                  </button>
                  <button
                    class="btn btn-sm btn-danger"
                    on:click={() => deletePreset(preset.id)}
                  >
                    Delete
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>
