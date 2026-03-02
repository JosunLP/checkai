//! Position evaluation for the CheckAI analysis engine.
//!
//! Implements a PeSTO-style evaluation function combining:
//! - Material counting (centipawn values)
//! - Piece-square tables (midgame + endgame, interpolated by game phase)
//! - Pawn structure bonuses/penalties
//! - Bishop pair bonus
//! - Rook on open / semi-open file bonus
//!
//! The evaluation is always from the perspective of the side to move.
//! Positive values favour the side to move; negative values favour the
//! opponent.

use crate::types::*;

// ---------------------------------------------------------------------------
// Material values (centipawns)
// ---------------------------------------------------------------------------

/// Midgame piece values.
const MG_VALUE: [i32; 6] = [
    82,   // Pawn
    337,  // Knight
    365,  // Bishop
    477,  // Rook
    1025, // Queen
    0,    // King (not counted for material balance)
];

/// Endgame piece values.
const EG_VALUE: [i32; 6] = [
    94,  // Pawn
    281, // Knight
    297, // Bishop
    512, // Rook
    936, // Queen
    0,   // King
];

/// Maps `PieceKind` to an index into the value / PST arrays.
fn piece_index(kind: PieceKind) -> usize {
    match kind {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 1,
        PieceKind::Bishop => 2,
        PieceKind::Rook => 3,
        PieceKind::Queen => 4,
        PieceKind::King => 5,
    }
}

// ---------------------------------------------------------------------------
// Game-phase weights (for midgame ↔ endgame interpolation)
// ---------------------------------------------------------------------------

/// Phase contribution of each non-pawn, non-king piece type.
const PHASE_WEIGHT: [i32; 6] = [
    0, // Pawn
    1, // Knight
    1, // Bishop
    2, // Rook
    4, // Queen
    0, // King
];

/// Maximum possible phase value (all minor + major pieces on the board).
/// 4 knights/bishops × 1 + 4 rooks × 2 + 2 queens × 4 = 24
const PHASE_MAX: i32 = 24;

// ---------------------------------------------------------------------------
// Piece-Square Tables (PeSTO-tuned values)
//
// Stored from White's perspective with a1 = index 0, h8 = index 63.
// Index = rank * 8 + file (rank 0 = rank 1, rank 7 = rank 8).
//
// For Black pieces, mirror the rank: sq_index = (7 - rank) * 8 + file.
// ---------------------------------------------------------------------------

/// Midgame Pawn PST.
#[rustfmt::skip]
const MG_PAWN_TABLE: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
    -35,  -1, -20, -23, -15,  24,  38, -22,
    -26,  -4,  -4, -10,   3,   3,  33, -12,
    -27,  -2,  -5,  12,  17,   6,  10, -25,
    -14,  13,   6,  21,  23,  12,  17, -23,
     -6,   7,  26,  31,  65,  56,  25, -20,
     98, 134,  61,  95,  68, 126,  34, -11,
      0,   0,   0,   0,   0,   0,   0,   0,
];

/// Endgame Pawn PST.
#[rustfmt::skip]
const EG_PAWN_TABLE: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
     13,   8,   8,  10,  13,   0,   2,  -7,
      4,   7,  -6,   1,   0,  -5,  -1,  -8,
     13,   9,  -3,  -7,  -7,  -8,   3,  -1,
     32,  24,  13,   5,  -2,   4,  17,  17,
     94, 100,  85,  67,  56,  53,  82,  84,
    178, 173, 158, 134, 147, 132, 165, 187,
      0,   0,   0,   0,   0,   0,   0,   0,
];

/// Midgame Knight PST.
#[rustfmt::skip]
const MG_KNIGHT_TABLE: [i32; 64] = [
   -167, -89, -34, -49,  61, -97, -15,-107,
    -73, -41,  72,  36,  23,  62,   7, -17,
    -47,  60,  37,  65,  84, 129,  73,  44,
     -9,  17,  19,  53,  37,  69,  18,  22,
    -13,   4,  16,  13,  28,  19,  21,  -8,
    -23,  -9,  12,  10,  19,  17,  25, -16,
    -29, -53, -12,  -3,  -1,  18, -14, -19,
   -105, -21, -58, -33, -17, -28, -19, -23,
];

