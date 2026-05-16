// ============================================================================
// CheckAI Desktop — Shared Types
// ============================================================================

// ── Desktop shell views ─────────────────────────────────────────────────────

export const DESKTOP_VIEWS = [
  'dashboard',
  'games',
  'board',
  'archive',
  'analysis',
  'engine',
  'logs',
  'settings',
] as const;

export type DesktopView = (typeof DESKTOP_VIEWS)[number];

export interface BackendPreset {
  id: string;
  name: string;
  backendExecutable: string;
  backendArgs: string;
  backendWorkingDirectory: string;
  backendUrl: string;
  openingBookPath: string;
  tablebasePath: string;
  autoStartBackend: boolean;
  createdAt: number;
}

export interface SaveTextFileOptions {
  defaultPath: string;
  content: string;
  filters?: Array<{
    name: string;
    extensions: string[];
  }>;
}

// ── Desktop state (persisted on disk) ───────────────────────────────────────

export interface DesktopState {
  backendUrl: string;
  autoStartBackend: boolean;
  backendExecutable: string;
  backendArgs: string;
  backendWorkingDirectory: string;
  openingBookPath: string;
  tablebasePath: string;
  lastView: DesktopView;
  theme: 'dark' | 'light' | 'system';
  boardFlipped: boolean;
  recentWorkspaces: string[];
  backendPresets: BackendPreset[];
  notificationsEnabled: boolean;
  compactMode: boolean;
  developerMode: boolean;
  lastGameId: string | null;
}

// ── Backend & update payloads ───────────────────────────────────────────────

export interface BackendStatusPayload {
  running: boolean;
  pid: number | null;
  command: string | null;
  startedAt: number | null;
  exitCode: number | null;
  lastError: string | null;
}

export interface UpdateStatusPayload {
  supported: boolean;
  currentVersion: string;
  state:
    | 'idle'
    | 'unsupported'
    | 'checking'
    | 'available'
    | 'downloading'
    | 'downloaded'
    | 'up-to-date'
    | 'error';
  availableVersion: string | null;
  percent: number | null;
  transferredBytes: number | null;
  totalBytes: number | null;
  message: string | null;
}

// ── Desktop API exposed via preload ─────────────────────────────────────────

export interface DesktopApi {
  getState(): Promise<DesktopState>;
  saveState(state: DesktopState): Promise<DesktopState>;
  getBackendStatus(): Promise<BackendStatusPayload>;
  getBackendLogs(): Promise<string>;
  getUpdateStatus(): Promise<UpdateStatusPayload>;
  setProgressBar(progress: number | null): Promise<void>;
  startBackend(state: DesktopState): Promise<BackendStatusPayload>;
  stopBackend(): Promise<BackendStatusPayload>;
  checkForUpdates(): Promise<UpdateStatusPayload>;
  downloadUpdate(): Promise<UpdateStatusPayload>;
  installUpdate(): Promise<void>;
  pickFile(): Promise<string | null>;
  pickDirectory(): Promise<string | null>;
  readTextFile(path: string): Promise<string>;
  saveTextFile(options: SaveTextFileOptions): Promise<string | null>;
  openPath(path: string): Promise<void>;
  openExternal(url: string): Promise<void>;
  notify(title: string, body: string): Promise<void>;
  onBackendStatus(callback: (status: BackendStatusPayload) => void): () => void;
  onBackendLogs(callback: (logs: string) => void): () => void;
  onUpdateStatus(callback: (status: UpdateStatusPayload) => void): () => void;
  onMenuCommand(callback: (command: string) => void): () => void;
}

export const DEFAULT_DESKTOP_STATE: DesktopState = {
  backendUrl: 'http://127.0.0.1:8080',
  autoStartBackend: false,
  backendExecutable: 'checkai',
  backendArgs: 'serve',
  backendWorkingDirectory: '',
  openingBookPath: '',
  tablebasePath: '',
  lastView: 'dashboard',
  theme: 'dark',
  boardFlipped: false,
  recentWorkspaces: [],
  backendPresets: [],
  notificationsEnabled: true,
  compactMode: false,
  developerMode: false,
  lastGameId: null,
};

// ── Engine API types ────────────────────────────────────────────────────────

export type PieceColor = 'white' | 'black';
export type FenChar =
  | 'K'
  | 'Q'
  | 'R'
  | 'B'
  | 'N'
  | 'P'
  | 'k'
  | 'q'
  | 'r'
  | 'b'
  | 'n'
  | 'p';
export type SquareName = string;
export type BoardMap = Partial<Record<SquareName, FenChar>>;

export interface SideCastling {
  kingside: boolean;
  queenside: boolean;
}
export interface CastlingRights {
  white: SideCastling;
  black: SideCastling;
}

export interface GameState {
  board: BoardMap;
  turn: PieceColor;
  castling: CastlingRights;
  en_passant: string | null;
  halfmove_clock: number;
  fullmove_number: number;
  position_history: string[];
}

export interface LegalMove {
  from: SquareName;
  to: SquareName;
  promotion?: string;
}

