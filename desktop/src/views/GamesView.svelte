<script lang="ts">
  import {
    activeGame,
    backendStatus,
    fenInput,
    gamesList,
  } from '../stores.js';
  import {
    createNewGame,
    deleteGameById,
    importFenFromField,
    importFenFromFile,
    openGame,
    refreshGamesList,
    setAnalysisDepth,
    submitAnalysisForGame,
  } from '../workspace.js';

  const QUICK_ANALYSIS_DEPTH = 24;
</script>

<div class="view-grid">
  <div class="card hero-card">
    <div class="card-head">
      <div>
        <h2>Active Games</h2>
        <p class="dim">
          Manage live games, import positions, and launch analysis-ready sessions.
        </p>
      </div>
      <span class="badge {$backendStatus.running ? 'badge-ok' : 'badge-danger'}">
        {$backendStatus.running ? 'Backend online' : 'Backend offline'}
      </span>
    </div>

    <div class="quick-strip">
      <button class="qbtn" on:click={createNewGame}>
        <strong>♟ Create game</strong>
        <span>Start from the default position and open the desktop board.</span>
      </button>
      <button class="qbtn" on:click={importFenFromFile}>
        <strong>📂 Import file</strong>
        <span>Load a FEN file through the native desktop file picker.</span>
      </button>
      <button class="qbtn" on:click={() => refreshGamesList()}>
        <strong>↻ Refresh list</strong>
        <span>Re-sync the current game catalog from the local backend.</span>
      </button>
    </div>

    <div class="fen-row">
      <label class="field-inline">
        <span>Import FEN</span>
        <input bind:value={$fenInput} type="text" placeholder="Paste a FEN string" />
      </label>
      <button class="btn btn-primary btn-sm" on:click={importFenFromField}>
        Import Position
      </button>
    </div>
  </div>

  <div class="card">
    <div class="card-head">
      <div>
        <h3>Live sessions</h3>
        <p class="dim">
          {$gamesList.length} active game{$gamesList.length === 1 ? '' : 's'} available.
        </p>
      </div>
    </div>

    {#if $gamesList.length === 0}
      <div class="empty-card">
        <p class="empty-text">
          No active games yet. Create a game or import a FEN to get started.
        </p>
      </div>
    {:else}
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>Game</th>
              <th>Move</th>
              <th>Status</th>
              <th>Turn</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each $gamesList as game (game.game_id)}
              <tr class:selected-row={$activeGame?.game_id === game.game_id}>
                <td class="mono">{game.game_id}</td>
                <td>{game.fullmove_number}</td>
                <td>
                  <span class="badge {game.is_over ? 'badge-ok' : 'badge-active'}">
                    {game.is_over ? 'Finished' : 'In progress'}
                  </span>
                </td>
                <td>{game.turn}</td>
                <td class="btn-row">
                  <button class="btn btn-sm" on:click={() => openGame(game.game_id)}>
                    Open
                  </button>
                  <button
                    class="btn btn-sm btn-ghost"
                    on:click={() => {
                      setAnalysisDepth(QUICK_ANALYSIS_DEPTH);
                      void submitAnalysisForGame(game.game_id);
                    }}
                  >
                    Analyze
                  </button>
                  <button
                    class="btn btn-sm btn-danger"
                    on:click={() => deleteGameById(game.game_id)}
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