/// Endgame Knight PST.
#[rustfmt::skip]
const EG_KNIGHT_TABLE: [i32; 64] = [
    -58, -38, -13, -28, -31, -27, -63, -99,
    -25,  -8, -25,  -2,  -9, -25, -24, -52,
    -24, -20,  10,   9,  -1,  -9, -19, -41,
    -17,   3,  22,  22,  22,  11,   8, -18,
    -18,  -6,  16,  25,  16,  17,   4, -18,
    -23,  -3,  -1,  15,  10,  -3, -20, -22,
    -42, -20, -10,  -5,  -2, -20, -23, -44,
    -29, -51, -23, -15, -22, -18, -50, -64,
];

/// Midgame Bishop PST.
#[rustfmt::skip]
const MG_BISHOP_TABLE: [i32; 64] = [
    -29,   4, -82, -37, -25, -42,   7,  -8,
    -26,  16, -18, -13,  30,  59,  18, -47,
    -16,  37,  43,  40,  35,  50,  37,  -2,
     -4,   5,  19,  50,  37,  37,   7,  -2,
     -6,  13,  13,  26,  34,  12,  10,   4,
      0,  15,  15,  15,  14,  27,  18,  10,
      4,  15,  16,   0,   7,  21,  33,   1,
    -33,  -3, -14, -21, -13, -12, -39, -21,
];

/// Endgame Bishop PST.
#[rustfmt::skip]
const EG_BISHOP_TABLE: [i32; 64] = [
    -14, -21, -11,  -8,  -7,  -9, -17, -24,
     -8,  -4,   7, -12,  -3, -13,  -4, -14,
      2,  -8,   0,  -1,  -2,   6,   0,   4,
     -3,   9,  12,   9,  14,  10,   3,   2,
     -6,   3,  13,  19,   7,  10,  -3,  -9,
    -12,  -3,   8,  10,  13,   3,  -7, -15,
    -14, -18,  -7,  -1,   4,  -9, -15, -27,
    -23,  -9, -23,  -5,  -9, -16,  -5, -17,
];

/// Midgame Rook PST.
#[rustfmt::skip]
const MG_ROOK_TABLE: [i32; 64] = [
     32,  42,  32,  51,  63,   9,  31,  43,
     27,  32,  58,  62,  80,  67,  26,  44,
     -5,  19,  26,  36,  17,  45,  61,  16,
    -24, -11,   7,  26,  24,  35,  -8, -20,
    -36, -26, -12,  -1,   9,  -7,   6, -23,
    -45, -25, -16, -17,   3,   0,  -5, -33,
    -44, -16, -20,  -9,  -1,  11,  -6, -71,
    -19, -13,   1,  17,  16,   7, -37, -26,
];

/// Endgame Rook PST.
#[rustfmt::skip]
const EG_ROOK_TABLE: [i32; 64] = [
     13,  10,  18,  15,  12,  12,   8,   5,
     11,  13,  13,  11,  -3,   3,   8,   3,
      7,   7,   7,   5,   4,  -3,  -5,  -3,
      4,   3,  13,   1,   2,   1,  -1,   2,
      3,   5,   8,   4,  -5,  -6,  -8, -11,
     -4,   0,  -5,  -1,  -7, -12,  -8, -16,
     -6,  -6,   0,   2,  -9,  -9, -11,  -3,
     -9,   2,   3,  -1,  -5, -13,   4, -20,
];

/// Midgame Queen PST.
#[rustfmt::skip]
const MG_QUEEN_TABLE: [i32; 64] = [
    -28,   0,  29,  12,  59,  44,  43,  45,
    -24, -39,  -5,   1, -16,  57,  28,  54,
    -13, -17,   7,   8,  29,  56,  47,  57,
    -27, -27, -16, -16,  -1,  17,  -2,   1,
     -9, -26,  -9, -10,  -2,  -4,   3,  -3,
    -14,   2, -11,  -2,  -5,   2,  14,   5,
    -35,  -8,  11,   2,   8,  15,  -3,   1,
     -1, -18,  -9,  10, -15, -25, -31, -50,
];

