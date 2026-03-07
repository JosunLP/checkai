// ============================================================================
// CheckAI Web UI — Core Types
// ============================================================================

/** Chess piece color */
export type PieceColor = 'white' | 'black';

/** Chess piece kind character (FEN) */
export type FenChar = 'K' | 'Q' | 'R' | 'B' | 'N' | 'P' | 'k' | 'q' | 'r' | 'b' | 'n' | 'p';

/** Square name like 'e4' */
export type SquareName = string;

/** Board map: square name → FEN char (or null) */
export type BoardMap = Record<SquareName, FenChar | null>;

/** Side-specific castling rights */
export interface SideCastling {
  kingside: boolean;
  queenside: boolean;
}

/** Full castling rights */
export interface CastlingRights {
  white: SideCastling;
  black: SideCastling;
}

/** Game state from the API */
export interface GameState {
  board: BoardMap;
  turn: PieceColor;
  castling: CastlingRights;
  en_passant: string | null;
  halfmove_clock: number;
  fullmove_number: number;
  position_history: string[];
}

/** A legal move returned by the API */
export interface LegalMove {
  from: SquareName;
  to: SquareName;
  promotion?: string;
}

/** Move history entry */
export interface MoveHistoryEntry {
  move_number: number;
  side: PieceColor;
  notation: string;
  move_json: {
    from: SquareName;
    to: SquareName;
    promotion?: string;
  };
}

/** End result */
export type GameResult = 'WhiteWins' | 'BlackWins' | 'Draw' | null;

/** End reason */
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

/** Full game object from the API */
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

/** Game summary in list responses */
export interface GameSummary {
  game_id: string;
  turn: PieceColor;
  fullmove_number: number;
  is_over: boolean;
  result: GameResult;
}

/** Archived game summary */
export interface ArchivedGameSummary {
  game_id: string;
  result: GameResult;
  end_reason: EndReason | null;
  move_count: number;
  compressed_bytes: number;
  start_timestamp: number | null;
}

/** Storage statistics */
export interface StorageStats {
  active_count: number;
  archived_count: number;
  active_bytes: number;
  archive_bytes: number;
}

/** Replay state from the API */
export interface ReplayState {
  game_id: string;
  at_move: number;
  total_moves: number;
  state: GameState;
}

/** WebSocket incoming message */
export interface WsMessage {
  type: 'event';
  event: 'game_updated' | 'game_created' | 'game_deleted';
  game_id?: string;
}

/** WebSocket outgoing payload */
export interface WsPayload {
  action: 'subscribe' | 'unsubscribe';
  game_id: string;
}

/** Move submission */
export interface MoveSubmission {
  from: SquareName;
  to: SquareName;
  promotion?: string;
}

/** Action submission */
export interface ActionSubmission {
  action: 'resign' | 'offer_draw' | 'accept_draw' | 'claim_draw';
  reason?: string;
}

/** Move response from the API */
export interface MoveResponse {
  success: boolean;
  message: string;
  state: GameState;
  is_over: boolean;
  result: GameResult;
  end_reason: EndReason | null;
  is_check: boolean;
}

export interface AnalysisRequest {
  depth?: number;
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

export interface AnalysisPanelState {
  statusText: string;
  progressText: string | null;
  errorMessage: string | null;
  depth: number | null;
  totalMoves: number | null;
  averageCpLoss: number | null;
  whiteAccuracy: number | null;
  blackAccuracy: number | null;
  whiteAverageCpLoss: number | null;
  blackAverageCpLoss: number | null;
  bookAvailable: boolean | null;
  tablebaseAvailable: boolean | null;
  counts: {
    best: number;
    excellent: number;
    good: number;
    inaccuracies: number;
    mistakes: number;
    blunders: number;
    book: number;
  } | null;
}

// ============================================================================
// Constants
// ============================================================================

export const FILES = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'] as const;
export const RANKS = ['1', '2', '3', '4', '5', '6', '7', '8'] as const;

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