export interface MoveHistoryEntry {
  move_number: number;
  side: PieceColor;
  notation: string;
  move_json: { from: SquareName; to: SquareName; promotion?: string };
}

export type GameResult = 'WhiteWins' | 'BlackWins' | 'Draw' | null;

export type EndReason =
  | 'Checkmate'
  | 'Stalemate'
  | 'ThreefoldRepetition'
  | 'FivefoldRepetition'
  | 'FiftyMoveRule'
  | 'SeventyFiveMoveRule'
  | 'InsufficientMaterial'
  | 'Resignation'
  | 'DrawAgreement';

export interface Game {
  game_id: string;
  state: GameState;
  is_over: boolean;
  is_check: boolean;
  result: GameResult;
  end_reason: EndReason | null;
  legal_move_count: number;
  move_history: MoveHistoryEntry[];
}

export interface GameSummary {
  game_id: string;
  turn: PieceColor;
  fullmove_number: number;
  is_over: boolean;
  result: GameResult;
}

export interface MoveResponse {
  success: boolean;
  message: string;
  state: GameState;
  is_over: boolean;
  result: GameResult;
  end_reason: EndReason | null;
  is_check: boolean;
}

export interface ArchivedGameSummary {
  game_id: string;
  result: GameResult;
  end_reason: EndReason | null;
  move_count: number;
  compressed_bytes: number;
  start_timestamp: number;
  end_timestamp: number;
  raw_bytes: number;
}

export interface StorageStats {
  active_count: number;
  archived_count: number;
  active_bytes: number;
  archive_bytes: number;
  total_bytes: number;
}

export interface ReplayState {
  game_id: string;
  at_move: number;
  total_moves: number;
  state: GameState;
  is_over: boolean;
  result: GameResult;
  is_check: boolean;
}

// ── Analysis types ──────────────────────────────────────────────────────────

export type AnalysisMoveQuality =
  | 'Best'
  | 'Excellent'
  | 'Good'
  | 'Inaccuracy'
  | 'Mistake'
  | 'Blunder'
  | 'Book';
export type AnalysisWdl = 'Win' | 'Draw' | 'Loss' | 'CursedWin' | 'BlessedLoss';

export interface AnalysisMoveJson {
  from: SquareName;
  to: SquareName;
  promotion: string | null;
}

export interface AnalysisBookMoveEntry {
  notation: string;
  weight: number;
  probability: number;
}

export interface AnalysisBookInfo {
  is_book_move: boolean;
  weight: number;
  total_weight: number;
  book_moves: AnalysisBookMoveEntry[];
  opening_name: string | null;
}

export interface AnalysisTablebaseInfo {
  is_tablebase_position: boolean;
  wdl: AnalysisWdl | null;
  dtz: number | null;
  configuration: string;
  source: string;
}

export interface AnalysisMoveAnnotation {
  move_number: number;
  side: PieceColor;
  played_move: AnalysisMoveJson;
  best_move: AnalysisMoveJson;
  played_eval: number;
  best_eval: number;
  centipawn_loss: number;
  quality: AnalysisMoveQuality;
  is_book_move: boolean;
  is_tablebase_position: boolean;
  book_info?: AnalysisBookInfo;
  tablebase_info?: AnalysisTablebaseInfo;
  search_depth: number;
  principal_variation: string[];
}

export interface AnalysisMoveSummary {
  total_moves: number;
  best_moves: number;
  excellent_moves: number;
  good_moves: number;
  inaccuracies: number;
  mistakes: number;
  blunders: number;
  book_moves: number;
  average_centipawn_loss: number;
  white_accuracy: number;
  black_accuracy: number;
  white_avg_cp_loss: number;
  black_avg_cp_loss: number;
}

export interface AnalysisResultPayload {
  annotations: AnalysisMoveAnnotation[];
  summary: AnalysisMoveSummary;
  depth: number;
  book_available: boolean;
  tablebase_available: boolean;
}

export type AnalysisStatus =
  | 'Queued'
  | 'Completed'
  | 'Cancelled'
  | { InProgress: { moves_analyzed: number; total_moves: number } }
  | { Failed: { error: string } };

export interface AnalysisJob {
  id: string;
  game_id: string | null;
  status: AnalysisStatus;
  result?: AnalysisResultPayload;
  created_at: number;
  completed_at?: number | null;
}

// ── Constants ───────────────────────────────────────────────────────────────

export const FILES = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'] as const;
export const RANKS = ['1', '2', '3', '4', '5', '6', '7', '8'] as const;
export const PROMOTION_PIECES = ['Q', 'R', 'B', 'N'] as const;
export const PROMOTION_PIECE_LIST = PROMOTION_PIECES.join(', ');

export const PIECE_UNICODE: Record<FenChar, string> = {
  K: '♔',
  Q: '♕',
  R: '♖',
  B: '♗',
  N: '♘',
  P: '♙',
  k: '♚',
  q: '♛',
  r: '♜',
  b: '♝',
  n: '♞',
  p: '♟',
};
