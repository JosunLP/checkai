//! Core types for the CheckAI chess engine.
//!
//! This module defines the fundamental data structures used throughout the
//! chess engine, including piece representation, board state, move encoding,
//! and game state management. All types follow the FIDE 2023 Laws of Chess
//! and use the JSON protocol defined in AGENT.md.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// Piece & Color
// ---------------------------------------------------------------------------

/// Represents the color (side) of a chess piece or player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    /// The White side (starts on ranks 1–2).
    White,
    /// The Black side (starts on ranks 7–8).
    Black,
}

impl Color {
    /// Returns the opposite color.
    pub fn opponent(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    /// Returns the home rank index (0-based) for pawns of this color.
    /// White pawns start on rank 2 (index 1), Black on rank 7 (index 6).
    pub fn pawn_start_rank(self) -> u8 {
        match self {
            Color::White => 1,
            Color::Black => 6,
        }
    }

    /// Returns the promotion rank index (0-based).
    /// White promotes on rank 8 (index 7), Black on rank 1 (index 0).
    pub fn promotion_rank(self) -> u8 {
        match self {
            Color::White => 7,
            Color::Black => 0,
        }
    }

    /// Returns the direction pawns move: +1 for White, -1 for Black.
    pub fn pawn_direction(self) -> i8 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::White => write!(f, "white"),
            Color::Black => write!(f, "black"),
        }
    }
}

/// Represents a chess piece type (without color information).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum PieceKind {
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn,
}

/// A chess piece with both kind and color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Piece {
    pub kind: PieceKind,
    pub color: Color,
}

impl Piece {
    /// Creates a new piece.
    pub fn new(kind: PieceKind, color: Color) -> Self {
        Self { kind, color }
    }

    /// Converts the piece to its FEN character representation.
    /// Uppercase for White, lowercase for Black.
    pub fn to_fen_char(self) -> char {
        let c = match self.kind {
            PieceKind::King => 'K',
            PieceKind::Queen => 'Q',
            PieceKind::Rook => 'R',
            PieceKind::Bishop => 'B',
            PieceKind::Knight => 'N',
            PieceKind::Pawn => 'P',
        };
        match self.color {
            Color::White => c,
            Color::Black => c.to_ascii_lowercase(),
        }
    }

    /// Parses a FEN character into a `Piece`.
    /// Returns `None` if the character is not a valid piece symbol.
    pub fn from_fen_char(c: char) -> Option<Self> {
        let color = if c.is_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        let kind = match c.to_ascii_uppercase() {
            'K' => PieceKind::King,
            'Q' => PieceKind::Queen,
            'R' => PieceKind::Rook,
            'B' => PieceKind::Bishop,
            'N' => PieceKind::Knight,
            'P' => PieceKind::Pawn,
            _ => return None,
        };
        Some(Piece { kind, color })
    }
}

// ---------------------------------------------------------------------------
// Square
// ---------------------------------------------------------------------------

/// Represents a square on the chessboard using 0-based file and rank indices.
///
/// - `file`: 0 (a) to 7 (h)
/// - `rank`: 0 (rank 1) to 7 (rank 8)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Square {
    pub file: u8,
    pub rank: u8,
}

impl Square {
    /// Creates a new square from 0-based file and rank.
    /// Panics if file or rank >= 8.
    pub fn new(file: u8, rank: u8) -> Self {
        debug_assert!(file < 8 && rank < 8, "Square out of bounds");
        Self { file, rank }
    }

    /// Parses an algebraic notation string (e.g. "e4") into a `Square`.
    /// Returns `None` for invalid input.
    pub fn from_algebraic(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let file = bytes[0].wrapping_sub(b'a');
        let rank = bytes[1].wrapping_sub(b'1');
        if file < 8 && rank < 8 {
            Some(Square { file, rank })
        } else {
            None
        }
    }

    /// Converts the square to its algebraic notation string (e.g. "e4").
    pub fn to_algebraic(self) -> String {
        format!("{}{}", (b'a' + self.file) as char, self.rank + 1)
    }

    /// Returns a new square offset by `(df, dr)`, or `None` if out of bounds.
    pub fn offset(self, df: i8, dr: i8) -> Option<Square> {
        let f = self.file as i8 + df;
        let r = self.rank as i8 + dr;
        if (0..8).contains(&f) && (0..8).contains(&r) {
            Some(Square::new(f as u8, r as u8))
        } else {
            None
        }
    }

