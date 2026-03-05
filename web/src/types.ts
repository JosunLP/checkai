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
}

/** A legal move returned by the API */
export interface LegalMove {
  from: SquareName;
  to: SquareName;
  promotion?: string;
  notation: string;
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
  action: 'resign' | 'offer_draw' | 'claim_draw';
  reason?: string;
}

/** Move response from the API */
export interface MoveResponse {
  message: string;
  is_over: boolean;
}

/** Analysis result from the API */
export interface AnalysisResult {
  job_id: string;
  game_id: string;
  status: 'completed' | 'running' | 'cancelled';
  depth: number;
  score: number;
  best_move: string | null;
  pv: string[];
  nodes: number;
  time_ms: number;
  nps: number;
}

/** Analysis request */
export interface AnalysisRequest {
  depth?: number;
  time_limit_ms?: number;
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
