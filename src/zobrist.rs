//! Internal Zobrist hashing for the CheckAI chess engine.
//!
//! Generates deterministic 64-bit Zobrist keys using the SplitMix64 PRNG
//! seeded with a fixed constant. Keys are used for:
//!
//! - Transposition table lookups during search
//!
//! **Note:** Three-fold repetition detection uses FEN string comparison
//! in `Game.position_history`, not Zobrist hashes.
//!
//! **Note:** These keys are intentionally separate from the standard
//! Polyglot Random64 table. Opening book lookups use the canonical
//! Polyglot keys from `polyglot_keys.rs` so that community-sourced
//! `.bin` books work out of the box.
//!
//! The key table contains 781 entries:
//! - `[0..767]`   — piece keys (`piece_index * 64 + square_index`)
//! - `[768..771]`  — castling rights keys (WK, WQ, BK, BQ)
//! - `[772..779]`  — en passant file keys (files a–h, only hashed
//!   when a pawn can actually capture)
//! - `[780]`       — side-to-move key (XORed when Black to move)

use crate::types::*;

// ---------------------------------------------------------------------------
// Compile-time key generation (SplitMix64 PRNG)
// ---------------------------------------------------------------------------

/// Seed for the SplitMix64 PRNG used to generate Zobrist keys.
const SEED: u64 = 0x12345678_9ABCDEF0;

