<script lang="ts">
  import { gamesList, storageStats, analysisJobs } from '../stores.js';
  import { startBackend, stopBackend } from '../desktop-api.js';
</script>

<div class="view-grid">
  <div class="card hero-card">
    <div class="card-head">
      <h2>Welcome to CheckAI Desktop</h2>
    </div>
    <p class="dim">
      A powerful chess analysis engine with comprehensive game tracking, analysis,
      and replay capabilities.
    </p>

    <div class="quick-strip">
      <button class="qbtn" on:click={startBackend}>
        <strong>Start Backend</strong>
        <span>Launch the chess engine</span>
      </button>
      <button class="qbtn" on:click={stopBackend}>
        <strong>Stop Backend</strong>
        <span>Shut down the engine</span>
      </button>
    </div>
  </div>

  <div class="card">
    <div class="card-head">
      <h3>Active Games</h3>
    </div>
    <p class="dim">{$gamesList.length} game(s)</p>
  </div>

  <div class="card">
    <div class="card-head">
      <h3>Analysis Jobs</h3>
    </div>
    <p class="dim">{$analysisJobs.length} job(s)</p>
  </div>

  {#if $storageStats}
    <div class="card">
      <div class="card-head">
        <h3>Storage</h3>
      </div>
      <div class="stat-grid">
        <div class="stat">
          <span class="stat-label">Active</span>
          <strong>{$storageStats.active_count}</strong>
        </div>
        <div class="stat">
          <span class="stat-label">Archived</span>
          <strong>{$storageStats.archived_count}</strong>
        </div>
      </div>
    </div>
  {/if}
</div>