    /// Returns a flat index (0..63) for the square.
    pub fn index(self) -> usize {
        (self.rank as usize) * 8 + self.file as usize
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_algebraic())
    }
}

// ---------------------------------------------------------------------------
// Castling Rights
// ---------------------------------------------------------------------------

/// Castling rights for one side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct SideCastlingRights {
    /// Whether kingside castling (short castling) is still available.
    pub kingside: bool,
    /// Whether queenside castling (long castling) is still available.
    pub queenside: bool,
}

impl Default for SideCastlingRights {
    fn default() -> Self {
        Self {
            kingside: true,
            queenside: true,
        }
    }
}

/// Castling rights for both sides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct CastlingRights {
    pub white: SideCastlingRights,
    pub black: SideCastlingRights,
}

impl CastlingRights {
    /// Returns the castling rights for the given color.
    pub fn for_color(&self, color: Color) -> &SideCastlingRights {
        match color {
            Color::White => &self.white,
            Color::Black => &self.black,
        }
    }

    /// Returns a mutable reference to the castling rights for the given color.
    pub fn for_color_mut(&mut self, color: Color) -> &mut SideCastlingRights {
        match color {
            Color::White => &mut self.white,
            Color::Black => &mut self.black,
        }
    }

    /// Generates the FEN castling string (e.g. "KQkq" or "-").
    pub fn to_fen(&self) -> String {
        let mut s = String::new();
        if self.white.kingside {
            s.push('K');
        }
        if self.white.queenside {
            s.push('Q');
        }
        if self.black.kingside {
            s.push('k');
        }
        if self.black.queenside {
            s.push('q');
        }
        if s.is_empty() {
            "-".to_string()
        } else {
            s
        }
    }
}

// ---------------------------------------------------------------------------
// Board
// ---------------------------------------------------------------------------

/// Represents the chess board as a flat 64-element array.
///
/// Each element is `Option<Piece>` — `None` means the square is empty.
/// Index mapping: `rank * 8 + file` (both 0-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub squares: [Option<Piece>; 64],
}

impl Default for Board {
    /// Returns an empty board.
    fn default() -> Self {
        Self {
            squares: [None; 64],
        }
    }
}

impl Board {
    /// Returns the piece at the given square, if any.
    pub fn get(&self, sq: Square) -> Option<Piece> {
        self.squares[sq.index()]
    }

    /// Sets (or clears) the piece at the given square.
    pub fn set(&mut self, sq: Square, piece: Option<Piece>) {
        self.squares[sq.index()] = piece;
    }

    /// Creates the standard starting position.
    pub fn starting_position() -> Self {
        let mut board = Board::default();

        // Helper to place a piece
        let mut place = |file: u8, rank: u8, kind: PieceKind, color: Color| {
            board.set(Square::new(file, rank), Some(Piece::new(kind, color)));
        };

        // White pieces (rank 0 = rank 1)
        place(0, 0, PieceKind::Rook, Color::White);
        place(1, 0, PieceKind::Knight, Color::White);
        place(2, 0, PieceKind::Bishop, Color::White);
        place(3, 0, PieceKind::Queen, Color::White);
        place(4, 0, PieceKind::King, Color::White);
        place(5, 0, PieceKind::Bishop, Color::White);
        place(6, 0, PieceKind::Knight, Color::White);
        place(7, 0, PieceKind::Rook, Color::White);

        // White pawns (rank 1 = rank 2)
        for f in 0..8 {
            place(f, 1, PieceKind::Pawn, Color::White);
        }

        // Black pawns (rank 6 = rank 7)
        for f in 0..8 {
            place(f, 6, PieceKind::Pawn, Color::Black);
        }

        // Black pieces (rank 7 = rank 8)
        place(0, 7, PieceKind::Rook, Color::Black);
        place(1, 7, PieceKind::Knight, Color::Black);
        place(2, 7, PieceKind::Bishop, Color::Black);
        place(3, 7, PieceKind::Queen, Color::Black);
        place(4, 7, PieceKind::King, Color::Black);
        place(5, 7, PieceKind::Bishop, Color::Black);
        place(6, 7, PieceKind::Knight, Color::Black);
        place(7, 7, PieceKind::Rook, Color::Black);

        board
    }

