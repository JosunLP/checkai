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

/// Penalty for backward pawns (no friendly pawn can protect it from behind,
/// and advancing would walk into an enemy pawn's control).
const BACKWARD_PAWN_PENALTY_MG: i32 = -8;
const BACKWARD_PAWN_PENALTY_EG: i32 = -10;

/// Bonus for connected pawns (pawns that defend each other diagonally).
const CONNECTED_PAWN_BONUS_MG: i32 = 7;
const CONNECTED_PAWN_BONUS_EG: i32 = 5;

/// Bonus for passed pawns (indexed by rank from own side, 0-7).
#[rustfmt::skip]
const PASSED_PAWN_BONUS_EG: [i32; 8] = [
    0, 5, 10, 20, 35, 60, 100, 0,
];

/// Tempo bonus: small advantage for having the move.
const TEMPO_BONUS: i32 = 10;

/// Space bonus per controlled square in the centre on ranks 2-4 (from own side).
const SPACE_BONUS_MG: i32 = 3;

// ---------------------------------------------------------------------------
// King safety
// ---------------------------------------------------------------------------

/// Penalty per missing pawn in the king's pawn shield (files around the king).
const KING_PAWN_SHIELD_PENALTY_MG: i32 = -10;

/// Penalty when an adjacent file to the king is open (no pawns from either side).
const KING_OPEN_FILE_PENALTY_MG: i32 = -20;

/// Penalty per enemy piece on a square within 2 squares of the king (tropism).
const KING_TROPISM_PENALTY_MG: i32 = -3;

// ---------------------------------------------------------------------------
// Piece mobility
// ---------------------------------------------------------------------------

/// Mobility bonus per pseudo-legal square a knight can reach.
const KNIGHT_MOBILITY_MG: i32 = 4;
const KNIGHT_MOBILITY_EG: i32 = 3;

/// Mobility bonus per pseudo-legal square a bishop can reach.
const BISHOP_MOBILITY_MG: i32 = 5;
const BISHOP_MOBILITY_EG: i32 = 4;

/// Mobility bonus per pseudo-legal square a rook can reach.
const ROOK_MOBILITY_MG: i32 = 2;
const ROOK_MOBILITY_EG: i32 = 3;

/// Mobility bonus per pseudo-legal square a queen can reach.
const QUEEN_MOBILITY_MG: i32 = 1;
const QUEEN_MOBILITY_EG: i32 = 2;

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

    // Return relative to side to move, with tempo bonus
    let relative = match turn {
        Color::White => score,
        Color::Black => -score,
    };
    relative + TEMPO_BONUS
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

    // Track the most-advanced rank per file for passed-pawn evaluation
    let mut white_pawn_max_rank = [0u8; 8];
    let mut black_pawn_min_rank = [7u8; 8];

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
                            if rank > white_pawn_max_rank[file as usize] {
                                white_pawn_max_rank[file as usize] = rank;
                            }
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
                            if rank < black_pawn_min_rank[file as usize] {
                                black_pawn_min_rank[file as usize] = rank;
                            }
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
    let (w_pawn_mg, w_pawn_eg) = pawn_structure_score(
        board,
        &white_pawns_per_file,
        &black_pawns_per_file,
        Color::White,
        &white_pawn_max_rank,
    );
    let (b_pawn_mg, b_pawn_eg) = pawn_structure_score(
        board,
        &black_pawns_per_file,
        &white_pawns_per_file,
        Color::Black,
        &black_pawn_min_rank,
    );
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

    // King safety (midgame only)
    let (w_king_mg, b_king_mg) =
        king_safety_score(board, &white_pawns_per_file, &black_pawns_per_file);
    mg_white += w_king_mg;
    mg_black += b_king_mg;

    // Piece mobility
    let (w_mob_mg, w_mob_eg, b_mob_mg, b_mob_eg) = mobility_score(board);
    mg_white += w_mob_mg;
    eg_white += w_mob_eg;
    mg_black += b_mob_mg;
    eg_black += b_mob_eg;

    (mg_white, eg_white, mg_black, eg_black, phase)
}

