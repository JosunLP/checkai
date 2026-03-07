// ============================================================================
// CheckAI Web UI — API Client
// ============================================================================

import type {
  ActionSubmission,
  AnalysisJob,
  AnalysisRequest,
  ArchivedGameSummary,
  Game,
  GameSummary,
  LegalMove,
  MoveResponse,
  MoveSubmission,
  ReplayState,
  StorageStats,
} from './types';

const API_BASE = `${window.location.origin}/api`;

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const opts: RequestInit = {
    method,
    headers: { 'Content-Type': 'application/json' },
  };
  if (body) opts.body = JSON.stringify(body);

  const res = await fetch(`${API_BASE}${path}`, opts);
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(err.error || res.statusText);
  }
  const ct = res.headers.get('Content-Type') || '';
  if (ct.includes('application/json')) return res.json() as Promise<T>;
  // For non-JSON responses (plain text), try to parse as JSON first;
  // if that fails, return the raw text. This avoids the unsafe `as unknown as T` cast.
  const text = await res.text();
  try {
    return JSON.parse(text) as T;
  } catch {
    return text as unknown as T;
  }
}

// ── Game CRUD ────────────────────────────────────────────────────────────────

export function createGame(): Promise<{ game_id: string }> {
  return request('POST', '/games');
}

export function listGames(): Promise<{ games: GameSummary[] }> {
  return request('GET', '/games');
}

export function getGame(id: string): Promise<Game> {
  return request('GET', `/games/${encodeURIComponent(id)}`);
}

export function deleteGame(id: string): Promise<void> {
  return request('DELETE', `/games/${encodeURIComponent(id)}`);
}

// ── Moves & Actions ──────────────────────────────────────────────────────────

export function submitMove(id: string, move: MoveSubmission): Promise<MoveResponse> {
  return request('POST', `/games/${encodeURIComponent(id)}/move`, move);
}

export function submitAction(id: string, action: ActionSubmission): Promise<MoveResponse> {
  return request('POST', `/games/${encodeURIComponent(id)}/action`, action);
}

export function getLegalMoves(id: string): Promise<{ moves: LegalMove[] }> {
  return request('GET', `/games/${encodeURIComponent(id)}/moves`);
}

// ── Archive ──────────────────────────────────────────────────────────────────

export function listArchived(): Promise<{
  games: ArchivedGameSummary[];
  storage?: StorageStats;
}> {
  return request('GET', '/archive');
}

export function getArchived(id: string): Promise<Game> {
  return request('GET', `/archive/${encodeURIComponent(id)}`);
}

export function replayArchived(id: string, moveNum?: number): Promise<ReplayState> {
  const q = moveNum !== undefined ? `?move_number=${moveNum}` : '';
  return request('GET', `/archive/${encodeURIComponent(id)}/replay${q}`);
}

export function getStorageStats(): Promise<StorageStats> {
  return request('GET', '/archive/stats');
}

// ── Analysis ─────────────────────────────────────────────────────────────────

export function startAnalysis(id: string, opts?: AnalysisRequest): Promise<{ job_id: string }> {
  return request('POST', `/analysis/game/${encodeURIComponent(id)}`, opts ?? {});
}

export function getAnalysis(jobId: string): Promise<AnalysisJob> {
  return request('GET', `/analysis/jobs/${encodeURIComponent(jobId)}`);
}

export function cancelAnalysis(jobId: string): Promise<void> {
  return request('DELETE', `/analysis/jobs/${encodeURIComponent(jobId)}`);
}

// ── FEN Import/Export ────────────────────────────────────────────────────────

export function exportFen(id: string): Promise<{ fen: string }> {
  return request('GET', `/games/${encodeURIComponent(id)}/fen`);
}

export function importFen(fen: string): Promise<{ game_id: string }> {
  return request('POST', '/games/fen', { fen });
}

// ── PGN Export ───────────────────────────────────────────────────────────────

export function exportPgn(id: string): Promise<string> {
  return request('GET', `/games/${encodeURIComponent(id)}/pgn`);
}