/// One step of the SplitMix64 PRNG.
/// Returns `(next_state, output_value)`.
const fn splitmix64(state: u64) -> (u64, u64) {
    let s = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let z = (s ^ (s >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (s, z ^ (z >> 31))
}

/// Generates all 781 Zobrist keys at compile time.
const fn generate_zobrist_keys() -> [u64; 781] {
    let mut keys = [0u64; 781];
    let mut state = SEED;
    let mut i = 0;
    while i < 781 {
        let (new_state, value) = splitmix64(state);
        state = new_state;
        keys[i] = value;
        i += 1;
    }
    keys
}

/// Full Zobrist key table (computed at compile time).
const ZOBRIST_KEYS: [u64; 781] = generate_zobrist_keys();

// ---------------------------------------------------------------------------
// Piece-index mapping
// ---------------------------------------------------------------------------

/// Returns the Zobrist piece index (0–11) for a given piece.
///
/// The ordering mirrors the Polyglot convention for consistency,
/// but the actual key values are **not** the standard Polyglot
/// Random64 constants. For Polyglot book lookups see
/// `polyglot_keys.rs`.
///
/// | Index | Piece         |
/// |-------|---------------|
/// | 0     | Black Pawn    |
/// | 1     | White Pawn    |
/// | 2     | Black Knight  |
/// | 3     | White Knight  |
/// | 4     | Black Bishop  |
/// | 5     | White Bishop  |
/// | 6     | Black Rook    |
/// | 7     | White Rook    |
/// | 8     | Black Queen   |
/// | 9     | White Queen   |
/// | 10    | Black King    |
/// | 11    | White King    |
fn piece_zobrist_index(piece: &Piece) -> usize {
    let kind_base = match piece.kind {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 2,
        PieceKind::Bishop => 4,
        PieceKind::Rook => 6,
        PieceKind::Queen => 8,
        PieceKind::King => 10,
    };
    let color_offset = match piece.color {
        Color::Black => 0,
        Color::White => 1,
    };
    kind_base + color_offset
}

// ---------------------------------------------------------------------------
// En passant legality helper
// ---------------------------------------------------------------------------

/// Returns `true` if the side to move has a pawn adjacent to the en-passant
/// target square that could (geometrically) perform the capture.
///
/// Only the presence of an own pawn on an adjacent file on the correct rank
/// is checked — full move legality is not verified.
fn has_ep_capture_candidate(board: &Board, turn: Color, ep_sq: Square) -> bool {
    // The capturing pawn sits on the rank behind the EP target.
    let pawn_rank = match turn {
        Color::White => {
            if ep_sq.rank == 0 {
                return false;
            }
            ep_sq.rank - 1
        }
        Color::Black => {
            if ep_sq.rank >= 7 {
                return false;
            }
            ep_sq.rank + 1
        }
    };

    // Check left and right adjacent files.
    for df in [-1i8, 1i8] {
        let f = ep_sq.file as i8 + df;
        if (0..8).contains(&f) {
            let candidate_sq = Square::new(f as u8, pawn_rank);
            if let Some(piece) = board.get(candidate_sq)
                && piece.kind == PieceKind::Pawn
                && piece.color == turn
            {
                return true;
            }
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Computes the full Zobrist hash for a board position.
pub fn hash_position(
    board: &Board,
    turn: Color,
    castling: &CastlingRights,
    en_passant: Option<Square>,
) -> u64 {
    let mut hash = 0u64;

    // Piece-square keys
    for rank in 0..8u8 {
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            if let Some(piece) = board.get(sq) {
                let idx = piece_zobrist_index(&piece) * 64 + sq.index();
                hash ^= ZOBRIST_KEYS[idx];
            }
        }
    }

    // Castling rights
    if castling.white.kingside {
        hash ^= ZOBRIST_KEYS[768];
    }
    if castling.white.queenside {
        hash ^= ZOBRIST_KEYS[769];
    }
    if castling.black.kingside {
        hash ^= ZOBRIST_KEYS[770];
    }
    if castling.black.queenside {
        hash ^= ZOBRIST_KEYS[771];
    }

    // En passant file — only if a pawn can actually capture
    if let Some(ep_sq) = en_passant
        && has_ep_capture_candidate(board, turn, ep_sq)
    {
        hash ^= ZOBRIST_KEYS[772 + ep_sq.file as usize];
    }

    // Side to move (hash when Black to move)
    if turn == Color::Black {
        hash ^= ZOBRIST_KEYS[780];
    }

    hash
}

/// Returns the Zobrist key for a specific piece on a specific square.
pub fn piece_square_key(piece: &Piece, sq: Square) -> u64 {
    let idx = piece_zobrist_index(piece) * 64 + sq.index();
    ZOBRIST_KEYS[idx]
}

/// Returns the Zobrist key for a specific castling flag.
///
/// Index: 0=WK, 1=WQ, 2=BK, 3=BQ
pub fn castling_key(index: usize) -> u64 {
    debug_assert!(index < 4);
    ZOBRIST_KEYS[768 + index]
}

/// Returns the Zobrist key for an en passant file (0=a, 7=h).
pub fn en_passant_key(file: u8) -> u64 {
    debug_assert!(file < 8);
    ZOBRIST_KEYS[772 + file as usize]
}

/// Returns the side-to-move Zobrist key.
pub fn side_key() -> u64 {
    ZOBRIST_KEYS[780]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position_hash_is_nonzero() {
        let board = Board::starting_position();
        let hash = hash_position(&board, Color::White, &CastlingRights::default(), None);
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_hash_changes_with_turn() {
        let board = Board::starting_position();
        let castling = CastlingRights::default();
        let h1 = hash_position(&board, Color::White, &castling, None);
        let h2 = hash_position(&board, Color::Black, &castling, None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_deterministic() {
        let board = Board::starting_position();
        let castling = CastlingRights::default();
        let h1 = hash_position(&board, Color::White, &castling, None);
        let h2 = hash_position(&board, Color::White, &castling, None);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_zobrist_keys_unique() {
        // Check that all keys are distinct (no collisions in the table)
        let keys = &ZOBRIST_KEYS;
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(keys[i], keys[j], "Collision at indices {} and {}", i, j);
            }
        }
    }

    #[test]
    fn test_ep_ignored_when_no_capture_possible() {
        // On the starting position no pawn can capture en passant, so
        // providing an EP square should not change the hash.
        let board = Board::starting_position();
        let castling = CastlingRights::default();
        let ep_sq = Square::new(4, 5); // e6 — no white pawn on d5/f5
        let h_no_ep = hash_position(&board, Color::White, &castling, None);
        let h_ep = hash_position(&board, Color::White, &castling, Some(ep_sq));
        assert_eq!(
            h_no_ep, h_ep,
            "EP key should not be included when no pawn can capture"
        );
    }
}
