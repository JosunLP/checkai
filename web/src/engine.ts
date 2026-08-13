// ============================================================================
// CheckAI Web UI — Live Engine Panel
// ============================================================================
//
// The counterpart to the job-based game analysis: a single bounded search on
// the position currently on the board, answered in one request. It drives the
// evaluation bar, the best-move arrow, the MultiPV candidate list and the
// opening-book / tablebase readouts.

import { batch } from '@bquery/bquery/reactive';
import * as api from './api';
import { t } from './i18n';
import { store } from './store';
import type { PositionAnalysis } from './types';
import { showToast } from './ui';

/** Guards against a slow response overwriting a newer one. */
let requestToken = 0;

/** The evaluation currently in flight; at most one runs at a time. */
let inFlight: Promise<void> | null = null;

/** Set when the position moved on while a search was still running. */
let rerunQueued = false;

/** White's winning expectancy for a centipawn score (logistic curve). */
export function winProbability(scoreWhiteCp: number): number {
  const cp = Math.max(-3000, Math.min(3000, scoreWhiteCp));
  return 1 / (1 + Math.pow(10, -cp / 400));
}

/**
 * Formats a score as pawns (`+1.23`) or mate distance (`#3`, `#-2`).
 *
 * The sign comes from `scoreCp`, never from `mateIn`. The API reports
 * `mate_in` from the side to move's point of view while `score_white_cp` is
 * White's, so pairing the two verbatim made a Black mate in three read `#3`
 * next to an evaluation bar pinned at 0% for Black.
 */
export function formatScore(scoreCp: number, mateIn: number | null): string {
  if (mateIn !== null && mateIn !== undefined) {
    return `#${scoreCp < 0 ? '-' : ''}${Math.abs(mateIn)}`;
  }
  const pawns = scoreCp / 100;
  return `${pawns >= 0 ? '+' : ''}${pawns.toFixed(2)}`;
}

