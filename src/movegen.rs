//! Move generation and validation for the CheckAI chess engine.
//!
//! This module implements complete legal move generation following
//! FIDE 2023 Laws of Chess (Articles 3, 5, 9). It handles:
//!
//! - Piece movement patterns (King, Queen, Rook, Bishop, Knight, Pawn)
//! - Castling (kingside and queenside, with all conditions)
//! - En passant captures
//! - Pawn promotion
//! - Check detection and prevention (no move may leave own king in check)
//! - Special draw conditions (insufficient material, stalemate)

use crate::types::*;

// ---------------------------------------------------------------------------
// Attack detection
// ---------------------------------------------------------------------------

/// Returns `true` if the given square is attacked by any piece of `attacker_color`.
///
/// This is used for:
/// - Check detection (is the king attacked?)
/// - Castling validation (king must not pass through or land on attacked squares)
pub fn is_square_attacked(board: &Board, sq: Square, attacker_color: Color) -> bool {
    // Check knight attacks
    let knight_offsets: [(i8, i8); 8] = [
        (-2, -1), (-2, 1), (-1, -2), (-1, 2),
        (1, -2), (1, 2), (2, -1), (2, 1),
    ];
    for &(df, dr) in &knight_offsets {
        if let Some(from) = sq.offset(df, dr)
            && let Some(piece) = board.get(from)
            && piece.color == attacker_color && piece.kind == PieceKind::Knight
        {
            return true;
        }
    }

    // Check king attacks (one square in any direction)
    for df in -1..=1i8 {
        for dr in -1..=1i8 {
            if df == 0 && dr == 0 {
                continue;
            }
            if let Some(from) = sq.offset(df, dr)
                && let Some(piece) = board.get(from)
                && piece.color == attacker_color && piece.kind == PieceKind::King
            {
                return true;
            }
        }
    }

    // Check pawn attacks
    let pawn_dir: i8 = match attacker_color {
        Color::White => 1,
        Color::Black => -1,
    };
    // Pawns attack diagonally from their perspective
    for df in [-1i8, 1] {
        // The attacking pawn is below (for white) or above (for black) the target
        if let Some(from) = sq.offset(df, -pawn_dir)
            && let Some(piece) = board.get(from)
            && piece.color == attacker_color && piece.kind == PieceKind::Pawn
        {
            return true;
        }
    }

    // Check sliding pieces (bishop, rook, queen) along rays
    let bishop_dirs: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
    let rook_dirs: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    // Bishop/Queen along diagonals
    for &(df, dr) in &bishop_dirs {
        let mut cur = sq;
        loop {
            match cur.offset(df, dr) {
                None => break,
                Some(next) => {
                    if let Some(piece) = board.get(next) {
                        if piece.color == attacker_color
                            && (piece.kind == PieceKind::Bishop || piece.kind == PieceKind::Queen)
                        {
                            return true;
                        }
                        break; // blocked by another piece
                    }
                    cur = next;
                }
            }
        }
    }

    // Rook/Queen along files and ranks
    for &(df, dr) in &rook_dirs {
        let mut cur = sq;
        loop {
            match cur.offset(df, dr) {
                None => break,
                Some(next) => {
                    if let Some(piece) = board.get(next) {
                        if piece.color == attacker_color
                            && (piece.kind == PieceKind::Rook || piece.kind == PieceKind::Queen)
                        {
                            return true;
                        }
                        break;
                    }
                    cur = next;
                }
            }
        }
    }

    false
}

/// Returns `true` if the king of the given color is currently in check.
pub fn is_in_check(board: &Board, color: Color) -> bool {
    if let Some(king_sq) = board.find_king(color) {
        is_square_attacked(board, king_sq, color.opponent())
    } else {
        // No king found — should never happen in a legal game
        false
    }
}

// ---------------------------------------------------------------------------
// Pseudo-legal move generation (before check filtering)
// ---------------------------------------------------------------------------

