// ============================================================================
// CheckAI Web UI — Reactive Store (bQuery signals)
// ============================================================================

import { signal } from '@bquery/bquery/reactive';
import type {
  ArchivedGameSummary,
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
  analysisResult: signal<{
    depth: number;
    score: number;
    bestMove: string | null;
    pv: string[];
    nodes: number;
    nps: number;
    timeMs: number;
  } | null>(null),
  analysisRunning: signal(false),
};
