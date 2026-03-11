<script lang="ts">
  import { gamesList } from '../stores.js';
</script>

<div class="card">
  <div class="card-head">
    <h2>Active Games</h2>
  </div>

  {#if $gamesList.length === 0}
    <div class="empty-card">
      <p class="empty-text">No active games</p>
    </div>
  {:else}
    <div class="mini-list">
      {#each $gamesList as game (game.game_id)}
        <div class="mini-item">
          <span class="mono">{game.game_id}</span>
          <div>
            <span class="badge badge-dim">Move {game.fullmove_number}</span>
            {#if game.is_over}
              <span class="badge badge-ok">Finished</span>
            {:else}
              <span class="badge badge-active">In Progress</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
