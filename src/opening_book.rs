//! Polyglot opening book reader for the CheckAI analysis engine.
//!
//! Supports the standard Polyglot `.bin` format:
//!
//! Each entry is 16 bytes:
//! - `[0..8]`   — 64-bit Zobrist hash (big-endian)
//! - `[8..10]`  — Encoded move (big-endian u16)
//! - `[10..12]` — Weight / frequency (big-endian u16)
//! - `[12..16]` — Learn data (big-endian u32, unused)
//!
//! The file is sorted by hash key, allowing binary search lookups.
//!
//! Position hashing uses the **standard Polyglot Random64 keys** from
//! `polyglot_keys.rs`, so any community-sourced `.bin` book that
//! conforms to the Polyglot specification will work.
//!
//! Castling moves are decoded correctly (king-to-rook-square is
//! remapped to the standard king destination).

use std::fs;
use std::path::{Path, PathBuf};

use crate::polyglot_keys;
use crate::types::*;

// ---------------------------------------------------------------------------
// Book entry
// ---------------------------------------------------------------------------

/// A single entry from a Polyglot opening book.
#[derive(Debug, Clone)]
pub struct BookEntry {
    /// Zobrist hash of the position.
    pub key: u64,
    /// The move in engine notation.
    pub chess_move: ChessMove,
    /// Weight (frequency / strength indicator).
    pub weight: u16,
    /// Win/draw/loss statistics (if available).
    pub learn: u32,
}

/// Information about a book move for the analysis output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct BookMoveInfo {
    /// Whether the played move is a known book move.
    pub is_book_move: bool,
    /// Weight of the played move (0 if not a book move).
    pub weight: u16,
    /// Total weight of all book moves for this position.
    ///
    /// **Breaking change (v0.3.1):** widened from `u16` to `u32` to prevent
    /// wraparound when multiple high-weight entries are present.
    pub total_weight: u32,
    /// All book moves available in this position.
    pub book_moves: Vec<BookMoveEntry>,
    /// Name of the opening line (if known).
    pub opening_name: Option<String>,
}

/// A single move from the opening book with statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct BookMoveEntry {
    /// The move in algebraic notation (e.g. "e2e4").
    pub notation: String,
    /// Weight / frequency.
    pub weight: u16,
    /// Relative probability (weight / total_weight).
    pub probability: f64,
}

// ---------------------------------------------------------------------------
// Opening book
// ---------------------------------------------------------------------------

/// An opening book loaded from a Polyglot `.bin` file.
pub struct OpeningBook {
    /// All book entries, sorted by key for binary search.
    entries: Vec<RawBookEntry>,
    /// Path the book was loaded from.
    pub path: PathBuf,
}

/// Raw 16-byte book entry (before move decoding).
#[derive(Debug, Clone, Copy)]
struct RawBookEntry {
    key: u64,
    raw_move: u16,
    weight: u16,
    learn: u32,
}

impl OpeningBook {
    /// Loads a Polyglot opening book from the given file path.
    ///
    /// Returns `Err` if the file cannot be read or has an invalid format.
    pub fn load(path: &Path) -> Result<Self, String> {
        let data = fs::read(path).map_err(|e| format!("Failed to read book file: {}", e))?;

        if data.len() % 16 != 0 {
            return Err(format!(
                "Invalid book file size: {} (must be a multiple of 16)",
                data.len()
            ));
        }

        let num_entries = data.len() / 16;
        let mut entries = Vec::with_capacity(num_entries);

        for i in 0..num_entries {
            let offset = i * 16;
            let key = u64::from_be_bytes(data[offset..offset + 8].try_into().unwrap());
            let raw_move = u16::from_be_bytes(data[offset + 8..offset + 10].try_into().unwrap());
            let weight = u16::from_be_bytes(data[offset + 10..offset + 12].try_into().unwrap());
            let learn = u32::from_be_bytes(data[offset + 12..offset + 16].try_into().unwrap());

            entries.push(RawBookEntry {
                key,
                raw_move,
                weight,
                learn,
            });
        }

        log::info!(
            "Loaded opening book with {} entries from {}",
            num_entries,
            path.display()
        );

        Ok(Self {
            entries,
            path: path.to_path_buf(),
        })
    }

