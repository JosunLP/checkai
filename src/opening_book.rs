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
//! Zobrist keys must match the book's key generation. This module uses
//! the same keys as `zobrist.rs`, so books must be generated with
//! matching keys. For maximum compatibility, the key table is
//! deterministic and reproducible.

use std::fs;
use std::path::{Path, PathBuf};

use crate::types::*;
use crate::zobrist;

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
    pub total_weight: u16,
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
        let key = zobrist::hash_position(board, turn, castling, en_passant);
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

        let total_weight: u16 = entries.iter().map(|e| e.weight).sum();
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

    // Castling in Polyglot: king moves to the rook's square
    // We need to adjust to standard king destination (g1/c1/g8/c8)
    let is_castling = false; // Will be set during move validation
    let is_en_passant = false;

    Some(ChessMove {
        from,
        to,
        promotion,
        is_castling,
        is_en_passant,
    })
}

/// Encodes a `ChessMove` into the Polyglot raw move format.
pub fn encode_polyglot_move(mv: &ChessMove) -> u16 {
    let mut raw: u16 = 0;
    raw |= mv.to.file as u16;
    raw |= (mv.to.rank as u16) << 3;
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
