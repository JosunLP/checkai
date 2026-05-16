<script lang="ts">
  import {
    activeGame,
    boardAscii,
    desktopState,
    highlightSquares,
    lastMove,
    legalMoves,
    selectedSquare,
  } from '../stores.js';
  import {
    copyActiveFen,
    copyActivePgn,
    handleBoardSquareClick,
    openBackendInBrowser,
    saveActiveFen,
    saveActivePgn,
    submitAnalysisForGame,
    updateDesktopState,
  } from '../workspace.js';
  import { FILES, PIECE_UNICODE, RANKS } from '../shared-types.js';

  type MovePair = {
    moveNumber: number;
    white?: string;
    black?: string;
  };

  type BoardFile = (typeof FILES)[number];
  type BoardRank = (typeof RANKS)[number];

  let boardFiles: BoardFile[] = [...FILES];
  let boardRanks: BoardRank[] = [...RANKS].reverse();
  let historyPairs: MovePair[] = [];

  function isLightSquare(file: BoardFile, rank: BoardRank): boolean {
    return (FILES.indexOf(file) + Number.parseInt(rank, 10) - 1) % 2 === 1;
  }

  const PIECE_LABELS: Record<string, string> = {
    K: 'white king',
    Q: 'white queen',
    R: 'white rook',
    B: 'white bishop',
    N: 'white knight',
    P: 'white pawn',
    k: 'black king',
    q: 'black queen',
    r: 'black rook',
    b: 'black bishop',
    n: 'black knight',
    p: 'black pawn',
  };

  function squareAriaLabel(
    square: string,
    piece: string | undefined,
    options: {
      selected: boolean;
      legalDestination: boolean;
      lastMove: boolean;
      inCheck: boolean;
    }
  ): string {
    const parts = [`square ${square}`, piece ? `occupied by ${PIECE_LABELS[piece]}` : 'empty'];
    if (options.selected) {
      parts.push('selected');
    }
    if (options.legalDestination) {
      parts.push('legal move');
    }
    if (options.lastMove) {
      parts.push('last move');
    }
    if (options.inCheck) {
      parts.push('king in check');
    }
    return parts.join(', ');
  }

  function movePairs(): MovePair[] {
    const game = $activeGame;
    if (!game) {
      return [];
    }

    const pairs: MovePair[] = [];
    let currentPair: MovePair | null = null;

    for (const entry of game.move_history) {
      if (!currentPair || currentPair.moveNumber !== entry.move_number) {
        currentPair = {
          moveNumber: entry.move_number,
        };
        pairs.push(currentPair);
      }

      if (entry.side === 'white') {
        currentPair.white = entry.notation;
      } else {
        currentPair.black = entry.notation;
      }
    }

    return pairs;
  }

  $: boardFiles = $desktopState.boardFlipped ? [...FILES].reverse() : [...FILES];
  $: boardRanks = $desktopState.boardFlipped ? [...RANKS] : [...RANKS].reverse();
  $: historyPairs = movePairs();
</script>

<div class="view-grid">
  {#if $activeGame}
    <div class="card board-card">
      <div class="card-head">
        <div>
          <h2>Board · <span class="mono">{$activeGame.game_id.slice(0, 8)}…</span></h2>
          <p class="dim">
            {$activeGame.state.turn} to move · {$activeGame.move_history.length} ply recorded
            {#if $activeGame.is_check}
              · Check
            {/if}
            {#if $activeGame.is_over}
              · {$activeGame.result ?? 'Finished'}
            {/if}
          </p>
        </div>
        <div class="btn-row">
          <button class="btn btn-primary btn-sm" on:click={() => submitAnalysisForGame()}>
            📊 Analyze
          </button>
          <button
            class="btn btn-ghost btn-sm"
            on:click={() =>
              updateDesktopState((state) => ({
                ...state,
                boardFlipped: !state.boardFlipped,
              }))}
          >
            ⇄ Flip
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
                {@const piece = $activeGame.state.board[square]}
                {@const isSelected = $selectedSquare === square}
                {@const isLegalDestination = $highlightSquares.has(square)}
                {@const isLastMove = $lastMove?.from === square || $lastMove?.to === square}
                {@const isCheckSquare = $activeGame.is_check &&
                  (($activeGame.state.turn === 'white' &&
                    $activeGame.state.board[square] === 'K') ||
                    ($activeGame.state.turn === 'black' &&
                      $activeGame.state.board[square] === 'k'))}
                <button
                  type="button"
                  class:sq={true}
                  class:sq-light={isLightSquare(file, rank)}
                  class:sq-dark={!isLightSquare(file, rank)}
                  class:sq-selected={isSelected}
                  class:sq-highlight={isLegalDestination}
                  class:sq-last-move={isLastMove}
                  class:sq-check={isCheckSquare}
                  aria-label={squareAriaLabel(square, piece, {
                    selected: isSelected,
                    legalDestination: isLegalDestination,
                    lastMove: isLastMove,
                    inCheck: isCheckSquare,
                  })}
                  aria-pressed={isSelected}
                  on:click={() => handleBoardSquareClick(square)}
                >
                  <span>{piece ? PIECE_UNICODE[piece] : ''}</span>
                </button>
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

      <div class="btn-row">
        <button class="btn btn-sm" on:click={copyActiveFen}>📋 Copy FEN</button>
        <button class="btn btn-sm" on:click={saveActiveFen}>💾 Save FEN</button>
        <button class="btn btn-sm" on:click={copyActivePgn}>📋 Copy PGN</button>
        <button class="btn btn-sm" on:click={saveActivePgn}>💾 Save PGN</button>
        <button class="btn btn-ghost btn-sm" on:click={openBackendInBrowser}>
          🌐 Open backend
        </button>
      </div>
    </div>

    <div class="card">
      <div class="card-head">
        <div>
          <h3>Move timeline</h3>
          <p class="dim">
            {$legalMoves.length} legal move{$legalMoves.length === 1 ? '' : 's'} from the current
            position.
          </p>
        </div>
      </div>

      {#if historyPairs.length === 0}
        <div class="empty-card">
          <p class="empty-text">No moves played yet.</p>
        </div>
      {:else}
        <div class="table-wrap">
          <table class="data-table compact">
            <thead>
              <tr>
                <th>#</th>
                <th>White</th>
                <th>Black</th>
              </tr>
            </thead>
            <tbody>
              {#each historyPairs as pair}
                <tr>
                  <td>{pair.moveNumber}</td>
                  <td>{pair.white ?? '—'}</td>
                  <td>{pair.black ?? '—'}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}

      <div class="card-head card-head-spaced">
        <div>
          <h3>Advanced board view</h3>
          <p class="dim">Desktop-native debugging aids for the active position.</p>
        </div>
      </div>
      <pre class="ascii-panel">{$boardAscii || 'Loading ASCII board…'}</pre>

      {#if $desktopState.developerMode}
        <details class="details-panel">
          <summary>Raw game JSON</summary>
          <pre class="json-panel">{JSON.stringify($activeGame, null, 2)}</pre>
        </details>
      {/if}
    </div>
  {:else}
    <div class="card">
      <div class="empty-card">
        <p class="empty-text">
          Open a game from the Games view to use the interactive desktop board.
        </p>
      </div>
    </div>
  {/if}
</div>