    /// Looks up all book moves for the given position.
    ///
    /// Returns an empty vec if the position is not in the book.
    pub fn lookup(
        &self,
        board: &Board,
        turn: Color,
        castling: &CastlingRights,
        en_passant: Option<Square>,
    ) -> Vec<BookEntry> {
        let key = polyglot_keys::polyglot_hash(board, turn, castling, en_passant);
        self.lookup_by_key(key)
    }

    /// Looks up all book entries for a given Zobrist key.
    fn lookup_by_key(&self, key: u64) -> Vec<BookEntry> {
        // Binary search for the first entry with matching key
        let start = self.entries.partition_point(|e| e.key < key);

        let mut results = Vec::new();
        for i in start..self.entries.len() {
            let entry = &self.entries[i];
            if entry.key != key {
                break;
            }
            if let Some(chess_move) = decode_polyglot_move(entry.raw_move) {
                results.push(BookEntry {
                    key: entry.key,
                    chess_move,
                    weight: entry.weight,
                    learn: entry.learn,
                });
            }
        }

        results
    }

    /// Probes the book for the given position and returns information
    /// about whether a specific move is a book move.
    pub fn probe_move(
        &self,
        board: &Board,
        turn: Color,
        castling: &CastlingRights,
        en_passant: Option<Square>,
        played_move: &ChessMove,
    ) -> BookMoveInfo {
        let entries = self.lookup(board, turn, castling, en_passant);

        if entries.is_empty() {
            return BookMoveInfo {
                is_book_move: false,
                weight: 0,
                total_weight: 0,
                book_moves: Vec::new(),
                opening_name: None,
            };
        }

        let total_weight: u32 = entries.iter().map(|e| e.weight as u32).sum();
        let mut is_book_move = false;
        let mut played_weight = 0u16;

        let book_moves: Vec<BookMoveEntry> = entries
            .iter()
            .map(|e| {
                let matches = e.chess_move.from == played_move.from
                    && e.chess_move.to == played_move.to
                    && e.chess_move.promotion == played_move.promotion;
                if matches {
                    is_book_move = true;
                    played_weight = e.weight;
                }
                BookMoveEntry {
                    notation: e.chess_move.to_string(),
                    weight: e.weight,
                    probability: if total_weight > 0 {
                        e.weight as f64 / total_weight as f64
                    } else {
                        0.0
                    },
                }
            })
            .collect();

        BookMoveInfo {
            is_book_move,
            weight: played_weight,
            total_weight,
            book_moves,
            opening_name: None, // Could be extended with an ECO database
        }
    }