/** Formats a node count compactly (`950`, `8.5k`, `1.2M`). */
export function humanizeCount(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`;
  if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  return `${(n / 1_000_000_000).toFixed(2)}G`;
}

/**
 * Runs one live evaluation of the current game position.
 *
 * Only one search is ever in flight. The server answers a position analysis
 * synchronously and cannot stop a search once the engine has started it, so a
 * second request would occupy another engine slot for a position the user has
 * already left — and be turned away with `429` once the server is saturated.
 * A position change during a search therefore only sets a flag; the search
 * that lands re-runs once, for whatever is on the board by then.
 */
export async function evaluatePosition(): Promise<void> {
  if (inFlight) {
    rerunQueued = true;
    return inFlight;
  }

  try {
    do {
      rerunQueued = false;
      inFlight = runEvaluation();
      await inFlight;
    } while (rerunQueued);
  } finally {
    inFlight = null;
    // `runEvaluation` sets `running` before it awaits and returns without
    // clearing it when the answer turns out to be stale — which is what
    // happens when the game is deleted or switched mid-search. Nothing else
    // ever clears the flag, so the panel would sit on "Thinking…" with the
    // Evaluate button disabled until another game was loaded.
    if (store.engine.value.running) {
      store.engine.value = { ...store.engine.value, running: false };
      renderEnginePanel();
    }
  }
}

/** One request/response round trip against the position endpoint. */
async function runEvaluation(): Promise<void> {
  const gameId = store.currentGameId.value;
  if (!gameId) return;

  const token = ++requestToken;
  batch(() => {
    store.engine.value = { ...store.engine.value, running: true, error: null };
  });
  renderEnginePanel();

  try {
    const analysis = await api.analyzePosition({
      game_id: gameId,
      movetime_ms: store.engineMovetimeMs.value,
      multi_pv: store.engineMultiPv.value,
      threads: store.engineThreads.value,
    });
    // Both the token and the game must still be current: the panel describes
    // the board in front of the user, not the one the search started from.
    if (token !== requestToken || store.currentGameId.value !== gameId) return;
    store.engine.value = { running: false, error: null, analysis };
  } catch (err: unknown) {
    if (token !== requestToken || store.currentGameId.value !== gameId) return;
    const message = err instanceof Error ? err.message : String(err);
    store.engine.value = { running: false, error: message, analysis: null };
    showToast(t('toast.error', { error: message }), 'error');
  }
  renderEnginePanel();
}

/** Clears the live engine state (called when the game changes). */
export function resetEngineState(): void {
  requestToken++;
  // Whatever the queued re-run was for is no longer on the board.
  rerunQueued = false;
  // A request already on the wire cannot be recalled — the server runs the
  // search to completion either way. Reporting `running: false` while one is
  // still out there re-enables the Evaluate button, and pressing it only sets
  // `rerunQueued`, so the click would appear to do nothing at all.
  store.engine.value = { running: inFlight !== null, error: null, analysis: null };
  renderEnginePanel();
}

/** Re-evaluates when auto-analysis is on; otherwise clears stale output. */
export function onPositionChanged(): void {
  if (store.engineAuto.value) {
    // A search may still be running for the position we just left; whatever
    // it returns must not land in the panel.
    requestToken++;
    // Drop the previous position's verdict right away. It stays on screen for
    // the whole of the next search otherwise — and the board reads
    // `analysis.best_move` for the hint, so it would point at a move for a
    // position that is no longer in front of the user.
    store.engine.value = { ...store.engine.value, error: null, analysis: null };
    renderEnginePanel();
    void evaluatePosition();
  } else {
    resetEngineState();
  }
}

/** Renders the live engine panel into `#engine-content`. */
export function renderEnginePanel(): void {
  const host = document.getElementById('engine-content');
  if (!host) return;

  const state = store.engine.value;
  const startBtn = document.getElementById('btn-engine-run') as HTMLButtonElement | null;
  if (startBtn) startBtn.disabled = state.running || !store.currentGameId.value;

  if (state.error) {
    host.innerHTML = '';
    host.appendChild(paragraph(state.error, 'engine-error'));
    return;
  }
  if (!state.analysis) {
    const key = state.running ? 'engine.thinking' : 'engine.idle';
    host.innerHTML = '';
    // Tagged with `data-i18n` so a later language switch re-translates it.
    // This paragraph replaces the static one from the HTML shell, and
    // `translateDom` only revisits nodes that carry the attribute.
    host.appendChild(paragraph(t(key), 'analysis-idle', key));
    return;
  }

  host.innerHTML = '';
  host.appendChild(buildEvalBar(state.analysis));
  host.appendChild(buildSummary(state.analysis));
  host.appendChild(buildLines(state.analysis));

  const book = state.analysis.book;
  if (book && book.book_moves.length > 0) {
    host.appendChild(buildBook(book.book_moves));
  }
  const tablebase = state.analysis.tablebase;
  if (tablebase && tablebase.wdl) {
    host.appendChild(
      paragraph(
        t('engine.tablebase', {
          config: tablebase.configuration,
          wdl: tablebase.wdl,
          source: tablebase.source,
        }),
        'engine-tablebase',
      ),
    );
  }
}

/** The horizontal evaluation bar, scaled by White's winning expectancy. */
function buildEvalBar(analysis: PositionAnalysis): HTMLElement {
  const wrap = document.createElement('div');
  wrap.className = 'eval-bar';
  wrap.setAttribute('role', 'img');
  wrap.setAttribute(
    'aria-label',
    t('engine.eval_aria', {
      score: formatScore(analysis.score_white_cp, analysis.mate_in),
    }),
  );

  const fill = document.createElement('div');
  fill.className = 'eval-bar-fill';
  fill.style.width = `${(winProbability(analysis.score_white_cp) * 100).toFixed(1)}%`;

  const label = document.createElement('span');
  label.className = 'eval-bar-label';
  label.textContent = formatScore(analysis.score_white_cp, analysis.mate_in);

  wrap.appendChild(fill);
  wrap.appendChild(label);
  return wrap;
}

/** Depth / node / speed summary line. */
function buildSummary(analysis: PositionAnalysis): HTMLElement {
  const el = document.createElement('div');
  el.className = 'engine-summary';
  const parts = [
    t('engine.depth', { depth: analysis.depth, seldepth: analysis.seldepth }),
    t('engine.nodes', {
      nodes: humanizeCount(analysis.nodes),
      nps: humanizeCount(analysis.nps),
    }),
    `${analysis.time_ms} ms`,
    t('engine.hashfull', { permille: analysis.hashfull }),
  ];
  if (analysis.source !== 'search') parts.push(analysis.source);
  el.textContent = parts.join(' · ');
  return el;
}

/** The MultiPV candidate list; clicking a line plays its first move. */
function buildLines(analysis: PositionAnalysis): HTMLElement {
  const list = document.createElement('ol');
  list.className = 'engine-lines';
  for (const line of analysis.lines) {
    const item = document.createElement('li');
    item.className = 'engine-line';

    const score = document.createElement('span');
    score.className = 'engine-line-score';
    // White's point of view, like the evaluation bar directly above — the
    // side-to-move score flips sign every ply, so the same move read +3.10
    // here and -3.10 in the bar.
    score.textContent = formatScore(line.score_white_cp, line.mate_in);

    const moves = document.createElement('span');
    moves.className = 'engine-line-moves';
    moves.textContent = line.moves.join(' ');

    item.appendChild(score);
    item.appendChild(moves);
    list.appendChild(item);
  }
  return list;
}

/** Opening-book moves with their relative popularity. */
function buildBook(entries: { notation: string; probability: number }[]): HTMLElement {
  const wrap = document.createElement('div');
  wrap.className = 'engine-book';

  const heading = document.createElement('h4');
  heading.textContent = t('engine.book');
  wrap.appendChild(heading);

  for (const entry of entries) {
    const row = document.createElement('div');
    row.className = 'engine-book-row';

    const move = document.createElement('span');
    move.className = 'engine-book-move';
    move.textContent = entry.notation;

    const meter = document.createElement('div');
    meter.className = 'engine-book-meter';
    const fill = document.createElement('div');
    fill.className = 'engine-book-fill';
    fill.style.width = `${(entry.probability * 100).toFixed(1)}%`;
    meter.appendChild(fill);

    const percent = document.createElement('span');
    percent.className = 'engine-book-percent';
    percent.textContent = `${(entry.probability * 100).toFixed(1)}%`;

    row.appendChild(move);
    row.appendChild(meter);
    row.appendChild(percent);
    wrap.appendChild(row);
  }
  return wrap;
}

/** Small helper for a single styled paragraph. */
function paragraph(text: string, className: string, i18nKey?: string): HTMLElement {
  const el = document.createElement('p');
  el.className = className;
  el.textContent = text;
  if (i18nKey) el.dataset.i18n = i18nKey;
  return el;
}

/** Wires up the engine panel controls. */
export function bindEngineEvents(): void {
  document
    .getElementById('btn-engine-run')
    ?.addEventListener('click', () => void evaluatePosition());

  const auto = document.getElementById('toggle-engine-auto') as HTMLInputElement | null;
  if (auto) {
    auto.checked = store.engineAuto.value;
    auto.addEventListener('change', () => {
      store.engineAuto.value = auto.checked;
      if (auto.checked) void evaluatePosition();
    });
  }

  bindNumberInput('input-engine-movetime', store.engineMovetimeMs, 10, 60000);
  bindNumberInput('input-engine-multipv', store.engineMultiPv, 1, 16);
  bindNumberInput('input-engine-threads', store.engineThreads, 1, 64);

  const arrow = document.getElementById('toggle-engine-arrow') as HTMLInputElement | null;
  if (arrow) {
    arrow.checked = store.engineShowArrow.value;
    arrow.addEventListener('change', () => {
      store.engineShowArrow.value = arrow.checked;
    });
  }
}

/** Binds a numeric input to a signal, clamping to `[min, max]`. */
function bindNumberInput(id: string, target: { value: number }, min: number, max: number): void {
  const input = document.getElementById(id) as HTMLInputElement | null;
  if (!input) return;
  input.value = String(target.value);
  input.addEventListener('change', () => {
    const parsed = Number.parseInt(input.value, 10);
    const clamped = Number.isFinite(parsed) ? Math.min(max, Math.max(min, parsed)) : target.value;
    target.value = clamped;
    input.value = String(clamped);
  });
}
