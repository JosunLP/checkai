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

        // Remember who is making the move (before turn switch)
        let mover = self.turn;

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

        // Draw offer handling:
        // - If the MOVER offered a draw, keep it active (opponent can still accept)
        // - If the OPPONENT offered a draw and the mover makes a move instead
        //   of accepting, the offer is implicitly declined and cleared.
        // - If no offer exists, this is a no-op.
        if self.draw_offered_by != Some(mover) {
            self.draw_offered_by = None;
        }

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movegen;

    /// Helper: create a MoveJson from strings.
    fn mv(from: &str, to: &str) -> MoveJson {
        MoveJson {
            from: from.to_string(),
            to: to.to_string(),
            promotion: None,
        }
    }

    /// Helper: create a MoveJson with promotion.
    #[allow(dead_code)]
    fn mv_promo(from: &str, to: &str, promo: &str) -> MoveJson {
        MoveJson {
            from: from.to_string(),
            to: to.to_string(),
            promotion: Some(promo.to_string()),
        }
    }

    // -------------------------------------------------------------------
    // Draw offer persistence tests (Bug Fix)
    // -------------------------------------------------------------------

    #[test]
    fn test_draw_offer_persists_after_offerer_moves() {
        // White offers a draw, then White makes a move.
        // The draw offer should persist so Black can accept.
        let mut game = Game::new();

        // White offers a draw
        let action = ActionJson {
            action: "offer_draw".to_string(),
            reason: None,
        };
        game.process_action(&action).unwrap();
        assert_eq!(game.draw_offered_by, Some(Color::White));

        // White makes a move (e2-e4)
        game.make_move(&mv("e2", "e4")).unwrap();

        // The draw offer should still be active for Black to accept
        assert_eq!(
            game.draw_offered_by,
            Some(Color::White),
            "Draw offer should persist after offerer makes a move"
        );
    }

    #[test]
    fn test_draw_offer_cleared_when_opponent_declines_by_moving() {
        // White offers a draw, White makes a move, then Black makes a
        // move (declining the offer). The offer should be cleared.
        let mut game = Game::new();

        // White offers draw
        game.process_action(&ActionJson {
            action: "offer_draw".to_string(),
            reason: None,
        })
        .unwrap();

        // White makes a move
        game.make_move(&mv("e2", "e4")).unwrap();
        assert_eq!(game.draw_offered_by, Some(Color::White));

        // Black makes a move (declining the draw)
        game.make_move(&mv("e7", "e5")).unwrap();

        // Draw offer should now be cleared
        assert_eq!(
            game.draw_offered_by, None,
            "Draw offer should be cleared after opponent declines by moving"
        );
    }

    #[test]
    fn test_draw_offer_accepted_by_opponent() {
        let mut game = Game::new();

        // White offers draw
        game.process_action(&ActionJson {
            action: "offer_draw".to_string(),
            reason: None,
        })
        .unwrap();

        // White makes a move
        game.make_move(&mv("e2", "e4")).unwrap();

        // Now it's Black's turn — Black accepts the draw
        let accept = ActionJson {
            action: "accept_draw".to_string(),
            reason: None,
        };
        game.process_action(&accept).unwrap();

        assert!(game.is_over());
        assert_eq!(game.result, Some(GameResult::Draw));
        assert_eq!(game.end_reason, Some(GameEndReason::DrawAgreement));
    }

    #[test]
    fn test_accept_draw_fails_without_offer() {
        let mut game = Game::new();

        let accept = ActionJson {
            action: "accept_draw".to_string(),
            reason: None,
        };
        let result = game.process_action(&accept);
        assert!(result.is_err(), "Should fail when no draw offer exists");
    }

    #[test]
    fn test_accept_draw_fails_for_own_offer() {
        // White offers a draw, then tries to accept their own offer
        let mut game = Game::new();

        game.process_action(&ActionJson {
            action: "offer_draw".to_string(),
            reason: None,
        })
        .unwrap();

        // White tries to accept their own offer — should fail
        let accept = ActionJson {
            action: "accept_draw".to_string(),
            reason: None,
        };
        let result = game.process_action(&accept);
        assert!(
            result.is_err(),
            "Should not be able to accept your own draw offer"
        );
    }

    // -------------------------------------------------------------------
    // Resignation tests
    // -------------------------------------------------------------------

    #[test]
    fn test_resignation_white() {
        let mut game = Game::new();
        game.process_action(&ActionJson {
            action: "resign".to_string(),
            reason: None,
        })
        .unwrap();

        assert!(game.is_over());
        assert_eq!(game.result, Some(GameResult::BlackWins));
        assert_eq!(game.end_reason, Some(GameEndReason::Resignation));
    }

    #[test]
    fn test_resignation_black() {
        let mut game = Game::new();
        game.make_move(&mv("e2", "e4")).unwrap(); // White moves
        game.process_action(&ActionJson {
            action: "resign".to_string(),
            reason: None,
        })
        .unwrap();

        assert!(game.is_over());
        assert_eq!(game.result, Some(GameResult::WhiteWins));
        assert_eq!(game.end_reason, Some(GameEndReason::Resignation));
    }

    // -------------------------------------------------------------------
    // Checkmate tests
    // -------------------------------------------------------------------

    #[test]
    fn test_scholars_mate() {
        // Scholar's mate: 1. e4 e5 2. Qh5 Nc6 3. Bc4 Nf6 4. Qxf7#
        let mut game = Game::new();
        game.make_move(&mv("e2", "e4")).unwrap();
        game.make_move(&mv("e7", "e5")).unwrap();
        game.make_move(&mv("d1", "h5")).unwrap();
        game.make_move(&mv("b8", "c6")).unwrap();
        game.make_move(&mv("f1", "c4")).unwrap();
        game.make_move(&mv("g8", "f6")).unwrap();
        game.make_move(&mv("h5", "f7")).unwrap();

        assert!(game.is_over());
        assert_eq!(game.result, Some(GameResult::WhiteWins));
        assert_eq!(game.end_reason, Some(GameEndReason::Checkmate));
    }

    #[test]
    fn test_fools_mate() {
        // Fool's mate: 1. f3 e5 2. g4 Qh4#
        let mut game = Game::new();
        game.make_move(&mv("f2", "f3")).unwrap();
        game.make_move(&mv("e7", "e5")).unwrap();
        game.make_move(&mv("g2", "g4")).unwrap();
        game.make_move(&mv("d8", "h4")).unwrap();

        assert!(game.is_over());
        assert_eq!(game.result, Some(GameResult::BlackWins));
        assert_eq!(game.end_reason, Some(GameEndReason::Checkmate));
    }

    // -------------------------------------------------------------------
    // Stalemate test
    // -------------------------------------------------------------------

    #[test]
    fn test_stalemate_detection() {
        // Classic stalemate: White Ka6, Qb6, Black Ka8
        // Ka8 is not in check, but a7/b8/b7 are all controlled by Qb6/Ka6
        let mut board = Board::default();
        board.set(
            Square::new(0, 5),
            Some(Piece::new(PieceKind::King, Color::White)),
        ); // Ka6
        board.set(
            Square::new(1, 5),
            Some(Piece::new(PieceKind::Queen, Color::White)),
        ); // Qb6
        board.set(
            Square::new(0, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        ); // Ka8

        let no_castling = CastlingRights {
            white: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
            black: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
        };

        // Black to move: Ka8 is not in check, but can't move anywhere
        let legal_moves = movegen::generate_legal_moves(&board, Color::Black, &no_castling, None);
        let in_check = movegen::is_in_check(&board, Color::Black);

        assert!(
            !in_check,
            "Black king should NOT be in check in stalemate position"
        );
        assert!(
            legal_moves.is_empty(),
            "Black should have no legal moves in stalemate position"
        );
    }

    // -------------------------------------------------------------------
    // Castling tests
    // -------------------------------------------------------------------

    #[test]
    fn test_castling_kingside_white() {
        // 1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. O-O
        let mut game = Game::new();
        game.make_move(&mv("e2", "e4")).unwrap();
        game.make_move(&mv("e7", "e5")).unwrap();
        game.make_move(&mv("g1", "f3")).unwrap();
        game.make_move(&mv("b8", "c6")).unwrap();
        game.make_move(&mv("f1", "c4")).unwrap();
        game.make_move(&mv("f8", "c5")).unwrap();

        // White castles kingside
        game.make_move(&mv("e1", "g1")).unwrap();

        // King should be on g1, rook on f1
        assert_eq!(
            game.board.get(Square::new(6, 0)),
            Some(Piece::new(PieceKind::King, Color::White))
        );
        assert_eq!(
            game.board.get(Square::new(5, 0)),
            Some(Piece::new(PieceKind::Rook, Color::White))
        );
        // Original squares should be empty
        assert!(game.board.get(Square::new(4, 0)).is_none());
        assert!(game.board.get(Square::new(7, 0)).is_none());

        // Castling rights should be gone for White
        assert!(!game.castling.white.kingside);
        assert!(!game.castling.white.queenside);
    }

    #[test]
    fn test_castling_blocked_by_check() {
        // Set up: White Ke1, Rh1, Black Ke8, Re8-attacking e1 via Rook on e8?
        // Actually Black Re8 wouldn't attack e1 through the entire board...
        // Better: White Ke1, Rh1; Black Ke7, Bb4 (attacking e1?... no, Bb4 attacks e1? Yes: diagonal b4-c3-d2-e1).
        // Wait, b4 to e1: (1,3) -> (4,0), distance is (3,-3). A bishop on b4 attacks e1 along the diagonal.
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        ); // Ke1
        board.set(
            Square::new(7, 0),
            Some(Piece::new(PieceKind::Rook, Color::White)),
        ); // Rh1
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        ); // Ke8
        board.set(
            Square::new(1, 3),
            Some(Piece::new(PieceKind::Bishop, Color::Black)),
        ); // Bb4

        let castling = CastlingRights {
            white: SideCastlingRights {
                kingside: true,
                queenside: false,
            },
            black: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
        };

        // White king is in check from Bb4, so castling should be impossible
        assert!(movegen::is_in_check(&board, Color::White));

        let moves = movegen::generate_legal_moves(&board, Color::White, &castling, None);
        let castling_moves: Vec<_> = moves.iter().filter(|m| m.is_castling).collect();
        assert!(castling_moves.is_empty(), "Cannot castle while in check");
    }

    #[test]
    fn test_castling_blocked_through_attacked_square() {
        // White Ke1, Rh1; Black Ke8, Rf8 (attacks f1 through the file)
        // Actually f8 to f1 is a clear file only if f2-f7 are empty.
        // Black Rook on f5 attacks f1 if f2,f3,f4 are empty.
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        ); // Ke1
        board.set(
            Square::new(7, 0),
            Some(Piece::new(PieceKind::Rook, Color::White)),
        ); // Rh1
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        ); // Ke8
        board.set(
            Square::new(5, 4),
            Some(Piece::new(PieceKind::Rook, Color::Black)),
        ); // Rf5

        let castling = CastlingRights {
            white: SideCastlingRights {
                kingside: true,
                queenside: false,
            },
            black: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
        };

        // f1 is attacked by Black rook on f5
        assert!(movegen::is_square_attacked(
            &board,
            Square::new(5, 0),
            Color::Black
        ));

        let moves = movegen::generate_legal_moves(&board, Color::White, &castling, None);
        let castling_moves: Vec<_> = moves.iter().filter(|m| m.is_castling).collect();
        assert!(
            castling_moves.is_empty(),
            "Cannot castle through attacked square f1"
        );
    }

    // -------------------------------------------------------------------
    // En passant tests
    // -------------------------------------------------------------------

    #[test]
    fn test_en_passant_capture() {
        // 1. e4 d5 2. e5 f5 3. exf6 (en passant)
        let mut game = Game::new();
        game.make_move(&mv("e2", "e4")).unwrap();
        game.make_move(&mv("d7", "d5")).unwrap();
        game.make_move(&mv("e4", "e5")).unwrap();
        game.make_move(&mv("f7", "f5")).unwrap();

        // After f7-f5, en passant should be f6
        assert_eq!(game.en_passant, Some(Square::new(5, 5))); // f6

        // White captures en passant
        game.make_move(&mv("e5", "f6")).unwrap();

        // Pawn should be on f6, f5 should be empty
        assert_eq!(
            game.board.get(Square::new(5, 5)),
            Some(Piece::new(PieceKind::Pawn, Color::White))
        );
        assert!(
            game.board.get(Square::new(5, 4)).is_none(),
            "Captured pawn on f5 should be removed"
        );
    }

    #[test]
    fn test_en_passant_discovered_check_blocked() {
        // Position: White Ka5, Pe5; Black Rh5, Pd5 (just double-stepped)
        // En passant e5xd6 would remove the black pawn on d5, exposing
        // the white king on a5 to the black rook on h5 along rank 5.
        // This en passant capture must be illegal.
        let mut board = Board::default();
        board.set(
            Square::new(0, 4),
            Some(Piece::new(PieceKind::King, Color::White)),
        ); // Ka5
        board.set(
            Square::new(4, 4),
            Some(Piece::new(PieceKind::Pawn, Color::White)),
        ); // Pe5
        board.set(
            Square::new(7, 4),
            Some(Piece::new(PieceKind::Rook, Color::Black)),
        ); // Rh5
        board.set(
            Square::new(3, 4),
            Some(Piece::new(PieceKind::Pawn, Color::Black)),
        ); // Pd5
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        ); // Ke8

        let castling = CastlingRights {
            white: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
            black: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
        };
        let ep = Some(Square::new(3, 5)); // d6

        let moves = movegen::generate_legal_moves(&board, Color::White, &castling, ep);
        let ep_moves: Vec<_> = moves.iter().filter(|m| m.is_en_passant).collect();

        assert!(
            ep_moves.is_empty(),
            "En passant should be illegal when it exposes own king to discovered check"
        );
    }

    #[test]
    fn test_en_passant_expires_after_one_move() {
        // After a pawn double-steps, en passant is only available for one move.
        let mut game = Game::new();
        game.make_move(&mv("e2", "e4")).unwrap();
        assert!(game.en_passant.is_some()); // e3

        game.make_move(&mv("d7", "d5")).unwrap();
        // d5 double-step sets new en passant, old one expired
        assert_eq!(game.en_passant, Some(Square::new(3, 5))); // d6
        // e3 is no longer available

        game.make_move(&mv("a2", "a3")).unwrap();
        // After a non-pawn-double-step, en passant is cleared
        assert_eq!(game.en_passant, None);
    }

    // -------------------------------------------------------------------
    // Pawn promotion tests
    // -------------------------------------------------------------------

    #[test]
    fn test_pawn_promotion_required() {
        // Set up: White Pawn on e7, White King on a1, Black King on h8
        let mut board = Board::default();
        board.set(
            Square::new(4, 6),
            Some(Piece::new(PieceKind::Pawn, Color::White)),
        ); // Pe7
        board.set(
            Square::new(0, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        ); // Ka1
        board.set(
            Square::new(7, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        ); // Kh8

        let castling = CastlingRights {
            white: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
            black: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
        };

        let moves = movegen::generate_legal_moves(&board, Color::White, &castling, None);
        let pawn_to_e8: Vec<_> = moves
            .iter()
            .filter(|m| m.from == Square::new(4, 6) && m.to == Square::new(4, 7))
            .collect();

        // All moves to e8 must have promotion set
        assert!(
            !pawn_to_e8.is_empty(),
            "Pawn should be able to advance to e8"
        );
        for m in &pawn_to_e8 {
            assert!(
                m.promotion.is_some(),
                "All pawn moves to last rank must have promotion"
            );
        }
        // Should have exactly 4 promotion options (Q, R, B, N)
        assert_eq!(
            pawn_to_e8.len(),
            4,
            "Should generate all 4 promotion options"
        );
    }

    #[test]
    fn test_pawn_promotion_to_queen() {
        let mut board = Board::default();
        board.set(
            Square::new(4, 6),
            Some(Piece::new(PieceKind::Pawn, Color::White)),
        ); // Pe7
        board.set(
            Square::new(0, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        ); // Ka1
        board.set(
            Square::new(7, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        ); // Kh8

        // Apply promotion move directly
        let promo_move = ChessMove {
            from: Square::new(4, 6),
            to: Square::new(4, 7),
            promotion: Some(PieceKind::Queen),
            is_castling: false,
            is_en_passant: false,
        };
        movegen::apply_move_to_board(&mut board, &promo_move, Color::White);

        assert_eq!(
            board.get(Square::new(4, 7)),
            Some(Piece::new(PieceKind::Queen, Color::White)),
            "Pawn should be promoted to queen on e8"
        );
        assert!(
            board.get(Square::new(4, 6)).is_none(),
            "Original pawn square should be empty"
        );
    }

    // -------------------------------------------------------------------
    // Pinned piece tests
    // -------------------------------------------------------------------

    #[test]
    fn test_pinned_piece_cannot_move() {
        // White Ke1, Nd2 (pinned by Black Qa5-e1 diagonal? No.)
        // Better: White Ke1, Bf3 (pinned by Black Rook on f8 along f-file? No, f3 isn't on f-file direction to king).
        // Simple pin: White Ke1, Bd2, Black Bb4 is not a pin.
        // Real pin: White Ke1, Bf2; Black Qb6 (along diagonal b6-c5-d4-e3-f2 — no, that's not to king).
        // Simplest: White Ke1, Re2; Black Re8 (rook on e2 is pinned along e-file)
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        ); // Ke1
        board.set(
            Square::new(4, 1),
            Some(Piece::new(PieceKind::Rook, Color::White)),
        ); // Re2 (pinned)
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::Rook, Color::Black)),
        ); // Re8 (pinning)
        board.set(
            Square::new(0, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        ); // Ka8

        let castling = CastlingRights {
            white: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
            black: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
        };

        let moves = movegen::generate_legal_moves(&board, Color::White, &castling, None);
        let rook_moves: Vec<_> = moves
            .iter()
            .filter(|m| m.from == Square::new(4, 1))
            .collect();

        // The rook on e2 is pinned along the e-file. It CAN move along
        // the e-file (e3, e4, e5, e6, e7, e8 capture) but NOT off the file.
        for m in &rook_moves {
            assert_eq!(
                m.to.file,
                4,
                "Pinned rook on e2 can only move along the e-file, not to {}",
                m.to.to_algebraic()
            );
        }
        assert!(
            !rook_moves.is_empty(),
            "Pinned rook should have some moves along the pin line"
        );
    }

    #[test]
    fn test_pinned_knight_has_no_moves() {
        // A pinned knight has NO legal moves (knights can't move along pin line)
        // White Ke1, Ne2; Black Re8 (knight pinned along e-file)
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        ); // Ke1
        board.set(
            Square::new(4, 1),
            Some(Piece::new(PieceKind::Knight, Color::White)),
        ); // Ne2 (pinned)
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::Rook, Color::Black)),
        ); // Re8
        board.set(
            Square::new(0, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        ); // Ka8

        let castling = CastlingRights {
            white: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
            black: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
        };

        let moves = movegen::generate_legal_moves(&board, Color::White, &castling, None);
        let knight_moves: Vec<_> = moves
            .iter()
            .filter(|m| m.from == Square::new(4, 1))
            .collect();

        assert!(
            knight_moves.is_empty(),
            "Pinned knight should have no legal moves"
        );
    }

    // -------------------------------------------------------------------
    // Halfmove clock tests
    // -------------------------------------------------------------------

    #[test]
    fn test_halfmove_clock_reset_on_pawn_move() {
        let mut game = Game::new();
        // Move a knight first to increase clock
        game.make_move(&mv("g1", "f3")).unwrap();
        assert_eq!(game.halfmove_clock, 1);

        game.make_move(&mv("g8", "f6")).unwrap();
        assert_eq!(game.halfmove_clock, 2);

        // Pawn move resets clock
        game.make_move(&mv("e2", "e4")).unwrap();
        assert_eq!(game.halfmove_clock, 0);
    }

    #[test]
    fn test_halfmove_clock_reset_on_capture() {
        let mut game = Game::new();
        game.make_move(&mv("e2", "e4")).unwrap();
        game.make_move(&mv("d7", "d5")).unwrap();

        // Knight move (non-capture, non-pawn)
        game.make_move(&mv("g1", "f3")).unwrap();
        assert_eq!(game.halfmove_clock, 1);

        game.make_move(&mv("d5", "e4")).unwrap(); // pawn captures
        assert_eq!(
            game.halfmove_clock, 0,
            "Capture should reset halfmove clock"
        );
    }

    // -------------------------------------------------------------------
    // Fullmove number tests
    // -------------------------------------------------------------------

    #[test]
    fn test_fullmove_number_incremented_after_black_move() {
        let mut game = Game::new();
        assert_eq!(game.fullmove_number, 1);

        game.make_move(&mv("e2", "e4")).unwrap();
        assert_eq!(
            game.fullmove_number, 1,
            "Should still be 1 after White's move"
        );

        game.make_move(&mv("e7", "e5")).unwrap();
        assert_eq!(
            game.fullmove_number, 2,
            "Should increment to 2 after Black's move"
        );

        game.make_move(&mv("g1", "f3")).unwrap();
        assert_eq!(game.fullmove_number, 2);

        game.make_move(&mv("b8", "c6")).unwrap();
        assert_eq!(game.fullmove_number, 3);
    }

    // -------------------------------------------------------------------
    // Position history & repetition tests
    // -------------------------------------------------------------------

    #[test]
    fn test_position_history_tracks_initial_position() {
        let game = Game::new();
        assert_eq!(
            game.position_history.len(),
            1,
            "Position history should contain the starting position"
        );
    }

    #[test]
    fn test_position_history_grows_with_moves() {
        let mut game = Game::new();
        game.make_move(&mv("e2", "e4")).unwrap();
        assert_eq!(game.position_history.len(), 2);

        game.make_move(&mv("e7", "e5")).unwrap();
        assert_eq!(game.position_history.len(), 3);
    }

    #[test]
    fn test_threefold_repetition_claim() {
        // Play moves that return to the same position three times:
        // 1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6 4. Ng1 Ng8 (pos repeats)
        // Starting position → after 2. Ng1 Ng8 → same as starting
        let mut game = Game::new();

        // First cycle
        game.make_move(&mv("g1", "f3")).unwrap();
        game.make_move(&mv("g8", "f6")).unwrap();
        game.make_move(&mv("f3", "g1")).unwrap();
        game.make_move(&mv("f6", "g8")).unwrap();
        // Position has repeated 2x (starting + now)

        // Second cycle
        game.make_move(&mv("g1", "f3")).unwrap();
        game.make_move(&mv("g8", "f6")).unwrap();
        game.make_move(&mv("f3", "g1")).unwrap();
        game.make_move(&mv("f6", "g8")).unwrap();
        // Position has repeated 3x

        // White can now claim threefold repetition
        let claim = ActionJson {
            action: "claim_draw".to_string(),
            reason: Some("threefold_repetition".to_string()),
        };
        game.process_action(&claim).unwrap();

        assert!(game.is_over());
        assert_eq!(game.result, Some(GameResult::Draw));
        assert_eq!(game.end_reason, Some(GameEndReason::ThreefoldRepetition));
    }

    #[test]
    fn test_fifty_move_rule_claim() {
        let mut game = Game::new();
        // Set halfmove_clock manually for testing
        game.halfmove_clock = 100;

        let claim = ActionJson {
            action: "claim_draw".to_string(),
            reason: Some("fifty_move_rule".to_string()),
        };
        game.process_action(&claim).unwrap();

        assert!(game.is_over());
        assert_eq!(game.result, Some(GameResult::Draw));
        assert_eq!(game.end_reason, Some(GameEndReason::FiftyMoveRule));
    }

    #[test]
    fn test_fifty_move_rule_claim_fails_too_early() {
        let mut game = Game::new();
        game.halfmove_clock = 99;

        let claim = ActionJson {
            action: "claim_draw".to_string(),
            reason: Some("fifty_move_rule".to_string()),
        };
        let result = game.process_action(&claim);
        assert!(
            result.is_err(),
            "Should not be able to claim 50-move rule with halfmove_clock < 100"
        );
    }

    // -------------------------------------------------------------------
    // Insufficient material tests
    // -------------------------------------------------------------------

    #[test]
    fn test_insufficient_material_k_vs_k_ends_game() {
        // Set up K vs K and make a move that results in this
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        );

        assert!(movegen::is_insufficient_material(&board));
    }

    #[test]
    fn test_insufficient_material_kn_vs_k() {
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(2, 2),
            Some(Piece::new(PieceKind::Knight, Color::White)),
        );
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        );

        assert!(
            movegen::is_insufficient_material(&board),
            "K+N vs K is insufficient material"
        );
    }

    #[test]
    fn test_sufficient_material_knn_vs_k() {
        // Two knights vs king is NOT insufficient — mate is possible with cooperation
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(2, 2),
            Some(Piece::new(PieceKind::Knight, Color::White)),
        );
        board.set(
            Square::new(5, 3),
            Some(Piece::new(PieceKind::Knight, Color::White)),
        );
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        );

        assert!(
            !movegen::is_insufficient_material(&board),
            "K+N+N vs K is NOT insufficient material per AGENT.md"
        );
    }

    #[test]
    fn test_insufficient_material_kb_vs_kb_same_color() {
        // K+B vs K+B with both bishops on same color squares
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(2, 0),
            Some(Piece::new(PieceKind::Bishop, Color::White)),
        ); // c1 (dark: 2+0=2, even)
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        );
        board.set(
            Square::new(5, 6),
            Some(Piece::new(PieceKind::Bishop, Color::Black)),
        ); // f7 (dark: 5+6=11, odd... wait)

        // c1: file=2, rank=0, sum=2 (even)
        // We need same color: c1 (even) and e3 (file=4, rank=2, sum=6 even)
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(2, 0),
            Some(Piece::new(PieceKind::Bishop, Color::White)),
        ); // c1: (2+0)%2 = 0
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        );
        board.set(
            Square::new(4, 2),
            Some(Piece::new(PieceKind::Bishop, Color::Black)),
        ); // e3: (4+2)%2 = 0

        assert!(
            movegen::is_insufficient_material(&board),
            "K+B vs K+B with same-colored bishops is insufficient"
        );
    }

    #[test]
    fn test_sufficient_material_kb_vs_kb_different_color() {
        // K+B vs K+B with bishops on different color squares -> NOT insufficient
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(2, 0),
            Some(Piece::new(PieceKind::Bishop, Color::White)),
        ); // c1: (2+0)%2 = 0
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        );
        board.set(
            Square::new(3, 2),
            Some(Piece::new(PieceKind::Bishop, Color::Black)),
        ); // d3: (3+2)%2 = 1 (different)

        assert!(
            !movegen::is_insufficient_material(&board),
            "K+B vs K+B with different-colored bishops is NOT insufficient"
        );
    }

    // -------------------------------------------------------------------
    // Castling rights update tests
    // -------------------------------------------------------------------

    #[test]
    fn test_castling_rights_lost_after_king_move() {
        let mut game = Game::new();
        game.make_move(&mv("e2", "e4")).unwrap();
        game.make_move(&mv("e7", "e5")).unwrap();
        game.make_move(&mv("e1", "e2")).unwrap(); // King moves

        assert!(!game.castling.white.kingside);
        assert!(!game.castling.white.queenside);
        // Black rights should be unchanged
        assert!(game.castling.black.kingside);
        assert!(game.castling.black.queenside);
    }

    #[test]
    fn test_castling_rights_lost_after_rook_move() {
        let mut game = Game::new();
        game.make_move(&mv("a2", "a4")).unwrap();
        game.make_move(&mv("e7", "e5")).unwrap();
        game.make_move(&mv("a1", "a3")).unwrap(); // a-rook moves

        assert!(
            game.castling.white.kingside,
            "Kingside should be unaffected"
        );
        assert!(
            !game.castling.white.queenside,
            "Queenside should be lost after a-rook moves"
        );
    }

    #[test]
    fn test_castling_rights_lost_when_rook_captured() {
        // Set up position where Black can capture the White h1 rook
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        ); // Ke1
        board.set(
            Square::new(7, 0),
            Some(Piece::new(PieceKind::Rook, Color::White)),
        ); // Rh1
        board.set(
            Square::new(0, 0),
            Some(Piece::new(PieceKind::Rook, Color::White)),
        ); // Ra1
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        ); // Ke8
        board.set(
            Square::new(7, 3),
            Some(Piece::new(PieceKind::Rook, Color::Black)),
        ); // Rh4 — can capture Rh1

        let mut game = Game::new();
        game.board = board;
        game.turn = Color::Black;
        game.castling = CastlingRights {
            white: SideCastlingRights {
                kingside: true,
                queenside: true,
            },
            black: SideCastlingRights {
                kingside: false,
                queenside: false,
            },
        };

        // Black captures Rh1
        game.make_move(&mv("h4", "h1")).unwrap();

        assert!(
            !game.castling.white.kingside,
            "White kingside castling should be lost when h1 rook is captured"
        );
        assert!(
            game.castling.white.queenside,
            "White queenside castling should be unaffected"
        );
    }

    // -------------------------------------------------------------------
    // Game flow tests
    // -------------------------------------------------------------------

    #[test]
    fn test_cannot_move_after_game_over() {
        let mut game = Game::new();
        game.result = Some(GameResult::Draw);
        let result = game.make_move(&mv("e2", "e4"));
        assert!(
            result.is_err(),
            "Should not be able to move after game is over"
        );
    }

    #[test]
    fn test_illegal_move_rejected() {
        let mut game = Game::new();
        // Try to move pawn backwards
        let result = game.make_move(&mv("e2", "e1"));
        assert!(result.is_err(), "Backward pawn move should be rejected");
    }

    #[test]
    fn test_moving_opponent_piece_rejected() {
        let mut game = Game::new();
        // White tries to move Black pawn
        let result = game.make_move(&mv("e7", "e5"));
        assert!(
            result.is_err(),
            "Should not be able to move opponent's piece"
        );
    }
}