/// Evaluates pawn structure bonuses and penalties.
fn pawn_structure_score(
    board: &Board,
    own_pawns: &[u8; 8],
    opponent_pawns: &[u8; 8],
    color: Color,
    most_advanced_rank: &[u8; 8],
) -> (i32, i32) {
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
        } else {
            // Backward pawn: has neighbors but none can protect from behind.
            // A pawn is backward if the stop-square is attacked by enemy pawns
            // and no friendly pawn on adjacent files is at the same rank or behind.
            let rank = most_advanced_rank[file];
            let behind_rank_ok = match color {
                Color::White => {
                    (file > 0
                        && own_pawns[file - 1] > 0
                        && most_advanced_rank.get(file - 1).copied().unwrap_or(0) >= rank)
                        || (file < 7
                            && own_pawns[file + 1] > 0
                            && most_advanced_rank.get(file + 1).copied().unwrap_or(0) >= rank)
                }
                Color::Black => {
                    (file > 0
                        && own_pawns[file - 1] > 0
                        && most_advanced_rank.get(file - 1).copied().unwrap_or(7) <= rank)
                        || (file < 7
                            && own_pawns[file + 1] > 0
                            && most_advanced_rank.get(file + 1).copied().unwrap_or(7) <= rank)
                }
            };
            if !behind_rank_ok {
                mg += BACKWARD_PAWN_PENALTY_MG;
                eg += BACKWARD_PAWN_PENALTY_EG;
            }
        }

        // Passed pawn: no opponent pawns on same or adjacent files ahead
        let blocked = (file > 0 && opponent_pawns[file - 1] > 0)
            || opponent_pawns[file] > 0
            || (file < 7 && opponent_pawns[file + 1] > 0);
        if !blocked {
            let rank_bonus = match color {
                Color::White => most_advanced_rank[file] as usize,
                Color::Black => (7 - most_advanced_rank[file]) as usize,
            };
            eg += PASSED_PAWN_BONUS_EG[rank_bonus.min(7)];
        }
    }

    // Connected pawns: bonus for pawns that diagonally defend each other
    let (dir, start_rank, end_rank): (i8, u8, u8) = match color {
        Color::White => (1, 1, 6),
        Color::Black => (-1, 1, 6),
    };
    for rank in start_rank..=end_rank {
        for file_idx in 0..8u8 {
            let sq = Square::new(file_idx, rank);
            if board
                .get(sq)
                .is_some_and(|p| p.kind == PieceKind::Pawn && p.color == color)
            {
                // Check if defended by a friendly pawn on adjacent file behind
                let behind_rank = (rank as i8 - dir) as u8;
                if behind_rank < 8 {
                    let defended = (file_idx > 0
                        && board
                            .get(Square::new(file_idx - 1, behind_rank))
                            .is_some_and(|p| p.kind == PieceKind::Pawn && p.color == color))
                        || (file_idx < 7
                            && board
                                .get(Square::new(file_idx + 1, behind_rank))
                                .is_some_and(|p| p.kind == PieceKind::Pawn && p.color == color));
                    if defended {
                        mg += CONNECTED_PAWN_BONUS_MG;
                        eg += CONNECTED_PAWN_BONUS_EG;
                    }
                }
            }
        }
    }

    // Space advantage: count controlled central squares (files c-f, ranks 2-4 from own side)
    let space_ranks: std::ops::RangeInclusive<u8> = match color {
        Color::White => 1..=3,
        Color::Black => 4..=6,
    };
    let mut space = 0i32;
    for rank in space_ranks {
        for file_idx in 2..=5u8 {
            let sq = Square::new(file_idx, rank);
            if board
                .get(sq)
                .is_some_and(|p| p.kind == PieceKind::Pawn && p.color == color)
            {
                space += 1;
            }
        }
    }
    mg += space * SPACE_BONUS_MG;

    (mg, eg)
}

