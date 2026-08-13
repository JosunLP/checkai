<script lang="ts">
  // Live engine panel: a single bounded search on the position currently on
  // the board, answered in one request by `POST /api/analysis/position`. It
  // complements the queued full-game analysis on the Analysis view.
  import {
    activeGame,
    engineAuto,
    engineMovetimeMs,
    engineMultiPv,
    enginePosition,
    engineRunning,
    engineThreads,
  } from '../stores.js';
  import { evaluateActivePosition } from '../workspace.js';

  /** White's winning expectancy for a centipawn score (logistic curve). */
  function winProbability(scoreWhiteCp: number): number {
    const cp = Math.max(-3000, Math.min(3000, scoreWhiteCp));
    return 1 / (1 + Math.pow(10, -cp / 400));
  }

  /** Formats a score as pawns (`+1.23`) or mate distance (`#3`). */
  function formatScore(scoreCp: number, mateIn: number | null): string {
    // The sign comes from the score, never from `mateIn`: the API reports
    // `mate_in` from the side to move's point of view while `score_white_cp`
    // is White's, so pairing them verbatim made a Black mate in three read
    // `#3` beside a bar pinned at 0% for Black.
    if (mateIn !== null && mateIn !== undefined) {
      return `#${scoreCp < 0 ? '-' : ''}${Math.abs(mateIn)}`;
    }
    const pawns = scoreCp / 100;
    return `${pawns >= 0 ? '+' : ''}${pawns.toFixed(2)}`;
  }

  /** Formats a node count compactly (`950`, `8.5k`, `1.2M`). */
  function humanize(n: number): string {
    if (n < 1000) return String(n);
    if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`;
    if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    return `${(n / 1_000_000_000).toFixed(2)}G`;
  }

  /**
   * Clamps a numeric input into its supported range.
   *
   * An empty or unparseable field keeps `fallback` — the setting currently in
   * force — rather than snapping to `min`, which would silently drop the
   * search budget to 10 ms the moment the user cleared the box to retype it.
   */
  function clamp(value: number, min: number, max: number, fallback: number): number {
    return Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : fallback;
  }

  /**
   * Applies a clamped value to both the store and the input element.
   *
   * Writing it back to the DOM is what keeps an out-of-range entry from
   * lingering on screen: Svelte only re-renders `value={...}` when the store
   * actually changes, so typing `999999` twice would leave the box showing
   * `999999` while the engine used the clamped 60000.
   */
  function applyNumber(
    input: HTMLInputElement,
    store: { set: (v: number) => void },
    min: number,
    max: number,
    fallback: number
  ): void {
    const next = clamp(Number.parseInt(input.value, 10), min, max, fallback);
    store.set(next);
    input.value = String(next);
  }
</script>

<section class="engine-panel" aria-label="Live engine">
  <header class="engine-header">
    <h3>Live engine</h3>
    <div class="engine-actions">
      <button
        class="btn btn-primary btn-sm"
        disabled={$engineRunning || !$activeGame}
        on:click={() => evaluateActivePosition()}
      >
        {$engineRunning ? 'Thinking…' : 'Evaluate'}
      </button>
      <label class="engine-toggle">
        <input type="checkbox" bind:checked={$engineAuto} />
        <span>Auto</span>
      </label>
    </div>
  </header>

  <div class="engine-settings">
    <label>
      <span>Time (ms)</span>
      <input
        type="number"
        min="10"
        max="60000"
        step="100"
        value={$engineMovetimeMs}
        on:change={(event) =>
          applyNumber(event.currentTarget, engineMovetimeMs, 10, 60000, $engineMovetimeMs)}
      />
    </label>
    <label>
      <span>Lines</span>
      <input
        type="number"
        min="1"
        max="16"
        value={$engineMultiPv}
        on:change={(event) =>
          applyNumber(event.currentTarget, engineMultiPv, 1, 16, $engineMultiPv)}
      />
    </label>
    <label>
      <span>Threads</span>
      <input
        type="number"
        min="1"
        max="64"
        value={$engineThreads}
        on:change={(event) =>
          applyNumber(event.currentTarget, engineThreads, 1, 64, $engineThreads)}
      />
    </label>
  </div>

  {#if $enginePosition}
    {@const analysis = $enginePosition}
    <div
      class="eval-bar"
      role="img"
      aria-label={`Evaluation ${formatScore(analysis.score_white_cp, analysis.mate_in)}`}
    >
      <div
        class="eval-bar-fill"
        style={`width: ${(winProbability(analysis.score_white_cp) * 100).toFixed(1)}%`}
      ></div>
      <span class="eval-bar-label">
        {formatScore(analysis.score_white_cp, analysis.mate_in)}
      </span>
    </div>

    <p class="engine-summary">
      depth {analysis.depth}/{analysis.seldepth} · {humanize(analysis.nodes)} nodes ·
      {humanize(analysis.nps)} n/s · {analysis.time_ms} ms · hash {analysis.hashfull}‰
      {#if analysis.source !== 'search'} · {analysis.source}{/if}
    </p>

    <ol class="engine-lines">
      {#each analysis.lines as line (line.rank)}
        <li class="engine-line" class:engine-line-best={line.rank === 1}>
          <!-- White's point of view, like the evaluation bar above: the
               side-to-move score flips sign on every ply. -->
          <span class="engine-line-score">{formatScore(line.score_white_cp, line.mate_in)}</span>
          <span class="engine-line-moves">{line.moves.join(' ')}</span>
        </li>
      {/each}
    </ol>

    {#if analysis.book && analysis.book.book_moves.length > 0}
      <h4 class="engine-subheading">Opening book</h4>
      <ul class="engine-book">
        <!-- Deliberately unkeyed: a polyglot book can hold several entries for
             the same move, and a duplicate key is a hard runtime error in
             Svelte 5 that would take down the whole board view. -->
        {#each analysis.book.book_moves as entry}
          <li>
            <span class="engine-book-move">{entry.notation}</span>
            <span class="engine-book-meter">
              <span
                class="engine-book-fill"
                style={`width: ${(entry.probability * 100).toFixed(1)}%`}
              ></span>
            </span>
            <span class="engine-book-percent">{(entry.probability * 100).toFixed(1)}%</span>
          </li>
        {/each}
      </ul>
    {/if}

    {#if analysis.tablebase && analysis.tablebase.wdl}
      <p class="engine-tablebase">
        Tablebase {analysis.tablebase.configuration}: {analysis.tablebase.wdl}
        ({analysis.tablebase.source})
      </p>
    {/if}
  {:else}
    <p class="engine-idle">
      {$engineRunning ? 'Thinking…' : 'Press Evaluate to see the engine’s verdict.'}
    </p>
  {/if}
</section>

<style>
  .engine-panel {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .engine-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .engine-header h3 {
    margin: 0;
    font-size: 0.95rem;
  }

  .engine-actions {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .engine-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.78rem;
    opacity: 0.8;
    cursor: pointer;
  }

  .engine-settings {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(5.5rem, 1fr));
    gap: 0.5rem;
  }

  .engine-settings label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.72rem;
    opacity: 0.75;
  }

  .engine-settings input {
    width: 100%;
    padding: 0.25rem 0.35rem;
    font-size: 0.82rem;
  }

  .eval-bar {
    position: relative;
    height: 1.5rem;
    border-radius: 6px;
    overflow: hidden;
    background: rgba(120, 130, 170, 0.18);
  }

  .eval-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, #6a8cff, #f0c846);
    transition: width 240ms ease;
  }

  .eval-bar-label {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.8rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: #10131c;
    text-shadow: 0 0 4px rgba(255, 255, 255, 0.5);
  }

  .engine-summary {
    margin: 0;
    font-size: 0.72rem;
    opacity: 0.7;
    font-variant-numeric: tabular-nums;
  }

  .engine-lines {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .engine-line {
    display: flex;
    gap: 0.6rem;
    align-items: baseline;
    font-size: 0.8rem;
    padding: 0.22rem 0.4rem;
    border-radius: 5px;
    background: rgba(120, 130, 170, 0.1);
  }

  .engine-line-best {
    background: rgba(106, 140, 255, 0.22);
  }

  .engine-line-score {
    flex: 0 0 3.2rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .engine-line-moves {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.85;
  }

  .engine-subheading {
    margin: 0.2rem 0 0;
    font-size: 0.78rem;
    opacity: 0.75;
  }

  .engine-book {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .engine-book li {
    display: grid;
    grid-template-columns: 3.4rem 1fr 3rem;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.76rem;
  }

  .engine-book-meter {
    height: 0.4rem;
    border-radius: 3px;
    background: rgba(120, 130, 170, 0.18);
    overflow: hidden;
  }

  .engine-book-fill {
    display: block;
    height: 100%;
    background: #34d399;
  }

  .engine-book-percent {
    text-align: right;
    font-variant-numeric: tabular-nums;
    opacity: 0.7;
  }

  .engine-tablebase,
  .engine-idle {
    margin: 0;
    font-size: 0.78rem;
    opacity: 0.7;
  }
</style>
