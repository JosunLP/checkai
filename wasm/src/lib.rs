//! CheckAI WASM — Chess engine compiled to WebAssembly.
//!
//! Exposes the full CheckAI chess engine for use from JavaScript / Node.js.
//! This crate shares the core engine source files from the parent checkai
//! crate via `#[path]` directives, ensuring zero divergence.
//!
//! ## Features
//!
//! - **Position analysis**: legal moves, evaluation, search (alpha-beta + PVS)
//! - **Game management**: create games, make moves, handle actions (resign, draw)
//! - **Export**: PGN, JSON, text formatting of game history
//! - **Board display**: ASCII board rendering

// Shim for the rust-i18n `t!` macro. The WASM crate does not bundle
// locale files; this returns the i18n key literal so functions that
// use `t!()` compile without pulling in the full i18n stack.
// The WASM API never calls those functions (they're in the API layer).
macro_rules! t {
    ($key:expr $(, $name:ident = $val:expr)* $(,)?) => {
        $key
    };
}

// Re-use source files from the parent crate (zero code duplication
// for the pure-computation modules).
#[path = "../../src/types.rs"]
pub mod types;

#[path = "../../src/movegen.rs"]
pub mod movegen;

#[path = "../../src/eval.rs"]
pub mod eval;

#[path = "../../src/zobrist.rs"]
pub mod zobrist;

#[path = "../../src/polyglot_keys.rs"]
pub mod polyglot_keys;

// search.rs uses std::time::Instant which panics on wasm32-unknown-unknown.
// This local copy replaces it with web_time::Instant.
pub mod search;

use serde::Serialize;
use wasm_bindgen::prelude::*;

use std::collections::HashMap;
use std::sync::Mutex;
use types::*;

// ---------------------------------------------------------------------------
// Initialisation
// ---------------------------------------------------------------------------

/// Initialise the WASM module (sets up console logging).
/// Call once before using any other function.
#[wasm_bindgen(js_name = "init")]
pub fn init() {
    console_log::init_with_level(log::Level::Warn).ok();
}

// ---------------------------------------------------------------------------
// Internal game state (mirrored from game.rs but without storage dependency)
// ---------------------------------------------------------------------------

/// A complete chess game with full state and history tracking.
/// This is the WASM-side equivalent of game.rs — no file I/O or storage.
struct WasmGame {
    id: String,
    board: Board,
    turn: Color,
    castling: CastlingRights,
    en_passant: Option<Square>,
    halfmove_clock: u32,
    fullmove_number: u32,
    position_history: Vec<String>,
    move_history: Vec<WasmMoveRecord>,
    result: Option<String>,
    end_reason: Option<String>,
    draw_offered_by: Option<Color>,
    start_timestamp: f64,
    end_timestamp: f64,
}

#[derive(Clone, Serialize)]
struct WasmMoveRecord {
    move_number: u32,
    side: String,
    notation: String,
    from: String,
    to: String,
    promotion: Option<String>,
}

impl WasmGame {
    fn new(id: String) -> Self {
        let board = Board::starting_position();
        let castling = CastlingRights::default();
        let turn = Color::White;
        let initial_fen = board.to_position_fen(turn, &castling, None);
        let now = js_sys::Date::now();

        Self {
            id,
            board,
            turn,
            castling,
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            position_history: vec![initial_fen],
            move_history: Vec::new(),
            result: None,
            end_reason: None,
            draw_offered_by: None,
            start_timestamp: now,
            end_timestamp: 0.0,
        }
    }

    fn from_fen(id: String, fen: &str) -> Result<Self, String> {
        let p = parse_fen(fen)?;
        let initial_fen = p.board.to_position_fen(p.turn, &p.castling, p.en_passant);
        let now = js_sys::Date::now();

        Ok(Self {
            id,
            board: p.board,
            turn: p.turn,
            castling: p.castling,
            en_passant: p.en_passant,
            halfmove_clock: p.halfmove_clock,
            fullmove_number: p.fullmove_number,
            position_history: vec![initial_fen],
            move_history: Vec::new(),
            result: None,
            end_reason: None,
            draw_offered_by: None,
            start_timestamp: now,
            end_timestamp: 0.0,
        })
    }

    fn is_over(&self) -> bool {
        self.result.is_some()
    }

    fn to_fen(&self) -> String {
        let pos_fen = self.board.to_position_fen(self.turn, &self.castling, self.en_passant);
        format!("{pos_fen} {} {}", self.halfmove_clock, self.fullmove_number)
    }

