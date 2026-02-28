//! Game state management for the CheckAI chess engine.
//!
//! This module manages the lifecycle of a chess game: creating games,
//! processing moves, detecting game-ending conditions, and maintaining
//! the full game history. It acts as the central coordinator between
//! the board representation and the move generator.

use crate::movegen;
use crate::storage::{self, GameStorage};
use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Game struct
// ---------------------------------------------------------------------------

/// A complete chess game with full state and history tracking.
///
/// This is the primary structure managed by the server. Each game
/// maintains its board position, move history, position history
/// (for repetition detection), and game result.
#[derive(Debug, Clone)]
pub struct Game {
    /// Unique identifier for the game.
    pub id: Uuid,

    /// Current board position.
    pub board: Board,

    /// Side to move.
    pub turn: Color,

    /// Castling rights.
    pub castling: CastlingRights,

    /// En passant target square (if a pawn just advanced two squares).
    pub en_passant: Option<Square>,

    /// Half-move clock for the 50-move rule.
    pub halfmove_clock: u32,

    /// Full-move number (starts at 1, incremented after Black moves).
    pub fullmove_number: u32,

    /// History of position FEN strings for threefold repetition detection.
    pub position_history: Vec<String>,

    /// History of moves made in the game (as JSON-compatible objects).
    pub move_history: Vec<MoveRecord>,

    /// The game result, if the game has ended.
    pub result: Option<GameResult>,

    /// The reason the game ended, if applicable.
    pub end_reason: Option<GameEndReason>,

    /// Whether a draw has been offered by the current side.
    pub draw_offered_by: Option<Color>,

    /// Unix timestamp when the game was created.
    pub start_timestamp: u64,

    /// Unix timestamp when the game ended (0 if still active).
    pub end_timestamp: u64,
}