    /// Converts the board to the JSON-compatible map format (only occupied squares).
    pub fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for rank in 0..8u8 {
            for file in 0..8u8 {
                let sq = Square::new(file, rank);
                if let Some(piece) = self.get(sq) {
                    map.insert(sq.to_algebraic(), piece.to_fen_char().to_string());
                }
            }
        }
        map
    }

    /// Creates a board from the JSON-compatible map format.
    pub fn from_map(map: &HashMap<String, String>) -> Result<Self, String> {
        let mut board = Board::default();
        for (sq_str, piece_str) in map {
            let sq = Square::from_algebraic(sq_str)
                .ok_or_else(|| format!("Invalid square: {}", sq_str))?;
            let ch = piece_str
                .chars()
                .next()
                .ok_or_else(|| format!("Empty piece string for square {}", sq_str))?;
            let piece = Piece::from_fen_char(ch)
                .ok_or_else(|| format!("Invalid piece symbol '{}' on {}", ch, sq_str))?;
            board.set(sq, Some(piece));
        }
        Ok(board)
    }

    /// Finds the king square for the given color.
    /// Returns `None` if the king is not on the board. (Should never happen in a legal game.)
    pub fn find_king(&self, color: Color) -> Option<Square> {
        for rank in 0..8u8 {
            for file in 0..8u8 {
                let sq = Square::new(file, rank);
                if let Some(piece) = self.get(sq)
                    && piece.kind == PieceKind::King && piece.color == color
                {
                    return Some(sq);
                }
            }
        }
        None
    }

    /// Generates a simplified FEN string for position comparison
    /// (piece placement + side to move + castling + en passant).
    pub fn to_position_fen(&self, turn: Color, castling: &CastlingRights, en_passant: Option<Square>) -> String {
        let mut fen = String::new();
        for rank in (0..8).rev() {
            let mut empty_count = 0;
            for file in 0..8u8 {
                let sq = Square::new(file, rank);
                match self.get(sq) {
                    Some(piece) => {
                        if empty_count > 0 {
                            fen.push_str(&empty_count.to_string());
                            empty_count = 0;
                        }
                        fen.push(piece.to_fen_char());
                    }
                    None => {
                        empty_count += 1;
                    }
                }
            }
            if empty_count > 0 {
                fen.push_str(&empty_count.to_string());
            }
            if rank > 0 {
                fen.push('/');
            }
        }

        fen.push(' ');
        fen.push(match turn {
            Color::White => 'w',
            Color::Black => 'b',
        });

        fen.push(' ');
        fen.push_str(&castling.to_fen());

        fen.push(' ');
        match en_passant {
            Some(sq) => fen.push_str(&sq.to_algebraic()),
            None => fen.push('-'),
        }

        fen
    }
}

// ---------------------------------------------------------------------------
// JSON protocol types (matching AGENT.md)
// ---------------------------------------------------------------------------

/// The complete game state sent to an AI agent before each move.
///
/// This follows the JSON protocol defined in AGENT.md Section 5.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GameStateJson {
    /// Map of occupied squares. Key = square name (e.g. "e4"), value = piece symbol.
    /// Empty squares are not listed.
    pub board: HashMap<String, String>,

    /// Side to move: "white" or "black".
    pub turn: Color,

    /// Castling rights for both sides.
    pub castling: CastlingRights,

    /// If a pawn advanced two squares in the last move, this is the
    /// en passant capture square. Otherwise null.
    pub en_passant: Option<String>,

    /// Number of half-moves since the last pawn move or capture (50-move rule).
    pub halfmove_clock: u32,

    /// Full-move counter. Starts at 1, incremented after Black's move.
    pub fullmove_number: u32,

    /// List of all previous position FEN strings for threefold repetition detection.
    pub position_history: Vec<String>,
}

/// A move submitted by an AI agent.
///
/// This follows the JSON protocol defined in AGENT.md Section 6.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MoveJson {
    /// Starting square of the piece (e.g. "e2").
    pub from: String,

    /// Target square of the piece (e.g. "e4").
    pub to: String,

    /// For pawn promotion: the target piece as an uppercase letter
    /// ("Q", "R", "B", "N"). Otherwise null.
    pub promotion: Option<String>,
}

/// A special action (non-move) submitted by an AI agent.
///
/// Used for draw claims, draw offers, and resignation
/// (AGENT.md Section 11).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActionJson {
    /// The action type: "claim_draw", "offer_draw", or "resign".
    pub action: String,

    /// Reason for the action (for draw claims): "threefold_repetition"
    /// or "fifty_move_rule". Optional for other actions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Combined agent response — either a move or a special action.