    fn legal_moves(&self) -> Vec<ChessMove> {
        movegen::generate_legal_moves(&self.board, self.turn, &self.castling, self.en_passant)
    }

    fn make_move(&mut self, from_str: &str, to_str: &str, promotion: Option<&str>) -> Result<(), String> {
        if self.is_over() {
            return Err("Game is already over".into());
        }

        let from = Square::from_algebraic(from_str)
            .ok_or_else(|| format!("Invalid from square: '{from_str}'"))?;
        let to = Square::from_algebraic(to_str)
            .ok_or_else(|| format!("Invalid to square: '{to_str}'"))?;
        let promo = match promotion {
            Some("Q" | "q") => Some(PieceKind::Queen),
            Some("R" | "r") => Some(PieceKind::Rook),
            Some("B" | "b") => Some(PieceKind::Bishop),
            Some("N" | "n") => Some(PieceKind::Knight),
            None => None,
            Some(p) => return Err(format!("Invalid promotion piece: '{p}'")),
        };

        let legal = self.legal_moves();
        let matched = legal.iter().find(|m| {
            m.from == from && m.to == to && m.promotion == promo
        }).ok_or("Illegal move")?;

        let mover = self.turn;
        let chess_move = *matched;

        // Record the move
        self.move_history.push(WasmMoveRecord {
            move_number: self.fullmove_number,
            side: format!("{:?}", self.turn),
            notation: chess_move.to_string(),
            from: from_str.to_string(),
            to: to_str.to_string(),
            promotion: promotion.map(|s| s.to_uppercase()),
        });

        // Determine if pawn move or capture
        let moving_piece = self.board.get(chess_move.from).unwrap();
        let is_pawn_move = moving_piece.kind == PieceKind::Pawn;
        let is_capture = self.board.get(chess_move.to).is_some() || chess_move.is_en_passant;

        // Apply move to board
        movegen::apply_move_to_board(&mut self.board, &chess_move, self.turn);

        // Update castling rights
        if moving_piece.kind == PieceKind::King {
            let rights = self.castling.for_color_mut(self.turn);
            rights.kingside = false;
            rights.queenside = false;
        }
        fn check_rook_sq(sq: Square, castling: &mut CastlingRights) {
            if sq == Square::new(7, 0) { castling.white.kingside = false; }
            if sq == Square::new(0, 0) { castling.white.queenside = false; }
            if sq == Square::new(7, 7) { castling.black.kingside = false; }
            if sq == Square::new(0, 7) { castling.black.queenside = false; }
        }
        check_rook_sq(chess_move.from, &mut self.castling);
        check_rook_sq(chess_move.to, &mut self.castling);

        // Update en passant
        self.en_passant = None;
        if is_pawn_move {
            let rank_diff = (chess_move.to.rank as i8 - chess_move.from.rank as i8).abs();
            if rank_diff == 2 {
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
        if self.turn == Color::White {
            self.fullmove_number += 1;
        }

        // Record position
        let fen = self.board.to_position_fen(self.turn, &self.castling, self.en_passant);
        self.position_history.push(fen);

        // Draw offer handling
        if self.draw_offered_by != Some(mover) {
            self.draw_offered_by = None;
        }

        // Check game-ending conditions
        self.check_game_end();

        Ok(())
    }

    fn check_game_end(&mut self) {
        let legal = self.legal_moves();

        if legal.is_empty() {
            if movegen::is_in_check(&self.board, self.turn) {
                self.result = Some(match self.turn {
                    Color::White => "BlackWins".into(),
                    Color::Black => "WhiteWins".into(),
                });
                self.end_reason = Some("Checkmate".into());
            } else {
                self.result = Some("Draw".into());
                self.end_reason = Some("Stalemate".into());
            }
            self.end_timestamp = js_sys::Date::now();
            return;
        }

        if movegen::is_insufficient_material(&self.board) {
            self.result = Some("Draw".into());
            self.end_reason = Some("InsufficientMaterial".into());
            self.end_timestamp = js_sys::Date::now();
            return;
        }

        // Fivefold repetition
        if let Some(current) = self.position_history.last() {
            let count = self.position_history.iter().filter(|p| *p == current).count();
            if count >= 5 {
                self.result = Some("Draw".into());
                self.end_reason = Some("FivefoldRepetition".into());
                self.end_timestamp = js_sys::Date::now();
                return;
            }
        }

        // 75-move rule
        if self.halfmove_clock >= 150 {
            self.result = Some("Draw".into());
            self.end_reason = Some("SeventyFiveMoveRule".into());
            self.end_timestamp = js_sys::Date::now();
        }
    }

    fn process_action(&mut self, action: &str, reason: Option<&str>) -> Result<(), String> {
        if self.is_over() {
            return Err("Game is already over".into());
        }

        match action {
            "resign" => {
                self.result = Some(match self.turn {
                    Color::White => "BlackWins".into(),
                    Color::Black => "WhiteWins".into(),
                });
                self.end_reason = Some("Resignation".into());
                self.end_timestamp = js_sys::Date::now();
                Ok(())
            }
            "offer_draw" => {
                self.draw_offered_by = Some(self.turn);
                Ok(())
            }
            "accept_draw" => {
                if self.draw_offered_by == Some(self.turn.opponent()) {
                    self.result = Some("Draw".into());
                    self.end_reason = Some("DrawAgreement".into());
                    self.end_timestamp = js_sys::Date::now();
                    Ok(())
                } else {
                    Err("No draw offer to accept".into())
                }
            }
            "claim_draw" => {
                match reason.unwrap_or("") {
                    "threefold_repetition" => {
                        if let Some(current) = self.position_history.last() {
                            let count = self.position_history.iter().filter(|p| *p == current).count();
                            if count >= 3 {
                                self.result = Some("Draw".into());
                                self.end_reason = Some("ThreefoldRepetition".into());
                                self.end_timestamp = js_sys::Date::now();
                                return Ok(());
                            }
                        }
                        Err("Threefold repetition condition not met".into())
                    }
                    "fifty_move_rule" => {
                        if self.halfmove_clock >= 100 {
                            self.result = Some("Draw".into());
                            self.end_reason = Some("FiftyMoveRule".into());
                            self.end_timestamp = js_sys::Date::now();
                            Ok(())
                        } else {
                            Err(format!("50-move rule not met (halfmove clock: {})", self.halfmove_clock))
                        }
                    }
                    r => Err(format!("Invalid draw reason: '{r}'")),
                }
            }
            _ => Err(format!("Unknown action: '{action}'")),
        }
    }

    fn to_state_json(&self) -> GameStateResponse {
        let is_check = movegen::is_in_check(&self.board, self.turn);
        let legal = self.legal_moves();

        GameStateResponse {
            game_id: self.id.clone(),
            fen: self.to_fen(),
            turn: format!("{:?}", self.turn),
            is_over: self.is_over(),
            result: self.result.clone(),
            end_reason: self.end_reason.clone(),
            is_check,
            legal_move_count: legal.len(),
            halfmove_clock: self.halfmove_clock,
            fullmove_number: self.fullmove_number,
            move_history: self.move_history.clone(),
            board: self.board.to_map(),
            draw_offered_by: self.draw_offered_by.map(|c| format!("{:?}", c)),
        }
    }

    /// Export as PGN string.
    fn to_pgn(&self) -> String {
        let mut out = String::new();
        out.push_str("[Event \"CheckAI Game\"]\n");
        out.push_str("[Site \"CheckAI WASM\"]\n");
        out.push_str("[Date \"????.??.??\"]\n");
        out.push_str("[Round \"1\"]\n");
        out.push_str("[White \"Player White\"]\n");
        out.push_str("[Black \"Player Black\"]\n");

        let result_str = match self.result.as_deref() {
            Some("WhiteWins") => "1-0",
            Some("BlackWins") => "0-1",
            Some("Draw") => "1/2-1/2",
            _ => "*",
        };
        out.push_str(&format!("[Result \"{}\"]\n", result_str));
        out.push_str(&format!("[GameId \"{}\"]\n", self.id));
        if let Some(reason) = &self.end_reason {
            out.push_str(&format!("[Termination \"{}\"]\n", reason));
        }
        out.push('\n');

        // Move text
        let mut text = String::new();
        for (i, rec) in self.move_history.iter().enumerate() {
            if i % 2 == 0 {
                if !text.is_empty() { text.push(' '); }
                text.push_str(&format!("{}.", i / 2 + 1));
            }
            text.push(' ');
            text.push_str(&rec.from);
            text.push_str(&rec.to);
            if let Some(p) = &rec.promotion {
                text.push_str(p);
            }
        }
        if !text.is_empty() { text.push(' '); }
        text.push_str(result_str);

        // Wrap at 80 cols
        let mut line_len = 0;
        for word in text.split_whitespace() {
            if line_len > 0 && line_len + 1 + word.len() > 80 {
                out.push('\n');
                line_len = 0;
            }
            if line_len > 0 { out.push(' '); line_len += 1; }
            out.push_str(word);
            line_len += word.len();
        }
        out.push('\n');
        out
    }

    /// Export as JSON string.
    fn to_export_json(&self) -> String {
        let moves: Vec<serde_json::Value> = self.move_history.iter().enumerate().map(|(i, rec)| {
            serde_json::json!({
                "half_move": i + 1,
                "move_number": i / 2 + 1,
                "side": rec.side,
                "from": rec.from,
                "to": rec.to,
                "promotion": rec.promotion,
                "notation": rec.notation,
            })
        }).collect();

        let export = serde_json::json!({
            "game_id": self.id,
            "result": self.result,
            "end_reason": self.end_reason,
            "move_count": self.move_history.len(),
            "fullmove_count": self.move_history.len().div_ceil(2),
            "moves": moves,
            "final_position": self.board.to_map(),
            "final_fen": self.to_fen(),
            "final_turn": format!("{:?}", self.turn),
        });
        serde_json::to_string_pretty(&export).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Game store (in-memory, thread-safe via Mutex)
// ---------------------------------------------------------------------------

static GAME_STORE: Mutex<Option<HashMap<String, WasmGame>>> = Mutex::new(None);

fn with_store<F, R>(f: F) -> R
where F: FnOnce(&mut HashMap<String, WasmGame>) -> R
{
    let mut guard = GAME_STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = guard.get_or_insert_with(HashMap::new);
    f(store)
}

// ---------------------------------------------------------------------------
// FEN parser (standalone — no dependency on game.rs / storage.rs)
// ---------------------------------------------------------------------------

/// Parsed position data from a FEN string.
struct ParsedFen {
    board: Board,
    turn: Color,
    castling: CastlingRights,
    en_passant: Option<Square>,
    halfmove_clock: u32,
    fullmove_number: u32,
}

fn parse_fen(fen: &str) -> Result<ParsedFen, String> {
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.len() < 4 {
        return Err("FEN must have at least 4 fields".into());
    }

    // --- piece placement ---
    let mut board = Board::default();
    let rows: Vec<&str> = parts[0].split('/').collect();
    if rows.len() != 8 {
        return Err("FEN piece placement must have exactly 8 ranks".into());
    }
    for (row_idx, row) in rows.iter().enumerate() {
        let rank = 7 - row_idx as u8;
        let mut file: u8 = 0;
        for ch in row.chars() {
            if ch.is_ascii_digit() {
                file += ch.to_digit(10).unwrap() as u8;
            } else {
                if file >= 8 {
                    return Err(format!("Too many pieces on rank {}", rank + 1));
                }
                let piece = Piece::from_fen_char(ch)
                    .ok_or_else(|| format!("Invalid piece '{ch}'"))?;
                board.set(Square::new(file, rank), Some(piece));
                file += 1;
            }
        }
        if file != 8 {
            return Err(format!("Rank {} has {file} files, expected 8", rank + 1));
        }
    }

    // --- turn ---
    let turn = match parts[1] {
        "w" => Color::White,
        "b" => Color::Black,
        other => return Err(format!("Invalid turn field: '{other}'")),
    };

    // --- castling ---
    let mut castling = CastlingRights {
        white: SideCastlingRights { kingside: false, queenside: false },
        black: SideCastlingRights { kingside: false, queenside: false },
    };
    if parts[2] != "-" {
        for ch in parts[2].chars() {
            match ch {
                'K' => castling.white.kingside = true,
                'Q' => castling.white.queenside = true,
                'k' => castling.black.kingside = true,
                'q' => castling.black.queenside = true,
                _ => return Err(format!("Invalid castling character: '{ch}'")),
            }
        }
    }

    // --- en passant ---
    let en_passant = if parts[3] == "-" {
        None
    } else {
        Some(Square::from_algebraic(parts[3])
            .ok_or_else(|| format!("Invalid en passant square: '{}'", parts[3]))?)
    };

    // --- halfmove clock ---
    let halfmove_clock = if parts.len() > 4 {
        parts[4].parse::<u32>().map_err(|_| format!("Invalid halfmove clock: '{}'", parts[4]))?
    } else {
        0
    };

    // --- fullmove number ---
    let fullmove_number = if parts.len() > 5 {
        parts[5].parse::<u32>().map_err(|_| format!("Invalid fullmove number: '{}'", parts[5]))?
    } else {
        1
    };

    Ok(ParsedFen { board, turn, castling, en_passant, halfmove_clock, fullmove_number })
}

/// Build a SearchPosition from a FEN string.
fn fen_to_search_pos(fen: &str) -> Result<search::SearchPosition, String> {
    let p = parse_fen(fen)?;
    Ok(search::SearchPosition::new(
        p.board, p.turn, p.castling, p.en_passant, p.halfmove_clock,
    ))
}

/// Build a full FEN string from a ParsedFen.
fn to_full_fen(p: &ParsedFen) -> String {
    let pos_fen = p.board.to_position_fen(p.turn, &p.castling, p.en_passant);
    format!("{pos_fen} {} {}", p.halfmove_clock, p.fullmove_number)
}

// ---------------------------------------------------------------------------
// JSON response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct MoveJson {
    from: String,
    to: String,
    promotion: Option<String>,
    notation: String,
}

impl From<&ChessMove> for MoveJson {
    fn from(mv: &ChessMove) -> Self {
        Self {
            from: mv.from.to_algebraic(),
            to: mv.to.to_algebraic(),
            promotion: mv.promotion.map(|k| match k {
                PieceKind::Queen => "q",
                PieceKind::Rook => "r",
                PieceKind::Bishop => "b",
                PieceKind::Knight => "n",
                _ => "q",
            }.to_string()),
            notation: mv.to_string(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResultJson {
    best_move: Option<MoveJson>,
    score: i32,
    depth: i32,
    pv: Vec<String>,
    nodes: u64,
    time_ms: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MakeMoveResult {
    fen: String,
    is_check: bool,
    is_checkmate: bool,
    is_stalemate: bool,
    is_insufficient_material: bool,
}

// ---------------------------------------------------------------------------
// Exported WASM functions
// ---------------------------------------------------------------------------

/// Returns the standard starting position FEN.
#[wasm_bindgen(js_name = "startingFen")]
pub fn starting_fen() -> String {
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".into()
}

/// Returns all legal moves for the given FEN position as a JSON array.
#[wasm_bindgen(js_name = "legalMoves")]
pub fn legal_moves(fen: &str) -> Result<JsValue, JsError> {
    let pos = fen_to_search_pos(fen).map_err(|e| JsError::new(&e))?;
    let moves = pos.legal_moves();
    let json: Vec<MoveJson> = moves.iter().map(MoveJson::from).collect();
    serde_wasm_bindgen::to_value(&json).map_err(|e| JsError::new(&e.to_string()))
}

/// Returns the static evaluation of a position (centipawns, positive = side to move is better).
#[wasm_bindgen(js_name = "evaluate")]
pub fn evaluate(fen: &str) -> Result<i32, JsError> {
    let pos = fen_to_search_pos(fen).map_err(|e| JsError::new(&e))?;
    Ok(eval::evaluate(&pos.board, pos.turn))
}

/// Searches for the best move at the given depth.
/// Returns a JSON object with best_move, score, depth, pv, nodes, time_ms.
#[wasm_bindgen(js_name = "bestMove")]
pub fn best_move(fen: &str, depth: i32) -> Result<JsValue, JsError> {
    let pos = fen_to_search_pos(fen).map_err(|e| JsError::new(&e))?;
    let depth = depth.clamp(1, 30);
    let mut engine = search::SearchEngine::new(16); // 16 MB TT for WASM
    let result = engine.search(&pos, depth);

    let json = SearchResultJson {
        best_move: result.best_move.as_ref().map(MoveJson::from),
        score: result.score,
        depth: result.depth,
        pv: result.pv.iter().map(|m| m.to_string()).collect(),
        nodes: result.stats.nodes,
        time_ms: result.time_ms,
    };
    serde_wasm_bindgen::to_value(&json).map_err(|e| JsError::new(&e.to_string()))
}

/// Returns `true` if the side to move is in checkmate.
#[wasm_bindgen(js_name = "isCheckmate")]
pub fn is_checkmate(fen: &str) -> Result<bool, JsError> {
    let pos = fen_to_search_pos(fen).map_err(|e| JsError::new(&e))?;
    Ok(pos.legal_moves().is_empty() && pos.is_in_check())
}

/// Returns `true` if the position is stalemate.
#[wasm_bindgen(js_name = "isStalemate")]
pub fn is_stalemate(fen: &str) -> Result<bool, JsError> {
    let pos = fen_to_search_pos(fen).map_err(|e| JsError::new(&e))?;
    Ok(pos.legal_moves().is_empty() && !pos.is_in_check())
}

/// Returns `true` if there is insufficient material for checkmate.
#[wasm_bindgen(js_name = "isInsufficientMaterial")]
pub fn is_insufficient_material(fen: &str) -> Result<bool, JsError> {
    let p = parse_fen(fen).map_err(|e| JsError::new(&e))?;
    Ok(movegen::is_insufficient_material(&p.board))
}

/// Returns `true` if the side to move is in check.
#[wasm_bindgen(js_name = "isCheck")]
pub fn is_check(fen: &str) -> Result<bool, JsError> {
    let pos = fen_to_search_pos(fen).map_err(|e| JsError::new(&e))?;
    Ok(pos.is_in_check())
}

/// Applies a move (in coordinate notation, e.g. "e2e4" or "e7e8q") to the
/// given FEN and returns a JSON object with the resulting FEN and status flags.
#[wasm_bindgen(js_name = "makeMove")]
pub fn make_move(fen: &str, move_str: &str) -> Result<JsValue, JsError> {
    let p = parse_fen(fen).map_err(|e| JsError::new(&e))?;

    // Parse the move string
    let move_str = move_str.trim().to_lowercase();
    if move_str.len() < 4 || move_str.len() > 5 {
        return Err(JsError::new("Move must be 4-5 characters (e.g. e2e4 or e7e8q)"));
    }
    let from = Square::from_algebraic(&move_str[..2])
        .ok_or_else(|| JsError::new(&format!("Invalid from square: '{}'", &move_str[..2])))?;
    let to = Square::from_algebraic(&move_str[2..4])
        .ok_or_else(|| JsError::new(&format!("Invalid to square: '{}'", &move_str[2..4])))?;
    let promotion = if move_str.len() == 5 {
        match move_str.as_bytes()[4] {
            b'q' => Some(PieceKind::Queen),
            b'r' => Some(PieceKind::Rook),
            b'b' => Some(PieceKind::Bishop),
            b'n' => Some(PieceKind::Knight),
            c => return Err(JsError::new(&format!("Invalid promotion piece: '{}'", c as char))),
        }
    } else {
        None
    };

    // Find the matching legal move
    let legal = movegen::generate_legal_moves(&p.board, p.turn, &p.castling, p.en_passant);
    let matched = legal.iter().find(|m| {
        m.from == from && m.to == to && m.promotion == promotion
    }).ok_or_else(|| JsError::new("Illegal move"))?;

    // Apply the move
    let search_pos = search::SearchPosition::new(
        p.board, p.turn, p.castling, p.en_passant, p.halfmove_clock,
    );
    let new_pos = search_pos.make_move(matched);

    // Determine new fullmove number
    let new_fullmove = if p.turn == Color::Black {
        p.fullmove_number + 1
    } else {
        p.fullmove_number
    };

    let new_parsed = ParsedFen {
        board: new_pos.board.clone(),
        turn: new_pos.turn,
        castling: new_pos.castling,
        en_passant: new_pos.en_passant,
        halfmove_clock: new_pos.halfmove_clock,
        fullmove_number: new_fullmove,
    };

    let new_legal = movegen::generate_legal_moves(
        &new_pos.board, new_pos.turn, &new_pos.castling, new_pos.en_passant,
    );
    let in_check = movegen::is_in_check(&new_pos.board, new_pos.turn);

    let result = MakeMoveResult {
        fen: to_full_fen(&new_parsed),
        is_check: in_check,
        is_checkmate: new_legal.is_empty() && in_check,
        is_stalemate: new_legal.is_empty() && !in_check,
        is_insufficient_material: movegen::is_insufficient_material(&new_pos.board),
    };

    serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
}

// ---------------------------------------------------------------------------
// Game state response type
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameStateResponse {
    game_id: String,
    fen: String,
    turn: String,
    is_over: bool,
    result: Option<String>,
    end_reason: Option<String>,
    is_check: bool,
    legal_move_count: usize,
    halfmove_clock: u32,
    fullmove_number: u32,
    move_history: Vec<WasmMoveRecord>,
    board: HashMap<String, String>,
    draw_offered_by: Option<String>,
}

// ---------------------------------------------------------------------------
// Game management — WASM exports
// ---------------------------------------------------------------------------

/// Create a new game from the starting position. Returns the game ID.
#[wasm_bindgen(js_name = "createGame")]
pub fn create_game() -> String {
    let id = generate_id();
    let game = WasmGame::new(id.clone());
    with_store(|store| store.insert(id.clone(), game));
    id
}

/// Create a new game from a custom FEN. Returns the game ID.
#[wasm_bindgen(js_name = "createGameFromFen")]
pub fn create_game_from_fen(fen: &str) -> Result<String, JsError> {
    let id = generate_id();
    let game = WasmGame::from_fen(id.clone(), fen).map_err(|e| JsError::new(&e))?;
    with_store(|store| store.insert(id.clone(), game));
    Ok(id)
}

/// Get the full state of a game as JSON.
#[wasm_bindgen(js_name = "gameState")]
pub fn game_state(game_id: &str) -> Result<JsValue, JsError> {
    with_store(|store| {
        let game = store.get(game_id).ok_or_else(|| JsError::new("Game not found"))?;
        let state = game.to_state_json();
        serde_wasm_bindgen::to_value(&state).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Submit a move to a game. `promotion` is optional (Q/R/B/N).
#[wasm_bindgen(js_name = "gameSubmitMove")]
pub fn game_submit_move(
    game_id: &str,
    from: &str,
    to: &str,
    promotion: Option<String>,
) -> Result<JsValue, JsError> {
    with_store(|store| {
        let game = store.get_mut(game_id).ok_or_else(|| JsError::new("Game not found"))?;
        game.make_move(from, to, promotion.as_deref())
            .map_err(|e| JsError::new(&e))?;
        let state = game.to_state_json();
        serde_wasm_bindgen::to_value(&state).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Process a game action: "resign", "offer_draw", "accept_draw",
/// or "claim_draw" (with optional reason "threefold_repetition" / "fifty_move_rule").
#[wasm_bindgen(js_name = "gameProcessAction")]
pub fn game_process_action(
    game_id: &str,
    action: &str,
    reason: Option<String>,
) -> Result<JsValue, JsError> {
    with_store(|store| {
        let game = store.get_mut(game_id).ok_or_else(|| JsError::new("Game not found"))?;
        game.process_action(action, reason.as_deref())
            .map_err(|e| JsError::new(&e))?;
        let state = game.to_state_json();
        serde_wasm_bindgen::to_value(&state).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get move history for a game as a JSON array.
#[wasm_bindgen(js_name = "gameMoveHistory")]
pub fn game_move_history(game_id: &str) -> Result<JsValue, JsError> {
    with_store(|store| {
        let game = store.get(game_id).ok_or_else(|| JsError::new("Game not found"))?;
        serde_wasm_bindgen::to_value(&game.move_history)
            .map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get the current FEN string for a game.
#[wasm_bindgen(js_name = "gameFen")]
pub fn game_fen(game_id: &str) -> Result<String, JsError> {
    with_store(|store| {
        let game = store.get(game_id).ok_or_else(|| JsError::new("Game not found"))?;
        Ok(game.to_fen())
    })
}

/// Delete a game from the in-memory store.
#[wasm_bindgen(js_name = "deleteGame")]
pub fn delete_game(game_id: &str) -> Result<(), JsError> {
    with_store(|store| {
        store.remove(game_id)
            .ok_or_else(|| JsError::new("Game not found"))?;
        Ok(())
    })
}

/// List all active game IDs.
#[wasm_bindgen(js_name = "listGames")]
pub fn list_games() -> JsValue {
    with_store(|store| {
        let ids: Vec<&String> = store.keys().collect();
        serde_wasm_bindgen::to_value(&ids).unwrap_or(JsValue::NULL)
    })
}

// ---------------------------------------------------------------------------
// Export — PGN / JSON / Text
// ---------------------------------------------------------------------------

/// Export a game as PGN string.
#[wasm_bindgen(js_name = "gameToPgn")]
pub fn game_to_pgn(game_id: &str) -> Result<String, JsError> {
    with_store(|store| {
        let game = store.get(game_id).ok_or_else(|| JsError::new("Game not found"))?;
        Ok(game.to_pgn())
    })
}

/// Export a game as JSON string.
#[wasm_bindgen(js_name = "gameToJson")]
pub fn game_to_json(game_id: &str) -> Result<String, JsError> {
    with_store(|store| {
        let game = store.get(game_id).ok_or_else(|| JsError::new("Game not found"))?;
        Ok(game.to_export_json())
    })
}

/// Export a game as human-readable text.
#[wasm_bindgen(js_name = "gameToText")]
pub fn game_to_text(game_id: &str) -> Result<String, JsError> {
    with_store(|store| {
        let game = store.get(game_id).ok_or_else(|| JsError::new("Game not found"))?;
        Ok(format_game_text(game))
    })
}

fn format_game_text(game: &WasmGame) -> String {
    let mut out = String::new();
    out.push_str("╔══════════════════════════════════════════╗\n");
    out.push_str("║           C H E C K  A I                ║\n");
    out.push_str("╚══════════════════════════════════════════╝\n\n");

    if let Some(result) = &game.result {
        out.push_str(&format!("Result: {}", result));
        if let Some(reason) = &game.end_reason {
            out.push_str(&format!(" ({})", reason));
        }
        out.push('\n');
    } else {
        out.push_str("In progress\n");
    }
    out.push_str(&format!("Moves: {}\n\n", game.move_history.len()));

    // Move table
    if !game.move_history.is_empty() {
        out.push_str("  #  White        Black\n");
        out.push_str(" ─── ──────────── ────────────\n");
        let mut i = 0;
        while i < game.move_history.len() {
            let num = i / 2 + 1;
            let white = &game.move_history[i];
            let white_str = format!("{}{}", white.from, white.to);
            if i + 1 < game.move_history.len() {
                let black = &game.move_history[i + 1];
                let black_str = format!("{}{}", black.from, black.to);
                out.push_str(&format!(" {:>3}. {:<12} {}\n", num, white_str, black_str));
            } else {
                out.push_str(&format!(" {:>3}. {}\n", num, white_str));
            }
            i += 2;
        }
        out.push('\n');
    }

    // Board diagram
    out.push_str(&board_to_ascii_internal(&game.board, game.turn));

    out
}

// ---------------------------------------------------------------------------
// Board ASCII rendering
// ---------------------------------------------------------------------------

/// Render an ASCII board diagram from a FEN string.
#[wasm_bindgen(js_name = "boardToAscii")]
pub fn board_to_ascii(fen: &str) -> Result<String, JsError> {
    let p = parse_fen(fen).map_err(|e| JsError::new(&e))?;
    Ok(board_to_ascii_internal(&p.board, p.turn))
}

fn board_to_ascii_internal(board: &Board, turn: Color) -> String {
    let mut out = String::new();
    out.push_str("    a   b   c   d   e   f   g   h\n");
    out.push_str("  ┌───┬───┬───┬───┬───┬───┬───┬───┐\n");
    for rank in (0..8u8).rev() {
        out.push_str(&format!("{} │", rank + 1));
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            let ch = match board.get(sq) {
                Some(piece) => format!(" {} ", piece.to_fen_char()),
                None => "   ".to_string(),
            };
            out.push_str(&ch);
            out.push('│');
        }
        out.push_str(&format!(" {}\n", rank + 1));
        if rank > 0 {
            out.push_str("  ├───┼───┼───┼───┼───┼───┼───┼───┤\n");
        }
    }
    out.push_str("  └───┴───┴───┴───┴───┴───┴───┴───┘\n");
    out.push_str("    a   b   c   d   e   f   g   h\n");
    let turn_str = match turn {
        Color::White => "White",
        Color::Black => "Black",
    };
    out.push_str(&format!("\n  {} to move\n", turn_str));
    out
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Simple ID generator using cryptographically secure randomness via globalThis.crypto.
/// Works in both browser and Node.js environments.
fn generate_id() -> String {
    // 16 random bytes => 32 hex characters.
    let mut bytes = [0u8; 16];

    // Try to use globalThis.crypto.getRandomValues (Node+browser compatible).
    let mut filled = false;
    {
        let global = js_sys::global();
        if let Ok(crypto_val) =
            js_sys::Reflect::get(&global, &wasm_bindgen::JsValue::from_str("crypto"))
        {
            if !crypto_val.is_undefined() && !crypto_val.is_null() {
                if let Ok(get_random_values) = js_sys::Reflect::get(
                    &crypto_val,
                    &wasm_bindgen::JsValue::from_str("getRandomValues"),
                ) {
                    if get_random_values.is_function() {
                        let func: js_sys::Function = get_random_values.into();
                        let uint8_array =
                            js_sys::Uint8Array::new_with_length(bytes.len() as u32);
                        let args = js_sys::Array::of1(&uint8_array);
                        if js_sys::Reflect::apply(
                            &func,
                            &crypto_val,
                            &args,
                        )
                        .is_ok()
                        {
                            uint8_array.copy_to(&mut bytes);
                            filled = true;
                        }
                    }
                }
            }
        }
    }

    // Fallback: if secure randomness is unavailable, use Math.random.
    if !filled {
        for b in &mut bytes {
            let r = js_sys::Math::random() * 256.0;
            *b = r.floor() as u8;
        }
    }

    let mut id = String::with_capacity(32);
    for byte in &bytes {
        let high = (byte >> 4) & 0x0f;
        let low = byte & 0x0f;
        id.push(char::from_digit(high as u32, 16).unwrap_or('0'));
        id.push(char::from_digit(low as u32, 16).unwrap_or('0'));
    }
    id
}
