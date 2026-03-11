<script lang="ts">
  import { analysisJobs } from '../stores.js';
</script>

<div class="card">
  <div class="card-head">
    <h2>Analysis Jobs</h2>
  </div>

  {#if $analysisJobs.length === 0}
    <div class="empty-card">
      <p class="empty-text">No analysis jobs</p>
    </div>
  {:else}
    <div class="mini-list">
      {#each $analysisJobs as job (job.id)}
        <div class="mini-item">
          <span class="mono">{job.id}</span>
          <span class="badge badge-active">
            {typeof job.status === 'object' && 'InProgress' in job.status
              ? 'In Progress'
              : String(job.status)}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</div>