///
/// The system tries to parse as a `MoveJson` first, then as an `ActionJson`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum AgentResponse {
    /// A regular chess move.
    Move(MoveJson),
    /// A special action (draw claim, draw offer, resignation).
    Action(ActionJson),
}

// ---------------------------------------------------------------------------
// Game result
// ---------------------------------------------------------------------------

/// The result of a completed game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum GameResult {
    /// White wins (e.g. by checkmate or Black resignation).
    WhiteWins,
    /// Black wins (e.g. by checkmate or White resignation).
    BlackWins,
    /// The game is a draw.
    Draw,
}

impl fmt::Display for GameResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameResult::WhiteWins => write!(f, "1-0 (White wins)"),
            GameResult::BlackWins => write!(f, "0-1 (Black wins)"),
            GameResult::Draw => write!(f, "1/2-1/2 (Draw)"),
        }
    }
}

/// The reason a game ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum GameEndReason {
    Checkmate,
    Stalemate,
    ThreefoldRepetition,
    FivefoldRepetition,
    FiftyMoveRule,
    SeventyFiveMoveRule,
    InsufficientMaterial,
    Resignation,
    DrawAgreement,
}

impl fmt::Display for GameEndReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameEndReason::Checkmate => write!(f, "Checkmate"),
            GameEndReason::Stalemate => write!(f, "Stalemate"),
            GameEndReason::ThreefoldRepetition => write!(f, "Threefold repetition"),
            GameEndReason::FivefoldRepetition => write!(f, "Fivefold repetition"),
            GameEndReason::FiftyMoveRule => write!(f, "50-move rule"),
            GameEndReason::SeventyFiveMoveRule => write!(f, "75-move rule"),
            GameEndReason::InsufficientMaterial => write!(f, "Insufficient material"),
            GameEndReason::Resignation => write!(f, "Resignation"),
            GameEndReason::DrawAgreement => write!(f, "Draw by agreement"),
        }
    }
}

// ---------------------------------------------------------------------------
// Internal move representation
// ---------------------------------------------------------------------------

/// Internal representation of a chess move (used by the engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChessMove {
    pub from: Square,
    pub to: Square,
    pub promotion: Option<PieceKind>,
    pub is_castling: bool,
    pub is_en_passant: bool,
}

impl ChessMove {
    /// Creates a simple move (no promotion, no castling, no en passant).
    pub fn simple(from: Square, to: Square) -> Self {
        Self {
            from,
            to,
            promotion: None,
            is_castling: false,
            is_en_passant: false,
        }
    }

    /// Converts this internal move to the JSON protocol format.
    pub fn to_json(&self) -> MoveJson {
        MoveJson {
            from: self.from.to_algebraic(),
            to: self.to.to_algebraic(),
            promotion: self.promotion.map(|k| {
                match k {
                    PieceKind::Queen => "Q",
                    PieceKind::Rook => "R",
                    PieceKind::Bishop => "B",
                    PieceKind::Knight => "N",
                    _ => unreachable!("Invalid promotion piece"),
                }
                .to_string()
            }),
        }
    }

    /// Parses a `MoveJson` into an internal `ChessMove`.
    /// Note: `is_castling` and `is_en_passant` flags are set later
    /// during move validation.
    pub fn from_json(mj: &MoveJson) -> Result<Self, String> {
        let from = Square::from_algebraic(&mj.from)
            .ok_or_else(|| format!("Invalid from square: {}", mj.from))?;
        let to = Square::from_algebraic(&mj.to)
            .ok_or_else(|| format!("Invalid to square: {}", mj.to))?;
        let promotion = match &mj.promotion {
            Some(p) => {
                let kind = match p.as_str() {
                    "Q" => PieceKind::Queen,
                    "R" => PieceKind::Rook,
                    "B" => PieceKind::Bishop,
                    "N" => PieceKind::Knight,
                    _ => return Err(format!("Invalid promotion piece: {}", p)),
                };
                Some(kind)
            }
            None => None,
        };
        Ok(ChessMove {
            from,
            to,
            promotion,
            is_castling: false,
            is_en_passant: false,
        })
    }
}

impl fmt::Display for ChessMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.from.to_algebraic(), self.to.to_algebraic())?;
        if let Some(promo) = self.promotion {
            let c = match promo {
                PieceKind::Queen => 'Q',
                PieceKind::Rook => 'R',
                PieceKind::Bishop => 'B',
                PieceKind::Knight => 'N',
                _ => '?',
            };
            write!(f, "={}", c)?;
        }
        Ok(())
    }
}