/// Endgame Queen PST.
#[rustfmt::skip]
const EG_QUEEN_TABLE: [i32; 64] = [
     -9,  22,  22,  27,  27,  19,  10,  20,
    -17,  20,  32,  41,  58,  25,  30,   0,
    -20,   6,   9,  49,  47,  35,  19,   9,
      3,  22,  24,  45,  57,  40,  57,  36,
    -18,  28,  19,  47,  31,  34,  39,  23,
    -16, -27,  15,   6,   9,  17,  10,   5,
    -22, -23, -30, -16, -16, -23, -36, -32,
    -33, -28, -22, -43,  -5, -32, -20, -41,
];

/// Midgame King PST.
#[rustfmt::skip]
const MG_KING_TABLE: [i32; 64] = [
    -65,  23,  16, -15, -56, -34,   2,  13,
     29,  -1, -20,  -7,  -8,  -4, -38, -29,
     -9,  24,   2, -16, -20,   6,  22, -22,
    -17, -20, -12, -27, -30, -25, -14, -36,
    -49,  -1, -27, -39, -46, -44, -33, -51,
    -14, -14, -22, -46, -44, -30, -15, -27,
      1,   7,  -8, -64, -43, -16,   9,   8,
    -15,  36,  12, -54,   8, -28,  24,  14,
];

/// Endgame King PST.
#[rustfmt::skip]
const EG_KING_TABLE: [i32; 64] = [
    -74, -35, -18, -18, -11,  15,   4, -17,
    -12,  17,  14,  17,  17,  38,  23,  11,
     10,  17,  23,  15,  20,  45,  44,  13,
     -8,  22,  24,  27,  26,  33,  26,   3,
    -18,  -4,  21,  24,  27,  23,   9, -11,
    -19,  -3,  11,  21,  23,  16,   7,  -9,
    -27, -11,   4,  13,  14,   4,  -5, -17,
    -53, -34, -21, -11, -28, -14, -24, -43,
];

// ---------------------------------------------------------------------------
// PST lookup helpers
// ---------------------------------------------------------------------------

/// All midgame PSTs indexed by piece type.
const MG_PST: [[i32; 64]; 6] = [
    MG_PAWN_TABLE,
    MG_KNIGHT_TABLE,
    MG_BISHOP_TABLE,
    MG_ROOK_TABLE,
    MG_QUEEN_TABLE,
    MG_KING_TABLE,
];

/// All endgame PSTs indexed by piece type.
const EG_PST: [[i32; 64]; 6] = [
    EG_PAWN_TABLE,
    EG_KNIGHT_TABLE,
    EG_BISHOP_TABLE,
    EG_ROOK_TABLE,
    EG_QUEEN_TABLE,
    EG_KING_TABLE,
];

/// Returns the square index from White's perspective.
/// For White pieces: `rank * 8 + file`.
/// For Black pieces: `(7 - rank) * 8 + file` (mirrored).
#[inline]
fn pst_index(sq: Square, color: Color) -> usize {
    match color {
        Color::White => sq.index(),
        Color::Black => (7 - sq.rank as usize) * 8 + sq.file as usize,
    }
}

// ---------------------------------------------------------------------------
// Bonus terms
// ---------------------------------------------------------------------------

/// Bonus for having the bishop pair (both bishops alive).
const BISHOP_PAIR_BONUS_MG: i32 = 30;
const BISHOP_PAIR_BONUS_EG: i32 = 50;

/// Bonus for a rook on an open file (no pawns).
const ROOK_OPEN_FILE_MG: i32 = 20;
const ROOK_OPEN_FILE_EG: i32 = 10;

/// Bonus for a rook on a semi-open file (no friendly pawns).
const ROOK_SEMI_OPEN_FILE_MG: i32 = 10;
const ROOK_SEMI_OPEN_FILE_EG: i32 = 5;

/// Penalty for doubled pawns (per doubled pawn).
const DOUBLED_PAWN_PENALTY_MG: i32 = -10;
const DOUBLED_PAWN_PENALTY_EG: i32 = -20;

/// Penalty for isolated pawns.
const ISOLATED_PAWN_PENALTY_MG: i32 = -10;
const ISOLATED_PAWN_PENALTY_EG: i32 = -15;

/// Bonus for passed pawns (indexed by rank from own side, 0-7).
#[rustfmt::skip]
const PASSED_PAWN_BONUS_EG: [i32; 8] = [
    0, 5, 10, 20, 35, 60, 100, 0,
];

// ---------------------------------------------------------------------------
// Evaluation constants
// ---------------------------------------------------------------------------