/// Generates all pseudo-legal moves for the given side.
///
/// "Pseudo-legal" means the moves follow piece movement rules but may leave
/// the own king in check. The final `generate_legal_moves` function filters
/// those out.
fn generate_pseudo_legal_moves(
    board: &Board,
    turn: Color,
    castling: &CastlingRights,
    en_passant: Option<Square>,
) -> Vec<ChessMove> {
    let mut moves = Vec::with_capacity(64);

    for rank in 0..8u8 {
        for file in 0..8u8 {
            let from = Square::new(file, rank);
            let piece = match board.get(from) {
                Some(p) if p.color == turn => p,
                _ => continue,
            };

            match piece.kind {
                PieceKind::King => generate_king_moves(board, from, turn, castling, &mut moves),
                PieceKind::Queen => generate_sliding_moves(board, from, turn, &QUEEN_DIRS, &mut moves),
                PieceKind::Rook => generate_sliding_moves(board, from, turn, &ROOK_DIRS, &mut moves),
                PieceKind::Bishop => generate_sliding_moves(board, from, turn, &BISHOP_DIRS, &mut moves),
                PieceKind::Knight => generate_knight_moves(board, from, turn, &mut moves),
                PieceKind::Pawn => generate_pawn_moves(board, from, turn, en_passant, &mut moves),
            }
        }
    }

    moves
}

/// Direction vectors for sliding pieces.
const ROOK_DIRS: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const BISHOP_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
const QUEEN_DIRS: [(i8, i8); 8] = [
    (-1, 0), (1, 0), (0, -1), (0, 1),
    (-1, -1), (-1, 1), (1, -1), (1, 1),
];

/// Generates sliding piece moves (rook, bishop, queen).
fn generate_sliding_moves(
    board: &Board,
    from: Square,
    color: Color,
    directions: &[(i8, i8)],
    moves: &mut Vec<ChessMove>,
) {
    for &(df, dr) in directions {
        let mut cur = from;
        loop {
            match cur.offset(df, dr) {
                None => break,
                Some(to) => {
                    match board.get(to) {
                        None => {
                            moves.push(ChessMove::simple(from, to));
                            cur = to;
                        }
                        Some(target) => {
                            if target.color != color {
                                moves.push(ChessMove::simple(from, to)); // capture
                            }
                            break; // blocked
                        }
                    }
                }
            }
        }
    }
}

/// Generates knight moves.
fn generate_knight_moves(
    board: &Board,
    from: Square,
    color: Color,
    moves: &mut Vec<ChessMove>,
) {
    let offsets: [(i8, i8); 8] = [
        (-2, -1), (-2, 1), (-1, -2), (-1, 2),
        (1, -2), (1, 2), (2, -1), (2, 1),
    ];
    for &(df, dr) in &offsets {
        if let Some(to) = from.offset(df, dr) {
            match board.get(to) {
                None => moves.push(ChessMove::simple(from, to)),
                Some(target) => {
                    if target.color != color {
                        moves.push(ChessMove::simple(from, to));
                    }
                }
            }
        }
    }
}

/// Generates king moves (including castling).
fn generate_king_moves(
    board: &Board,
    from: Square,
    color: Color,
    castling: &CastlingRights,
    moves: &mut Vec<ChessMove>,
) {
    // Normal king moves (one square in any direction)
    for df in -1..=1i8 {
        for dr in -1..=1i8 {
            if df == 0 && dr == 0 {
                continue;
            }
            if let Some(to) = from.offset(df, dr) {
                match board.get(to) {
                    None => moves.push(ChessMove::simple(from, to)),
                    Some(target) => {
                        if target.color != color {
                            moves.push(ChessMove::simple(from, to));
                        }
                    }
                }
            }
        }
    }

    // Castling
    let rights = castling.for_color(color);
    let rank = match color {
        Color::White => 0u8,
        Color::Black => 7u8,
    };
    let king_start = Square::new(4, rank);

    // Only attempt castling if king is on its starting square
    if from != king_start {
        return;
    }

    // King must not be in check to castle
    if is_square_attacked(board, from, color.opponent()) {
        return;
    }

    // Kingside castling
    if rights.kingside {
        let f_sq = Square::new(5, rank);
        let g_sq = Square::new(6, rank);
        let rook_sq = Square::new(7, rank);

        // Squares between king and rook must be empty
        let path_clear = board.get(f_sq).is_none() && board.get(g_sq).is_none();

        // Rook must be present
        let rook_present = matches!(
            board.get(rook_sq),
            Some(Piece { kind: PieceKind::Rook, color: c }) if c == color
        );

        // King must not pass through or land on attacked squares
        let safe = !is_square_attacked(board, f_sq, color.opponent())
            && !is_square_attacked(board, g_sq, color.opponent());

        if path_clear && rook_present && safe {
            moves.push(ChessMove {
                from,
                to: g_sq,
                promotion: None,
                is_castling: true,
                is_en_passant: false,
            });
        }
    }

    // Queenside castling
    if rights.queenside {
        let d_sq = Square::new(3, rank);
        let c_sq = Square::new(2, rank);
        let b_sq = Square::new(1, rank);
        let rook_sq = Square::new(0, rank);

        // Squares between king and rook must be empty
        let path_clear = board.get(d_sq).is_none()
            && board.get(c_sq).is_none()
            && board.get(b_sq).is_none();

        // Rook must be present
        let rook_present = matches!(
            board.get(rook_sq),
            Some(Piece { kind: PieceKind::Rook, color: c }) if c == color
        );

        // King must not pass through or land on attacked squares
        // (b1/b8 does not need to be safe — only the king's path d,c)
        let safe = !is_square_attacked(board, d_sq, color.opponent())
            && !is_square_attacked(board, c_sq, color.opponent());

        if path_clear && rook_present && safe {
            moves.push(ChessMove {
                from,
                to: c_sq,
                promotion: None,
                is_castling: true,
                is_en_passant: false,
            });
        }
    }
}