/// Evaluates king safety for both sides (midgame only).
///
/// Considers the pawn shield around the king and open files near the king.
/// Returns `(white_mg, black_mg)`.
fn king_safety_score(
    board: &Board,
    white_pawns_per_file: &[u8; 8],
    black_pawns_per_file: &[u8; 8],
) -> (i32, i32) {
    let mut w_mg = 0i32;
    let mut b_mg = 0i32;

    // White king safety
    if let Some(wk) = board.find_king(Color::White) {
        let king_file = wk.file as usize;
        let shield_rank = 1u8; // White's pawn shield is on rank 2 (index 1)

        // Check pawn shield on king's file and adjacent files
        let files_to_check: &[usize] = if king_file == 0 {
            &[0, 1]
        } else if king_file == 7 {
            &[6, 7]
        } else {
            &[king_file - 1, king_file, king_file + 1]
        };

        for &f in files_to_check {
            // Penalty if no friendly pawn on this file near the king
            let has_shield = (0..8u8).any(|r| {
                r <= shield_rank + 1
                    && board
                        .get(Square::new(f as u8, r))
                        .is_some_and(|p| p.kind == PieceKind::Pawn && p.color == Color::White)
            });
            if !has_shield {
                w_mg += KING_PAWN_SHIELD_PENALTY_MG;
            }

            // Penalty if the file is completely open
            if white_pawns_per_file[f] == 0 && black_pawns_per_file[f] == 0 {
                w_mg += KING_OPEN_FILE_PENALTY_MG;
            }
        }

        // King tropism: count enemy pieces within Chebyshev distance 2
        for dr in -2..=2i8 {
            for df in -2..=2i8 {
                if dr == 0 && df == 0 {
                    continue;
                }
                if let Some(sq) = wk.offset(df, dr)
                    && let Some(piece) = board.get(sq)
                    && piece.color == Color::Black
                    && piece.kind != PieceKind::Pawn
                {
                    w_mg += KING_TROPISM_PENALTY_MG;
                }
            }
        }
    }

    // Black king safety
    if let Some(bk) = board.find_king(Color::Black) {
        let king_file = bk.file as usize;
        let shield_rank = 6u8; // Black's pawn shield is on rank 7 (index 6)

        let files_to_check: &[usize] = if king_file == 0 {
            &[0, 1]
        } else if king_file == 7 {
            &[6, 7]
        } else {
            &[king_file - 1, king_file, king_file + 1]
        };

        for &f in files_to_check {
            let has_shield = (0..8u8).any(|r| {
                r >= shield_rank - 1
                    && board
                        .get(Square::new(f as u8, r))
                        .is_some_and(|p| p.kind == PieceKind::Pawn && p.color == Color::Black)
            });
            if !has_shield {
                b_mg += KING_PAWN_SHIELD_PENALTY_MG;
            }

            if white_pawns_per_file[f] == 0 && black_pawns_per_file[f] == 0 {
                b_mg += KING_OPEN_FILE_PENALTY_MG;
            }
        }

        for dr in -2..=2i8 {
            for df in -2..=2i8 {
                if dr == 0 && df == 0 {
                    continue;
                }
                if let Some(sq) = bk.offset(df, dr)
                    && let Some(piece) = board.get(sq)
                    && piece.color == Color::White
                    && piece.kind != PieceKind::Pawn
                {
                    b_mg += KING_TROPISM_PENALTY_MG;
                }
            }
        }
    }

    (w_mg, b_mg)
}