/// Centipawn value representing a forced mate.
pub const MATE_SCORE: i32 = 30_000;

/// Minimum score that indicates a mating line.
pub const MATE_THRESHOLD: i32 = MATE_SCORE - 500;

/// Evaluation score for a draw.
pub const DRAW_SCORE: i32 = 0;

// ---------------------------------------------------------------------------
// Main evaluation function
// ---------------------------------------------------------------------------

/// Evaluates the position and returns a score in centipawns.
///
/// Positive = side to move is better.
/// Negative = opponent is better.
pub fn evaluate(board: &Board, turn: Color) -> i32 {
    let (mg_white, eg_white, mg_black, eg_black, phase) = accumulate(board);

    let mg_score = mg_white - mg_black;
    let eg_score = eg_white - eg_black;

    // Clamp phase to [0, PHASE_MAX]
    let phase = phase.clamp(0, PHASE_MAX);

    // Interpolate between midgame and endgame scores.
    // phase = PHASE_MAX → pure midgame, phase = 0 → pure endgame.
    let score = (mg_score * phase + eg_score * (PHASE_MAX - phase)) / PHASE_MAX;

    // Return relative to side to move
    match turn {
        Color::White => score,
        Color::Black => -score,
    }
}

/// Accumulates material, PST, and bonus scores for both sides.
///
/// Returns `(mg_white, eg_white, mg_black, eg_black, phase)`.
fn accumulate(board: &Board) -> (i32, i32, i32, i32, i32) {
    let mut mg_white = 0i32;
    let mut eg_white = 0i32;
    let mut mg_black = 0i32;
    let mut eg_black = 0i32;
    let mut phase = 0i32;

    let mut white_bishops = 0u8;
    let mut black_bishops = 0u8;

    // Per-file pawn counts for pawn structure evaluation
    let mut white_pawns_per_file = [0u8; 8];
    let mut black_pawns_per_file = [0u8; 8];

    // Collect rook files
    let mut white_rook_files: Vec<u8> = Vec::new();
    let mut black_rook_files: Vec<u8> = Vec::new();

    for rank in 0..8u8 {
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            if let Some(piece) = board.get(sq) {
                let pi = piece_index(piece.kind);
                let pst_sq = pst_index(sq, piece.color);

                let mg_val = MG_VALUE[pi] + MG_PST[pi][pst_sq];
                let eg_val = EG_VALUE[pi] + EG_PST[pi][pst_sq];

                match piece.color {
                    Color::White => {
                        mg_white += mg_val;
                        eg_white += eg_val;
                        if piece.kind == PieceKind::Bishop {
                            white_bishops += 1;
                        }
                        if piece.kind == PieceKind::Pawn {
                            white_pawns_per_file[file as usize] += 1;
                        }
                        if piece.kind == PieceKind::Rook {
                            white_rook_files.push(file);
                        }
                    }
                    Color::Black => {
                        mg_black += mg_val;
                        eg_black += eg_val;
                        if piece.kind == PieceKind::Bishop {
                            black_bishops += 1;
                        }
                        if piece.kind == PieceKind::Pawn {
                            black_pawns_per_file[file as usize] += 1;
                        }
                        if piece.kind == PieceKind::Rook {
                            black_rook_files.push(file);
                        }
                    }
                }

                phase += PHASE_WEIGHT[pi];
            }
        }
    }

    // Bishop pair bonus
    if white_bishops >= 2 {
        mg_white += BISHOP_PAIR_BONUS_MG;
        eg_white += BISHOP_PAIR_BONUS_EG;
    }
    if black_bishops >= 2 {
        mg_black += BISHOP_PAIR_BONUS_MG;
        eg_black += BISHOP_PAIR_BONUS_EG;
    }

    // Pawn structure
    let (w_pawn_mg, w_pawn_eg) =
        pawn_structure_score(&white_pawns_per_file, &black_pawns_per_file, Color::White);
    let (b_pawn_mg, b_pawn_eg) =
        pawn_structure_score(&black_pawns_per_file, &white_pawns_per_file, Color::Black);
    mg_white += w_pawn_mg;
    eg_white += w_pawn_eg;
    mg_black += b_pawn_mg;
    eg_black += b_pawn_eg;

    // Rook on open / semi-open files
    for &f in &white_rook_files {
        let fi = f as usize;
        if white_pawns_per_file[fi] == 0 && black_pawns_per_file[fi] == 0 {
            mg_white += ROOK_OPEN_FILE_MG;
            eg_white += ROOK_OPEN_FILE_EG;
        } else if white_pawns_per_file[fi] == 0 {
            mg_white += ROOK_SEMI_OPEN_FILE_MG;
            eg_white += ROOK_SEMI_OPEN_FILE_EG;
        }
    }
    for &f in &black_rook_files {
        let fi = f as usize;
        if white_pawns_per_file[fi] == 0 && black_pawns_per_file[fi] == 0 {
            mg_black += ROOK_OPEN_FILE_MG;
            eg_black += ROOK_OPEN_FILE_EG;
        } else if black_pawns_per_file[fi] == 0 {
            mg_black += ROOK_SEMI_OPEN_FILE_MG;
            eg_black += ROOK_SEMI_OPEN_FILE_EG;
        }
    }

    (mg_white, eg_white, mg_black, eg_black, phase)
}