/// A record of a single move in the game history.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MoveRecord {
    /// The move number (fullmove_number at the time of the move).
    pub move_number: u32,
    /// Which side made the move.
    pub side: Color,
    /// The move in algebraic notation (e.g. "e2e4").
    pub notation: String,
    /// The move as a JSON-compatible object.
    pub move_json: MoveJson,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    /// Creates a new game from the standard starting position.
    pub fn new() -> Self {
        let board = Board::starting_position();
        let castling = CastlingRights::default();
        let turn = Color::White;
        let en_passant = None;

        let initial_fen = board.to_position_fen(turn, &castling, en_passant);

        Self {
            id: Uuid::new_v4(),
            board,
            turn,
            castling,
            en_passant,
            halfmove_clock: 0,
            fullmove_number: 1,
            position_history: vec![initial_fen],
            move_history: Vec::new(),
            result: None,
            end_reason: None,
            draw_offered_by: None,
            start_timestamp: storage::unix_timestamp(),
            end_timestamp: 0,
        }
    }

    /// Creates a game with a specific ID and timestamps (used for replay).
    pub fn new_with_id_and_timestamps(id: Uuid, start_ts: u64, end_ts: u64) -> Self {
        let mut game = Self::new();
        game.id = id;
        game.start_timestamp = start_ts;
        game.end_timestamp = end_ts;
        game
    }

    /// Returns `true` if the game has ended (has a result).
    pub fn is_over(&self) -> bool {
        self.result.is_some()
    }

    /// Returns the current game state as a JSON-compatible object
    /// for sending to an AI agent (per AGENT.md Section 5).
    pub fn to_game_state_json(&self) -> GameStateJson {
        GameStateJson {
            board: self.board.to_map(),
            turn: self.turn,
            castling: self.castling,
            en_passant: self.en_passant.map(|sq| sq.to_algebraic()),
            halfmove_clock: self.halfmove_clock,
            fullmove_number: self.fullmove_number,
            position_history: self.position_history.clone(),
        }
    }

    /// Generates all legal moves for the current position.
    pub fn legal_moves(&self) -> Vec<ChessMove> {
        movegen::generate_legal_moves(&self.board, self.turn, &self.castling, self.en_passant)
    }

    /// Processes a move submitted by an agent.
    ///
    /// Validates the move, applies it to the board, updates game state,
    /// and checks for game-ending conditions.
    ///
    /// Returns `Ok(())` on success, or `Err(String)` with a detailed
    /// error message for illegal moves.
    pub fn make_move(&mut self, move_json: &MoveJson) -> Result<(), String> {
        if self.is_over() {
            return Err("Game is already over".to_string());
        }

        // Clear any pending draw offer from the opponent
        // (a draw offer is only valid for one move)
        if self.draw_offered_by == Some(self.turn.opponent()) {
            // The opponent offered a draw but we're making a move instead
            // This implicitly declines the draw offer
        }

        // Find the matching legal move
        let chess_move = movegen::find_matching_legal_move(
            &self.board,
            self.turn,
            &self.castling,
            self.en_passant,
            move_json,
        )?;

        // Record the move
        let record = MoveRecord {
            move_number: self.fullmove_number,
            side: self.turn,
            notation: chess_move.to_string(),
            move_json: move_json.clone(),
        };
        self.move_history.push(record);

        // Determine if this is a pawn move or capture (for halfmove clock)
        let moving_piece = self.board.get(chess_move.from).unwrap();
        let is_pawn_move = moving_piece.kind == PieceKind::Pawn;
        let is_capture = self.board.get(chess_move.to).is_some() || chess_move.is_en_passant;

        // Apply the move to the board
        movegen::apply_move_to_board(&mut self.board, &chess_move, self.turn);

        // Update castling rights
        self.update_castling_rights(&chess_move);

        // Update en passant square
        self.en_passant = None;
        if is_pawn_move {
            let rank_diff = (chess_move.to.rank as i8 - chess_move.from.rank as i8).abs();
            if rank_diff == 2 {
                // Pawn double-stepped — set en passant square
                let ep_rank = (chess_move.from.rank as i8 + self.turn.pawn_direction()) as u8;
                self.en_passant = Some(Square::new(chess_move.from.file, ep_rank));
            }
        }

        // Update halfmove clock
        if is_pawn_move || is_capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }

        // Switch turns
        self.turn = self.turn.opponent();

        // Increment fullmove number after Black's move
        if self.turn == Color::White {
            self.fullmove_number += 1;
        }

        // Record position for repetition detection
        let fen = self
            .board
            .to_position_fen(self.turn, &self.castling, self.en_passant);
        self.position_history.push(fen);

        // Clear draw offers when a move is made
        self.draw_offered_by = None;

        // Check for automatic game-ending conditions
        self.check_game_end_conditions();

        // Set end timestamp if game just ended
        if self.is_over() && self.end_timestamp == 0 {
            self.end_timestamp = storage::unix_timestamp();
        }

        Ok(())
    }

    /// Updates castling rights after a move.
    fn update_castling_rights(&mut self, mv: &ChessMove) {
        // King move — lose all castling rights for that side
        if let Some(piece) = self.board.get(mv.to)
            && piece.kind == PieceKind::King
        {
            let rights = self.castling.for_color_mut(piece.color);
            rights.kingside = false;
            rights.queenside = false;
        }

        // Check if a rook moved from or was captured on its starting square
        let check_rook_square = |sq: Square, castling: &mut CastlingRights| {
            // White rooks
            if sq == Square::new(7, 0) {
                castling.white.kingside = false;
            }
            if sq == Square::new(0, 0) {
                castling.white.queenside = false;
            }
            // Black rooks
            if sq == Square::new(7, 7) {
                castling.black.kingside = false;
            }
            if sq == Square::new(0, 7) {
                castling.black.queenside = false;
            }
        };

        check_rook_square(mv.from, &mut self.castling);
        check_rook_square(mv.to, &mut self.castling);
    }

    /// Checks for automatic game-ending conditions after a move.
    fn check_game_end_conditions(&mut self) {
        let legal_moves = self.legal_moves();

        // No legal moves — checkmate or stalemate
        if legal_moves.is_empty() {
            if movegen::is_in_check(&self.board, self.turn) {
                // Checkmate — the side that just moved wins
                self.result = Some(match self.turn {
                    Color::White => GameResult::BlackWins,
                    Color::Black => GameResult::WhiteWins,
                });
                self.end_reason = Some(GameEndReason::Checkmate);
            } else {
                // Stalemate
                self.result = Some(GameResult::Draw);
                self.end_reason = Some(GameEndReason::Stalemate);
            }
            return;
        }

        // Insufficient material
        if movegen::is_insufficient_material(&self.board) {
            self.result = Some(GameResult::Draw);
            self.end_reason = Some(GameEndReason::InsufficientMaterial);
            return;
        }

        // Fivefold repetition (automatic draw, no claim needed)
        if self.count_position_repetitions() >= 5 {
            self.result = Some(GameResult::Draw);
            self.end_reason = Some(GameEndReason::FivefoldRepetition);
            return;
        }

        // 75-move rule (automatic draw, no claim needed)
        // 150 halfmoves = 75 full moves by each side
        if self.halfmove_clock >= 150 {
            self.result = Some(GameResult::Draw);
            self.end_reason = Some(GameEndReason::SeventyFiveMoveRule);
        }
    }

    /// Counts how many times the current position has occurred.
    fn count_position_repetitions(&self) -> usize {
        if let Some(current) = self.position_history.last() {
            self.position_history
                .iter()
                .filter(|p| *p == current)
                .count()
        } else {
            0
        }
    }

    /// Processes a special action (draw claim, draw offer, resignation).
    ///
    /// Returns `Ok(())` on success, or `Err(String)` if the action is invalid.
    pub fn process_action(&mut self, action: &ActionJson) -> Result<(), String> {
        if self.is_over() {
            return Err(t!("game.already_over").to_string());
        }

        match action.action.as_str() {
            "resign" => {
                self.result = Some(match self.turn {
                    Color::White => GameResult::BlackWins,
                    Color::Black => GameResult::WhiteWins,
                });
                self.end_reason = Some(GameEndReason::Resignation);
                self.end_timestamp = storage::unix_timestamp();
                Ok(())
            }

            "offer_draw" => {
                self.draw_offered_by = Some(self.turn);
                Ok(())
            }

            "accept_draw" => {
                if self.draw_offered_by == Some(self.turn.opponent()) {
                    self.result = Some(GameResult::Draw);
                    self.end_reason = Some(GameEndReason::DrawAgreement);
                    self.end_timestamp = storage::unix_timestamp();
                    Ok(())
                } else {
                    Err(t!("game.no_draw_offer").to_string())
                }
            }

            "claim_draw" => {
                let reason = action.reason.as_deref().unwrap_or("");
                match reason {
                    "threefold_repetition" => {
                        if self.count_position_repetitions() >= 3 {
                            self.result = Some(GameResult::Draw);
                            self.end_reason = Some(GameEndReason::ThreefoldRepetition);
                            self.end_timestamp = storage::unix_timestamp();
                            Ok(())
                        } else {
                            Err(t!("game.no_threefold").to_string())
                        }
                    }
                    "fifty_move_rule" => {
                        if self.halfmove_clock >= 100 {
                            self.result = Some(GameResult::Draw);
                            self.end_reason = Some(GameEndReason::FiftyMoveRule);
                            self.end_timestamp = storage::unix_timestamp();
                            Ok(())
                        } else {
                            Err(t!("game.no_fifty_move", clock = self.halfmove_clock).to_string())
                        }
                    }
                    _ => Err(t!("game.invalid_draw_reason", reason = reason).to_string()),
                }
            }

            _ => Err(t!("game.unknown_action", action = &action.action).to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Game manager (multi-game support)
// ---------------------------------------------------------------------------

/// Manages multiple concurrent chess games with persistent storage.
///
/// This is the central store used by the REST API to create, retrieve,
/// and update games. Thread-safe access is handled at the API layer
/// using `Arc<Mutex<GameManager>>`.
///
/// Games are automatically persisted after each state change:
/// - Active games are saved uncompressed for crash recovery.
/// - Completed games are compressed with zstd and moved to the archive.
pub struct GameManager {
    /// Map of game ID to game state.
    pub games: HashMap<Uuid, Game>,
    /// Persistent storage backend.
    pub storage: GameStorage,
}

impl GameManager {
    /// Creates a new game manager with persistent storage at the given path.
    ///
    /// On startup, loads any previously active games from disk.
    pub fn new(storage_path: &str) -> Self {
        let storage = GameStorage::new(storage_path).expect("Failed to initialize game storage");

        let mut manager = Self {
            games: HashMap::new(),
            storage,
        };

        // Restore active games from disk
        manager.restore_active_games();

        manager
    }

    /// Restores any previously persisted active games from disk.
    fn restore_active_games(&mut self) {
        match self.storage.list_active_on_disk() {
            Ok(ids) => {
                for id in ids {
                    match self.storage.load_active(&id) {
                        Ok(archive) => match archive.replay_full() {
                            Ok(game) => {
                                log::info!(
                                    "Restored active game {} ({} moves)",
                                    id,
                                    game.move_history.len()
                                );
                                self.games.insert(id, game);
                            }
                            Err(e) => log::warn!("Failed to replay game {}: {}", id, e),
                        },
                        Err(e) => log::warn!("Failed to load active game {}: {}", id, e),
                    }
                }
                if !self.games.is_empty() {
                    log::info!("Restored {} active game(s) from disk", self.games.len());
                }
            }
            Err(e) => log::warn!("Failed to list active games: {}", e),
        }
    }

    /// Creates a new game, persists it, and returns its ID.
    pub fn create_game(&mut self) -> Uuid {
        let game = Game::new();
        let id = game.id;

        // Persist the new game immediately
        if let Err(e) = self.storage.save_active(&game) {
            log::error!("Failed to persist new game {}: {}", id, e);
        }

        self.games.insert(id, game);
        id
    }

    /// Returns an immutable reference to a game, if it exists.
    pub fn get_game(&self, id: &Uuid) -> Option<&Game> {
        self.games.get(id)
    }

    /// Returns a mutable reference to a game, if it exists.
    pub fn get_game_mut(&mut self, id: &Uuid) -> Option<&mut Game> {
        self.games.get_mut(id)
    }

    /// Persists the current state of a game to disk.
    ///
    /// If the game is over, it is archived (compressed) and removed
    /// from the active directory. Should be called after every move
    /// or action that changes game state.
    pub fn persist_game(&self, game_id: &Uuid) {
        if let Some(game) = self.games.get(game_id) {
            if game.is_over() {
                // Archive completed game (compress + move to archive/)
                match self.storage.archive_game(game) {
                    Ok(size) => log::info!("Game {} archived ({} bytes compressed)", game_id, size),
                    Err(e) => log::error!("Failed to archive game {}: {}", game_id, e),
                }
            } else {
                // Save active game (uncompressed for crash recovery)
                if let Err(e) = self.storage.save_active(game) {
                    log::error!("Failed to persist game {}: {}", game_id, e);
                }
            }
        }
    }

    /// Returns all game IDs.
    pub fn list_game_ids(&self) -> Vec<Uuid> {
        self.games.keys().cloned().collect()
    }

    /// Deletes a game and removes its storage file.
    pub fn delete_game(&mut self, id: &Uuid) -> bool {
        if self.games.remove(id).is_some() {
            // Clean up storage files
            let _ = self.storage.remove_active(id);
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// API response/request types
// ---------------------------------------------------------------------------

/// Response returned when a new game is created.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateGameResponse {
    /// The unique identifier for the newly created game.
    pub game_id: String,
    /// A message confirming creation.
    pub message: String,
}

/// Response containing information about a game.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GameInfoResponse {
    /// The game's unique identifier.
    pub game_id: String,
    /// The current game state for the agent.
    pub state: GameStateJson,
    /// Whether the game is still in progress.
    pub is_over: bool,
    /// The game result, if the game has ended.
    pub result: Option<GameResult>,
    /// The reason the game ended, if applicable.
    pub end_reason: Option<GameEndReason>,
    /// Whether the current side to move is in check.
    pub is_check: bool,
    /// Number of legal moves available to the side to move.
    pub legal_move_count: usize,
    /// History of all moves made in the game.
    pub move_history: Vec<MoveRecord>,
}

/// Response after processing an agent's move or action.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MoveResponse {
    /// Whether the move/action was accepted.
    pub success: bool,
    /// A descriptive message about the result.
    pub message: String,
    /// The current game state (after the move, if successful).
    pub state: GameStateJson,
    /// Whether the game is still in progress.
    pub is_over: bool,
    /// The game result, if the game has ended.
    pub result: Option<GameResult>,
    /// The reason the game ended, if applicable.
    pub end_reason: Option<GameEndReason>,
    /// Whether the current side to move is in check.
    pub is_check: bool,
}

/// A list of available games.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GameListResponse {
    /// List of game summaries.
    pub games: Vec<GameSummary>,
    /// Total number of games.
    pub total: usize,
}

/// Summary information about a single game.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GameSummary {
    /// The game's unique identifier.
    pub game_id: String,
    /// Side to move ("white" or "black").
    pub turn: Color,
    /// The current full-move number.
    pub fullmove_number: u32,
    /// Whether the game has ended.
    pub is_over: bool,
    /// The game result, if ended.
    pub result: Option<GameResult>,
}

/// Error response for the API.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// Error message describing what went wrong.
    pub error: String,
}

/// Request body for submitting a move (wraps MoveJson).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitMoveRequest {
    /// Starting square of the piece (e.g. "e2").
    pub from: String,
    /// Target square of the piece (e.g. "e4").
    pub to: String,
    /// For pawn promotion: "Q", "R", "B", or "N". Otherwise null.
    pub promotion: Option<String>,
}

/// Request body for submitting a special action.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitActionRequest {
    /// Action type: "claim_draw", "offer_draw", "accept_draw", or "resign".
    pub action: String,
    /// Reason for draw claim: "threefold_repetition" or "fifty_move_rule".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Response listing all legal moves from the current position.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LegalMovesResponse {
    /// The current side to move.
    pub turn: Color,
    /// List of legal moves in the JSON protocol format.
    pub moves: Vec<MoveJson>,
    /// Total number of legal moves.
    pub count: usize,
}