/// Generates pawn moves (forward, captures, en passant, promotion).
fn generate_pawn_moves(
    board: &Board,
    from: Square,
    color: Color,
    en_passant: Option<Square>,
    moves: &mut Vec<ChessMove>,
) {
    let dir = color.pawn_direction();
    let start_rank = color.pawn_start_rank();
    let promo_rank = color.promotion_rank();

    // Helper to add moves (with promotion variants if applicable)
    let mut add_move = |from: Square, to: Square, is_ep: bool| {
        if to.rank == promo_rank {
            // Must promote — add all four options
            for kind in [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight] {
                moves.push(ChessMove {
                    from,
                    to,
                    promotion: Some(kind),
                    is_castling: false,
                    is_en_passant: false,
                });
            }
        } else {
            moves.push(ChessMove {
                from,
                to,
                promotion: None,
                is_castling: false,
                is_en_passant: is_ep,
            });
        }
    };

    // Single step forward
    if let Some(one_ahead) = from.offset(0, dir)
        && board.get(one_ahead).is_none()
    {
        add_move(from, one_ahead, false);

        // Double step from starting rank
        if from.rank == start_rank
            && let Some(two_ahead) = from.offset(0, dir * 2)
            && board.get(two_ahead).is_none()
        {
            add_move(from, two_ahead, false);
        }
    }

    // Diagonal captures
    for df in [-1i8, 1] {
        if let Some(to) = from.offset(df, dir) {
            // Normal capture
            if let Some(target) = board.get(to)
                && target.color != color
            {
                add_move(from, to, false);
            }

            // En passant capture
            if let Some(ep_sq) = en_passant
                && to == ep_sq
            {
                add_move(from, to, true);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Legal move generation (with check filtering)
// ---------------------------------------------------------------------------

/// Generates all legal moves for the given position.
///
/// A legal move is a pseudo-legal move that does not leave or place
/// the own king in check (FIDE Art. 3).
pub fn generate_legal_moves(
    board: &Board,
    turn: Color,
    castling: &CastlingRights,
    en_passant: Option<Square>,
) -> Vec<ChessMove> {
    let pseudo_moves = generate_pseudo_legal_moves(board, turn, castling, en_passant);
    let mut legal_moves = Vec::with_capacity(pseudo_moves.len());

    for mv in pseudo_moves {
        // Apply the move on a temporary board
        let mut test_board = board.clone();
        apply_move_to_board(&mut test_board, &mv, turn);

        // Check if our king is safe after the move
        if !is_in_check(&test_board, turn) {
            legal_moves.push(mv);
        }
    }

    legal_moves
}

/// Applies a move to a board (mutating it). Used for testing legality
/// and for actually making moves in the game.
///
/// This handles:
/// - Normal moves and captures
/// - Castling (moves both king and rook)
/// - En passant (removes the captured pawn)
/// - Promotion (replaces pawn with promoted piece)
pub fn apply_move_to_board(board: &mut Board, mv: &ChessMove, color: Color) {
    let piece = board.get(mv.from).expect("No piece on from square");

    // Clear the source square
    board.set(mv.from, None);

    // Handle castling — move the rook
    if mv.is_castling {
        let rank = mv.from.rank;
        if mv.to.file == 6 {
            // Kingside: rook h -> f
            let rook = board.get(Square::new(7, rank));
            board.set(Square::new(7, rank), None);
            board.set(Square::new(5, rank), rook);
        } else if mv.to.file == 2 {
            // Queenside: rook a -> d
            let rook = board.get(Square::new(0, rank));
            board.set(Square::new(0, rank), None);
            board.set(Square::new(3, rank), rook);
        }
    }

    // Handle en passant — remove the captured pawn
    if mv.is_en_passant {
        let captured_rank = match color {
            Color::White => mv.to.rank - 1,
            Color::Black => mv.to.rank + 1,
        };
        board.set(Square::new(mv.to.file, captured_rank), None);
    }

    // Place the piece (or promoted piece) on the target square
    let placed_piece = if let Some(promo_kind) = mv.promotion {
        Piece::new(promo_kind, color)
    } else {
        piece
    };
    board.set(mv.to, Some(placed_piece));
}

// ---------------------------------------------------------------------------
// Insufficient material detection (dead position)
// ---------------------------------------------------------------------------

/// Checks if the position has insufficient material for checkmate.
///
/// Returns `true` for "dead positions" per FIDE Art. 5.2.2:
/// - K vs K
/// - K+B vs K
/// - K+N vs K
/// - K+B vs K+B (both bishops on same color squares)
pub fn is_insufficient_material(board: &Board) -> bool {
    let mut white_pieces: Vec<(PieceKind, Square)> = Vec::new();
    let mut black_pieces: Vec<(PieceKind, Square)> = Vec::new();

    for rank in 0..8u8 {
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            if let Some(piece) = board.get(sq) {
                match piece.color {
                    Color::White => white_pieces.push((piece.kind, sq)),
                    Color::Black => black_pieces.push((piece.kind, sq)),
                }
            }
        }
    }

    // Filter out kings to get non-king pieces
    let white_non_king: Vec<_> = white_pieces.iter().filter(|(k, _)| *k != PieceKind::King).collect();
    let black_non_king: Vec<_> = black_pieces.iter().filter(|(k, _)| *k != PieceKind::King).collect();

    let wc = white_non_king.len();
    let bc = black_non_king.len();

    // K vs K
    if wc == 0 && bc == 0 {
        return true;
    }

    // K+B vs K or K+N vs K
    if wc == 0 && bc == 1 {
        let kind = black_non_king[0].0;
        if kind == PieceKind::Bishop || kind == PieceKind::Knight {
            return true;
        }
    }
    if bc == 0 && wc == 1 {
        let kind = white_non_king[0].0;
        if kind == PieceKind::Bishop || kind == PieceKind::Knight {
            return true;
        }
    }

    // K+B vs K+B (same-colored squares)
    if wc == 1 && bc == 1 {
        let (wk, wsq) = white_non_king[0];
        let (bk, bsq) = black_non_king[0];
        if *wk == PieceKind::Bishop && *bk == PieceKind::Bishop {
            let w_color = (wsq.file + wsq.rank) % 2;
            let b_color = (bsq.file + bsq.rank) % 2;
            if w_color == b_color {
                return true;
            }
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Move matching (find the legal move matching a MoveJson)
// ---------------------------------------------------------------------------

/// Finds the legal move that matches the given `MoveJson` input.
///
/// Returns `Ok(ChessMove)` if exactly one legal move matches,
/// or `Err(String)` with a detailed error message.
pub fn find_matching_legal_move(
    board: &Board,
    turn: Color,
    castling: &CastlingRights,
    en_passant: Option<Square>,
    move_json: &MoveJson,
) -> Result<ChessMove, String> {
    let from = Square::from_algebraic(&move_json.from)
        .ok_or_else(|| format!("Invalid from square: {}", move_json.from))?;
    let to = Square::from_algebraic(&move_json.to)
        .ok_or_else(|| format!("Invalid to square: {}", move_json.to))?;
    let promotion = match &move_json.promotion {
        Some(p) => Some(match p.as_str() {
            "Q" => PieceKind::Queen,
            "R" => PieceKind::Rook,
            "B" => PieceKind::Bishop,
            "N" => PieceKind::Knight,
            _ => return Err(format!("Invalid promotion piece: {}", p)),
        }),
        None => None,
    };

    // Verify a piece of the correct color is on the from square
    match board.get(from) {
        None => return Err(format!("No piece on square {}", move_json.from)),
        Some(piece) => {
            if piece.color != turn {
                return Err(format!(
                    "Piece on {} belongs to {:?}, but it is {:?}'s turn",
                    move_json.from, piece.color, turn
                ));
            }
        }
    }

    let legal_moves = generate_legal_moves(board, turn, castling, en_passant);

    // Find matching move
    let matching: Vec<_> = legal_moves
        .iter()
        .filter(|m| m.from == from && m.to == to && m.promotion == promotion)
        .cloned()
        .collect();

    match matching.len() {
        0 => {
            // Provide helpful error message
            let from_piece = board.get(from).map(|p| p.to_fen_char()).unwrap_or('?');
            let available: Vec<String> = legal_moves
                .iter()
                .filter(|m| m.from == from)
                .map(|m| m.to_string())
                .collect();
            if available.is_empty() {
                Err(format!(
                    "Illegal move: {} ({}) has no legal moves",
                    move_json.from, from_piece
                ))
            } else {
                Err(format!(
                    "Illegal move: {}{}{} is not legal. Legal moves from {}: {}",
                    move_json.from,
                    move_json.to,
                    move_json.promotion.as_deref().unwrap_or(""),
                    move_json.from,
                    available.join(", ")
                ))
            }
        }
        1 => Ok(matching[0]),
        _ => {
            // Should not happen with proper promotion disambiguation
            Ok(matching[0])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position_has_20_moves() {
        let board = Board::starting_position();
        let castling = CastlingRights::default();
        let moves = generate_legal_moves(&board, Color::White, &castling, None);
        assert_eq!(moves.len(), 20, "White should have 20 legal moves in starting position");
    }

    #[test]
    fn test_starting_position_not_in_check() {
        let board = Board::starting_position();
        assert!(!is_in_check(&board, Color::White));
        assert!(!is_in_check(&board, Color::Black));
    }

    #[test]
    fn test_insufficient_material_k_vs_k() {
        let mut board = Board::default();
        board.set(Square::new(4, 0), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(4, 7), Some(Piece::new(PieceKind::King, Color::Black)));
        assert!(is_insufficient_material(&board));
    }

    #[test]
    fn test_insufficient_material_kb_vs_k() {
        let mut board = Board::default();
        board.set(Square::new(4, 0), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(2, 2), Some(Piece::new(PieceKind::Bishop, Color::White)));
        board.set(Square::new(4, 7), Some(Piece::new(PieceKind::King, Color::Black)));
        assert!(is_insufficient_material(&board));
    }

    #[test]
    fn test_not_insufficient_with_rook() {
        let mut board = Board::default();
        board.set(Square::new(4, 0), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(4, 7), Some(Piece::new(PieceKind::King, Color::Black)));
        assert!(!is_insufficient_material(&board));
    }

    #[test]
    fn test_en_passant_move_generated() {
        let mut board = Board::default();
        board.set(Square::new(4, 0), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(4, 7), Some(Piece::new(PieceKind::King, Color::Black)));
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Pawn, Color::Black)));

        let castling = CastlingRights {
            white: SideCastlingRights { kingside: false, queenside: false },
            black: SideCastlingRights { kingside: false, queenside: false },
        };
        let ep = Some(Square::new(3, 5)); // d6
        let moves = generate_legal_moves(&board, Color::White, &castling, ep);

        let ep_moves: Vec<_> = moves.iter().filter(|m| m.is_en_passant).collect();
        assert_eq!(ep_moves.len(), 1, "Should have exactly one en passant move");
        assert_eq!(ep_moves[0].from, Square::new(4, 4)); // e5
        assert_eq!(ep_moves[0].to, Square::new(3, 5));   // d6
    }

    #[test]
    fn test_castling_available_in_clear_position() {
        let mut board = Board::default();
        board.set(Square::new(4, 0), Some(Piece::new(PieceKind::King, Color::White)));
        board.set(Square::new(7, 0), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(4, 7), Some(Piece::new(PieceKind::King, Color::Black)));

        let castling = CastlingRights {
            white: SideCastlingRights { kingside: true, queenside: true },
            black: SideCastlingRights { kingside: false, queenside: false },
        };

        let moves = generate_legal_moves(&board, Color::White, &castling, None);
        let castling_moves: Vec<_> = moves.iter().filter(|m| m.is_castling).collect();
        assert_eq!(castling_moves.len(), 2, "Should have both kingside and queenside castling");
    }
}