/// Evaluates pawn structure bonuses and penalties.
fn pawn_structure_score(own_pawns: &[u8; 8], opponent_pawns: &[u8; 8], color: Color) -> (i32, i32) {
    let mut mg = 0i32;
    let mut eg = 0i32;

    for file in 0..8usize {
        let count = own_pawns[file];
        if count == 0 {
            continue;
        }

        // Doubled pawns penalty (for each extra pawn beyond the first)
        if count > 1 {
            let extra = (count - 1) as i32;
            mg += DOUBLED_PAWN_PENALTY_MG * extra;
            eg += DOUBLED_PAWN_PENALTY_EG * extra;
        }

        // Isolated pawn: no friendly pawns on adjacent files
        let has_neighbor =
            (file > 0 && own_pawns[file - 1] > 0) || (file < 7 && own_pawns[file + 1] > 0);
        if !has_neighbor {
            mg += ISOLATED_PAWN_PENALTY_MG * count as i32;
            eg += ISOLATED_PAWN_PENALTY_EG * count as i32;
        }

        // Passed pawn: no opponent pawns on same or adjacent files ahead
        // (simplified — checks if any opponent pawn exists on those files)
        let blocked = (file > 0 && opponent_pawns[file - 1] > 0)
            || opponent_pawns[file] > 0
            || (file < 7 && opponent_pawns[file + 1] > 0);
        if !blocked {
            // Rough rank estimate: use the most advanced pawn on this file
            let rank_bonus = match color {
                Color::White => file.min(6), // approximate
                Color::Black => file.min(6),
            };
            eg += PASSED_PAWN_BONUS_EG[rank_bonus.min(7)];
        }
    }

    (mg, eg)
}

/// Quick material-only evaluation for simple endgame detection.
pub fn material_score(board: &Board) -> i32 {
    let mut score = 0i32;
    for rank in 0..8u8 {
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            if let Some(piece) = board.get(sq) {
                let val = MG_VALUE[piece_index(piece.kind)];
                match piece.color {
                    Color::White => score += val,
                    Color::Black => score -= val,
                }
            }
        }
    }
    score
}

/// Counts total pieces on the board (including kings).
pub fn piece_count(board: &Board) -> usize {
    board.squares.iter().filter(|sq| sq.is_some()).count()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position_roughly_equal() {
        let board = Board::starting_position();
        let score = evaluate(&board, Color::White);
        // Starting position should be roughly equal (within ~50 cp)
        assert!(
            score.abs() < 50,
            "Starting position eval should be near 0, got {}",
            score
        );
    }

    #[test]
    fn test_material_up_is_positive() {
        let mut board = Board::starting_position();
        // Remove Black's queen
        board.set(Square::new(3, 7), None);
        let score = evaluate(&board, Color::White);
        assert!(
            score > 500,
            "White should be winning without Black queen, got {}",
            score
        );
    }

    #[test]
    fn test_eval_symmetry() {
        let board = Board::starting_position();
        let w = evaluate(&board, Color::White);
        let b = evaluate(&board, Color::Black);
        // Should be opposite signs (or both near 0)
        assert_eq!(w, -b, "Eval should be symmetric");
    }

    #[test]
    fn test_piece_count() {
        let board = Board::starting_position();
        assert_eq!(piece_count(&board), 32);
    }
}