    /// Returns the number of entries in the book.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the book has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Polyglot move decoding
// ---------------------------------------------------------------------------

/// Decodes a Polyglot raw move (u16) into a `ChessMove`.
///
/// Polyglot move encoding:
/// - Bits 0–2:  to file (0=a, 7=h)
/// - Bits 3–5:  to rank (0=rank 1, 7=rank 8)
/// - Bits 6–8:  from file
/// - Bits 9–11: from rank
/// - Bits 12–14: promotion piece (0=none, 1=knight, 2=bishop, 3=rook, 4=queen)
fn decode_polyglot_move(raw: u16) -> Option<ChessMove> {
    let to_file = (raw & 0x07) as u8;
    let to_rank = ((raw >> 3) & 0x07) as u8;
    let from_file = ((raw >> 6) & 0x07) as u8;
    let from_rank = ((raw >> 9) & 0x07) as u8;
    let promo = ((raw >> 12) & 0x07) as u8;

    if from_file >= 8 || from_rank >= 8 || to_file >= 8 || to_rank >= 8 {
        return None;
    }

    let from = Square::new(from_file, from_rank);
    let to = Square::new(to_file, to_rank);

    let promotion = match promo {
        0 => None,
        1 => Some(PieceKind::Knight),
        2 => Some(PieceKind::Bishop),
        3 => Some(PieceKind::Rook),
        4 => Some(PieceKind::Queen),
        _ => return None,
    };

    // Polyglot encodes castling as king-to-rook-square.
    // Detect this and remap to the standard king destination.
    let (adjusted_to, is_castling) = adjust_castling_move(from, to);
    let is_en_passant = false;

    Some(ChessMove {
        from,
        to: adjusted_to,
        promotion,
        is_castling,
        is_en_passant,
    })
}

/// Adjusts a Polyglot castling move (king-to-rook) to the standard
/// king destination (e.g. e1h1 → e1g1 for white kingside castling).
///
/// Returns `(adjusted_to, is_castling)`.
fn adjust_castling_move(from: Square, to: Square) -> (Square, bool) {
    // White kingside: e1 → h1 (rook) → g1 (king destination)
    if from.file == 4 && from.rank == 0 && to.file == 7 && to.rank == 0 {
        return (Square::new(6, 0), true);
    }
    // White queenside: e1 → a1 (rook) → c1
    if from.file == 4 && from.rank == 0 && to.file == 0 && to.rank == 0 {
        return (Square::new(2, 0), true);
    }
    // Black kingside: e8 → h8 (rook) → g8
    if from.file == 4 && from.rank == 7 && to.file == 7 && to.rank == 7 {
        return (Square::new(6, 7), true);
    }
    // Black queenside: e8 → a8 (rook) → c8
    if from.file == 4 && from.rank == 7 && to.file == 0 && to.rank == 7 {
        return (Square::new(2, 7), true);
    }
    (to, false)
}

/// Encodes a `ChessMove` into the Polyglot raw move format.
///
/// Castling moves are mapped back to king-to-rook-square encoding
/// (the inverse of the adjustment performed during decoding).
pub fn encode_polyglot_move(mv: &ChessMove) -> u16 {
    // For castling moves, Polyglot expects king → rook square.
    let to = if mv.is_castling {
        unadjust_castling_move(mv.from, mv.to)
    } else {
        mv.to
    };

    let mut raw: u16 = 0;
    raw |= to.file as u16;
    raw |= (to.rank as u16) << 3;
    raw |= (mv.from.file as u16) << 6;
    raw |= (mv.from.rank as u16) << 9;

    let promo = match mv.promotion {
        None => 0u16,
        Some(PieceKind::Knight) => 1,
        Some(PieceKind::Bishop) => 2,
        Some(PieceKind::Rook) => 3,
        Some(PieceKind::Queen) => 4,
        _ => 0,
    };
    raw |= promo << 12;

    raw
}

/// Maps a standard castling king destination back to the rook square
/// for Polyglot encoding (inverse of `adjust_castling_move`).
fn unadjust_castling_move(from: Square, to: Square) -> Square {
    // White kingside: e1→g1 back to e1→h1
    if from.file == 4 && from.rank == 0 && to.file == 6 && to.rank == 0 {
        return Square::new(7, 0);
    }
    // White queenside: e1→c1 back to e1→a1
    if from.file == 4 && from.rank == 0 && to.file == 2 && to.rank == 0 {
        return Square::new(0, 0);
    }
    // Black kingside: e8→g8 back to e8→h8
    if from.file == 4 && from.rank == 7 && to.file == 6 && to.rank == 7 {
        return Square::new(7, 7);
    }
    // Black queenside: e8→c8 back to e8→a8
    if from.file == 4 && from.rank == 7 && to.file == 2 && to.rank == 7 {
        return Square::new(0, 7);
    }
    to
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let mv = ChessMove::simple(Square::new(4, 1), Square::new(4, 3)); // e2-e4
        let encoded = encode_polyglot_move(&mv);
        let decoded = decode_polyglot_move(encoded).unwrap();
        assert_eq!(decoded.from, mv.from);
        assert_eq!(decoded.to, mv.to);
        assert_eq!(decoded.promotion, None);
    }

