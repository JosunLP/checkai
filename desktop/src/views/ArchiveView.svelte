<script lang="ts">
  import { archivedList } from '../stores.js';
</script>

<div class="card">
  <div class="card-head">
    <h2>Archived Games</h2>
  </div>

  {#if $archivedList.length === 0}
    <div class="empty-card">
      <p class="empty-text">No archived games</p>
    </div>
  {:else}
    <div class="mini-list">
      {#each $archivedList as game (game.game_id)}
        <div class="mini-item">
          <span class="mono">{game.game_id}</span>
          <span class="badge badge-dim">{game.move_count} moves</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
