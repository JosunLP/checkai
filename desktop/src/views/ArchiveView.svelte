<script lang="ts">
  import { archivedList, desktopState, replayState } from '../stores.js';
  import { PIECE_UNICODE, FILES, RANKS } from '../shared-types.js';
  import {
    closeReplay,
    openArchivedReplay,
    refreshArchive,
    replayTo,
    setAnalysisDepth,
    submitAnalysisForGame,
  } from '../workspace.js';

  const ARCHIVE_ANALYSIS_DEPTH = 30;

  function isLightSquare(file: string, rank: string): boolean {
    return (file.charCodeAt(0) + Number.parseInt(rank, 10)) % 2 === 0;
  }

  $: boardFiles = $desktopState.boardFlipped ? [...FILES].reverse() : [...FILES];
  $: boardRanks = $desktopState.boardFlipped ? [...RANKS] : [...RANKS].reverse();
</script>

<div class="view-grid">
  {#if $replayState}
    <div class="card board-card">
      <div class="card-head">
        <div>
          <h2>Replay · <span class="mono">{$replayState.game_id.slice(0, 8)}…</span></h2>
          <p class="dim">Move {$replayState.at_move} / {$replayState.total_moves}</p>
        </div>
        <div class="btn-row">
          <button
            class="btn btn-sm"
            type="button"
            aria-label="First move"
            title="First move"
            on:click={() => replayTo(0)}
          >
            ⏮
          </button>
          <button
            class="btn btn-sm"
            type="button"
            aria-label="Previous move"
            title="Previous move"
            on:click={() => replayTo(Math.max(0, $replayState.at_move - 1))}
          >
            ◀
          </button>
          <button
            class="btn btn-sm"
            type="button"
            aria-label="Next move"
            title="Next move"
            on:click={() =>
              replayTo(Math.min($replayState.total_moves, $replayState.at_move + 1))}
          >
            ▶
          </button>
          <button
            class="btn btn-sm"
            type="button"
            aria-label="Last move"
            title="Last move"
            on:click={() => replayTo($replayState.total_moves)}
          >
            ⏭
          </button>
          <button
            class="btn btn-ghost btn-sm"
            type="button"
            aria-label="Close replay"
            title="Close replay"
            on:click={closeReplay}
          >
            ✕ Close
          </button>
        </div>
      </div>

      <div class="board-container">
        <div class="chess-board">
          {#each boardRanks as rank}
            <div class="board-row">
              <span class="rank-label">{rank}</span>
              {#each boardFiles as file}
                {@const square = `${file}${rank}`}
                {@const piece = $replayState.state.board[square]}
                <div
                  class:sq={true}
                  class:sq-light={isLightSquare(file, rank)}
                  class:sq-dark={!isLightSquare(file, rank)}
                >
                  {piece ? PIECE_UNICODE[piece] : ''}
                </div>
              {/each}
            </div>
          {/each}
          <div class="file-labels">
            <span></span>
            {#each boardFiles as file}
              <span>{file}</span>
            {/each}
          </div>
        </div>
      </div>

      <input
        class="replay-slider"
        type="range"
        min="0"
        max={$replayState.total_moves}
        value={$replayState.at_move}
        on:input={(event) => replayTo(Number.parseInt((event.currentTarget as HTMLInputElement).value, 10))}
      />
    </div>
  {/if}

  <div class="card">
    <div class="card-head">
      <div>
        <h2>Archived games</h2>
        <p class="dim">
          {$archivedList.length} archived game{$archivedList.length === 1 ? '' : 's'} stored locally.
        </p>
      </div>
      <button class="btn btn-ghost btn-sm" on:click={() => refreshArchive()}>↻ Refresh</button>
    </div>

    {#if $archivedList.length === 0}
      <div class="empty-card">
        <p class="empty-text">No archived games yet. Finished games will appear here.</p>
      </div>
    {:else}
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>Game</th>
              <th>Result</th>
              <th>Reason</th>
              <th>Moves</th>
              <th>Size</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each $archivedList as game (game.game_id)}
              <tr>
                <td class="mono">{game.game_id}</td>
                <td>{game.result ?? '—'}</td>
                <td>{game.end_reason ?? '—'}</td>
                <td>{game.move_count}</td>
                <td>{game.compressed_bytes} B</td>
                <td class="btn-row">
                  <button class="btn btn-sm" on:click={() => openArchivedReplay(game.game_id)}>
                    Replay
                  </button>
                  <button
                    class="btn btn-sm btn-ghost"
                    on:click={() => {
                      setAnalysisDepth(ARCHIVE_ANALYSIS_DEPTH);
                      void submitAnalysisForGame(game.game_id);
                    }}
                  >
                    Analyze
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