    #[test]
    fn test_encode_decode_promotion() {
        let mv = ChessMove {
            from: Square::new(4, 6), // e7
            to: Square::new(4, 7),   // e8
            promotion: Some(PieceKind::Queen),
            is_castling: false,
            is_en_passant: false,
        };
        let encoded = encode_polyglot_move(&mv);
        let decoded = decode_polyglot_move(encoded).unwrap();
        assert_eq!(decoded.from, mv.from);
        assert_eq!(decoded.to, mv.to);
        assert_eq!(decoded.promotion, Some(PieceKind::Queen));
    }

    #[test]
    fn test_encode_decode_castling_white_kingside() {
        let mv = ChessMove {
            from: Square::new(4, 0), // e1
            to: Square::new(6, 0),   // g1 (standard king destination)
            promotion: None,
            is_castling: true,
            is_en_passant: false,
        };
        let encoded = encode_polyglot_move(&mv);
        let decoded = decode_polyglot_move(encoded).unwrap();
        assert_eq!(decoded.from, mv.from);
        assert_eq!(decoded.to, mv.to);
        assert!(decoded.is_castling);
    }

    #[test]
    fn test_encode_decode_castling_white_queenside() {
        let mv = ChessMove {
            from: Square::new(4, 0), // e1
            to: Square::new(2, 0),   // c1
            promotion: None,
            is_castling: true,
            is_en_passant: false,
        };
        let encoded = encode_polyglot_move(&mv);
        let decoded = decode_polyglot_move(encoded).unwrap();
        assert_eq!(decoded.from, mv.from);
        assert_eq!(decoded.to, mv.to);
        assert!(decoded.is_castling);
    }

    #[test]
    fn test_encode_decode_castling_black_kingside() {
        let mv = ChessMove {
            from: Square::new(4, 7), // e8
            to: Square::new(6, 7),   // g8
            promotion: None,
            is_castling: true,
            is_en_passant: false,
        };
        let encoded = encode_polyglot_move(&mv);
        let decoded = decode_polyglot_move(encoded).unwrap();
        assert_eq!(decoded.from, mv.from);
        assert_eq!(decoded.to, mv.to);
        assert!(decoded.is_castling);
    }

    #[test]
    fn test_encode_decode_castling_black_queenside() {
        let mv = ChessMove {
            from: Square::new(4, 7), // e8
            to: Square::new(2, 7),   // c8
            promotion: None,
            is_castling: true,
            is_en_passant: false,
        };
        let encoded = encode_polyglot_move(&mv);
        let decoded = decode_polyglot_move(encoded).unwrap();
        assert_eq!(decoded.from, mv.from);
        assert_eq!(decoded.to, mv.to);
        assert!(decoded.is_castling);
    }

    #[test]
    fn test_total_weight_no_overflow() {
        // Creates two entries for the starting position, each with weight 40_000.
        // Their sum (80_000) exceeds u16::MAX (65_535); the u32 field must
        // hold the full value without wrapping.
        let board = Board::starting_position();
        let castling = CastlingRights::default();
        let key = polyglot_keys::polyglot_hash(&board, Color::White, &castling, None);

        let e2e4 = encode_polyglot_move(&ChessMove::simple(Square::new(4, 1), Square::new(4, 3)));
        let d2d4 = encode_polyglot_move(&ChessMove::simple(Square::new(3, 1), Square::new(3, 3)));

        let book = OpeningBook {
            entries: vec![
                RawBookEntry { key, raw_move: e2e4, weight: 40_000, learn: 0 },
                RawBookEntry { key, raw_move: d2d4, weight: 40_000, learn: 0 },
            ],
            path: PathBuf::from("test.bin"),
        };

        let played = ChessMove::simple(Square::new(4, 1), Square::new(4, 3)); // e2e4
        let info = book.probe_move(&board, Color::White, &castling, None, &played);

        assert_eq!(info.total_weight, 80_000u32, "total_weight must not overflow u16");
        assert!(info.is_book_move);
    }

    #[test]
    fn test_empty_book_lookup() {
        // Create a minimal book with no entries
        let book = OpeningBook {
            entries: Vec::new(),
            path: PathBuf::from("empty.bin"),
        };
        let board = Board::starting_position();
        let entries = book.lookup(&board, Color::White, &CastlingRights::default(), None);
        assert!(entries.is_empty());
    }
}