/// Evaluates piece mobility for both sides.
///
/// Counts the number of pseudo-legal squares each piece can reach.
/// Returns `(white_mg, white_eg, black_mg, black_eg)`.
fn mobility_score(board: &Board) -> (i32, i32, i32, i32) {
    let mut w_mg = 0i32;
    let mut w_eg = 0i32;
    let mut b_mg = 0i32;
    let mut b_eg = 0i32;

    let knight_offsets: [(i8, i8); 8] = [
        (-2, -1),
        (-2, 1),
        (-1, -2),
        (-1, 2),
        (1, -2),
        (1, 2),
        (2, -1),
        (2, 1),
    ];
    let bishop_dirs: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
    let rook_dirs: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    for rank in 0..8u8 {
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            let Some(piece) = board.get(sq) else { continue };

            let mut moves = 0i32;

            match piece.kind {
                PieceKind::Knight => {
                    for &(df, dr) in &knight_offsets {
                        if let Some(to) = sq.offset(df, dr) {
                            let blocked = board.get(to).is_some_and(|p| p.color == piece.color);
                            if !blocked {
                                moves += 1;
                            }
                        }
                    }
                    match piece.color {
                        Color::White => {
                            w_mg += moves * KNIGHT_MOBILITY_MG;
                            w_eg += moves * KNIGHT_MOBILITY_EG;
                        }
                        Color::Black => {
                            b_mg += moves * KNIGHT_MOBILITY_MG;
                            b_eg += moves * KNIGHT_MOBILITY_EG;
                        }
                    }
                }
                PieceKind::Bishop => {
                    for &(df, dr) in &bishop_dirs {
                        let mut cur = sq;
                        loop {
                            match cur.offset(df, dr) {
                                None => break,
                                Some(to) => {
                                    match board.get(to) {
                                        None => {
                                            moves += 1;
                                        }
                                        Some(p) if p.color != piece.color => {
                                            moves += 1;
                                            break;
                                        }
                                        _ => break,
                                    }
                                    cur = to;
                                }
                            }
                        }
                    }
                    match piece.color {
                        Color::White => {
                            w_mg += moves * BISHOP_MOBILITY_MG;
                            w_eg += moves * BISHOP_MOBILITY_EG;
                        }
                        Color::Black => {
                            b_mg += moves * BISHOP_MOBILITY_MG;
                            b_eg += moves * BISHOP_MOBILITY_EG;
                        }
                    }
                }
                PieceKind::Rook => {
                    for &(df, dr) in &rook_dirs {
                        let mut cur = sq;
                        loop {
                            match cur.offset(df, dr) {
                                None => break,
                                Some(to) => {
                                    match board.get(to) {
                                        None => {
                                            moves += 1;
                                        }
                                        Some(p) if p.color != piece.color => {
                                            moves += 1;
                                            break;
                                        }
                                        _ => break,
                                    }
                                    cur = to;
                                }
                            }
                        }
                    }
                    match piece.color {
                        Color::White => {
                            w_mg += moves * ROOK_MOBILITY_MG;
                            w_eg += moves * ROOK_MOBILITY_EG;
                        }
                        Color::Black => {
                            b_mg += moves * ROOK_MOBILITY_MG;
                            b_eg += moves * ROOK_MOBILITY_EG;
                        }
                    }
                }
                PieceKind::Queen => {
                    let all_dirs: [(i8, i8); 8] = [
                        (-1, -1),
                        (-1, 0),
                        (-1, 1),
                        (0, -1),
                        (0, 1),
                        (1, -1),
                        (1, 0),
                        (1, 1),
                    ];
                    for &(df, dr) in &all_dirs {
                        let mut cur = sq;
                        loop {
                            match cur.offset(df, dr) {
                                None => break,
                                Some(to) => {
                                    match board.get(to) {
                                        None => {
                                            moves += 1;
                                        }
                                        Some(p) if p.color != piece.color => {
                                            moves += 1;
                                            break;
                                        }
                                        _ => break,
                                    }
                                    cur = to;
                                }
                            }
                        }
                    }
                    match piece.color {
                        Color::White => {
                            w_mg += moves * QUEEN_MOBILITY_MG;
                            w_eg += moves * QUEEN_MOBILITY_EG;
                        }
                        Color::Black => {
                            b_mg += moves * QUEEN_MOBILITY_MG;
                            b_eg += moves * QUEEN_MOBILITY_EG;
                        }
                    }
                }
                _ => {} // King and Pawn mobility not scored
            }
        }
    }

    (w_mg, w_eg, b_mg, b_eg)
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
