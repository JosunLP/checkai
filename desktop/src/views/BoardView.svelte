<script lang="ts">
  import { activeGame } from '../stores.js';
</script>

<div class="card">
  <div class="card-head">
    <h2>Chess Board</h2>
  </div>

  {#if $activeGame}
    <p>Board view for game: {$activeGame.game_id}</p>
    <!-- Full board implementation would go here -->
  {:else}
    <div class="empty-card">
      <p class="empty-text">No active game selected</p>
    </div>
  {/if}
</div>
