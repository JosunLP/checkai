// ============================================================================
// CheckAI Web UI — Reactive Store (bQuery signals)
// ============================================================================

import { signal } from '@bquery/bquery/reactive';
import type {
  AnalysisPanelState,
  ArchivedGameSummary,
  EnginePanelState,
  Game,
  GameSummary,
  LegalMove,
  ReplayState,
  SquareName,
  StorageStats,
} from './types';

/** Application-wide reactive state powered by bQuery signals. */
export const store = {
  // ── Navigation ───────────────────────────────────────────────────────────
  currentView: signal<'dashboard' | 'game' | 'archive'>('dashboard'),

  // ── Game list ────────────────────────────────────────────────────────────
  games: signal<GameSummary[]>([]),

  // ── Current game ─────────────────────────────────────────────────────────
  currentGameId: signal<string | null>(null),
  currentGame: signal<Game | null>(null),
  legalMoves: signal<LegalMove[]>([]),
  isCheck: signal(false),

  // ── Board interaction ────────────────────────────────────────────────────
  selectedSquare: signal<SquareName | null>(null),
  legalTargets: signal<SquareName[]>([]),
  lastMove: signal<{ from: SquareName; to: SquareName } | null>(null),
  boardFlipped: signal(false),

  // ── Promotion ────────────────────────────────────────────────────────────
  pendingPromotion: signal<{ from: SquareName; to: SquareName } | null>(null),

  // ── Archive ──────────────────────────────────────────────────────────────
  archivedGames: signal<ArchivedGameSummary[]>([]),
  replayData: signal<ReplayState | null>(null),
  replayMoveNum: signal(0),
  replayTotalMoves: signal(0),

  // ── WebSocket ────────────────────────────────────────────────────────────
  wsConnected: signal(false),

  // ── Storage stats ────────────────────────────────────────────────────────
  storageStats: signal<StorageStats | null>(null),

  // ── Analysis ─────────────────────────────────────────────────────────────
  analysisJobId: signal<string | null>(null),
  analysisResult: signal<AnalysisPanelState | null>(null),
  analysisRunning: signal(false),

  // ── Live engine panel ────────────────────────────────────────────────────
  /** Latest single-position verdict from `POST /api/analysis/position`. */
  engine: signal<EnginePanelState>({ running: false, error: null, analysis: null }),
  /** Re-run the live engine automatically after every move. */
  engineAuto: signal(false),
  /** Search time per live evaluation, in milliseconds. */
  engineMovetimeMs: signal(1000),
  /** Number of principal variations the live engine reports. */
  engineMultiPv: signal(3),
  /** Search threads used by the live engine. */
  engineThreads: signal(1),
  /** Show the engine's best move as an arrow on the board. */
  engineShowArrow: signal(true),
};
