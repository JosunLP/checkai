// ============================================================================
// CheckAI Desktop — Engine REST API Client
// ============================================================================

import type {
  AnalysisJob,
  ArchivedGameSummary,
  Game,
  GameSummary,
  LegalMove,
  MoveResponse,
  ReplayState,
  StorageStats,
} from './shared-types.js';

let apiBase = 'http://127.0.0.1:8080/api';

export function setApiBase(backendUrl: string): void {
  apiBase = `${backendUrl.replace(/\/+$/, '')}/api`;
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown
): Promise<T> {
  const opts: RequestInit = {
    method,
    headers: { 'Content-Type': 'application/json' },
  };
  if (body) opts.body = JSON.stringify(body);

  const res = await fetch(`${apiBase}${path}`, opts);
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error((err as Record<string, string>).error || res.statusText);
  }
  const ct = res.headers.get('Content-Type') || '';
  if (ct.includes('application/json')) return res.json() as Promise<T>;
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

export function listGames(): Promise<{ games: GameSummary[]; total: number }> {
  return request('GET', '/games');
}

export function getGame(id: string): Promise<Game> {
  return request('GET', `/games/${encodeURIComponent(id)}`);
}

export function deleteGame(id: string): Promise<void> {
  return request('DELETE', `/games/${encodeURIComponent(id)}`);
}

// ── Moves & Actions ──────────────────────────────────────────────────────────

export function submitMove(
  id: string,
  from: string,
  to: string,
  promotion?: string
): Promise<MoveResponse> {
  return request('POST', `/games/${encodeURIComponent(id)}/move`, {
    from,
    to,
    promotion,
  });
}

export function submitAction(
  id: string,
  action: string,
  reason?: string
): Promise<MoveResponse> {
  return request('POST', `/games/${encodeURIComponent(id)}/action`, {
    action,
    reason,
  });
}

export function getLegalMoves(
  id: string
): Promise<{ moves: LegalMove[]; turn: string; count: number }> {
  return request('GET', `/games/${encodeURIComponent(id)}/moves`);
}

export function getBoardAscii(id: string): Promise<string> {
  return request('GET', `/games/${encodeURIComponent(id)}/board`);
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

export function replayArchived(
  id: string,
  moveNum?: number
): Promise<ReplayState> {
  const q = moveNum !== undefined ? `?move_number=${moveNum}` : '';
  return request('GET', `/archive/${encodeURIComponent(id)}/replay${q}`);
}

export function getStorageStats(): Promise<StorageStats> {
  return request('GET', '/archive/stats');
}

// ── Analysis ─────────────────────────────────────────────────────────────────

export function startAnalysis(
  gameId: string,
  depth?: number
): Promise<{ job_id: string; message: string }> {
  return request(
    'POST',
    `/analysis/game/${encodeURIComponent(gameId)}`,
    depth ? { depth } : {}
  );
}

export function listAnalysisJobs(): Promise<{
  jobs: AnalysisJob[];
  count: number;
}> {
  return request('GET', '/analysis/jobs');
}

export function getAnalysisJob(jobId: string): Promise<AnalysisJob> {
  return request('GET', `/analysis/jobs/${encodeURIComponent(jobId)}`);
}

export function cancelAnalysisJob(jobId: string): Promise<void> {
  return request('DELETE', `/analysis/jobs/${encodeURIComponent(jobId)}`);
}

// ── FEN / PGN ────────────────────────────────────────────────────────────────

export function exportFen(id: string): Promise<{ fen: string }> {
  return request('GET', `/games/${encodeURIComponent(id)}/fen`);
}

export function importFen(fen: string): Promise<{ game_id: string }> {
  return request('POST', '/games/fen', { fen });
}

export function exportPgn(id: string): Promise<string> {
  return request('GET', `/games/${encodeURIComponent(id)}/pgn`);
}
