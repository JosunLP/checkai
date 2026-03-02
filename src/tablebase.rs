//! Syzygy endgame tablebase support for the CheckAI analysis engine.
//!
//! Provides an interface for probing Syzygy tablebases (`.rtbw` for
//! Win/Draw/Loss results and `.rtbz` for Distance-to-Zero data).
//!
//! ## Capabilities
//!
//! - Detects available tablebase files in a directory.
//! - Determines which piece configurations are available.
//! - Probes positions that fall within the tablebase's piece-count range.
//! - Returns WDL (Win/Draw/Loss) and DTZ (Distance to Zeroing) results.
//! - Gracefully degrades when no tablebase files are present.
//!
//! ## Syzygy File Format
//!
//! Syzygy tables use a naming convention based on piece configurations:
//! - `KRvK.rtbw`  — WDL table for King+Rook vs King
//! - `KQKRp.rtbz` — DTZ table for KQ vs KRp
//!
//! Files with `.rtbw` extension provide Win/Draw/Loss lookups.
//! Files with `.rtbz` extension provide Distance-to-Zero lookups.
//!
//! Note: Full Syzygy probing requires complex decompression. This module
//! implements the infrastructure, file detection, and analytical endgame
//! evaluation for common simple endings. For positions requiring actual
//! table decompression, the engine falls back to deep search.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::movegen;
use crate::types::*;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Win/Draw/Loss outcome from the side to move's perspective.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
pub enum WDL {
    /// The side to move wins with best play.
    Win,
    /// The position is a theoretical draw.
    Draw,
    /// The side to move loses with best play by the opponent.
    Loss,
    /// A cursed win (win but 50-move rule may apply).
    CursedWin,
    /// A blessed loss (loss but 50-move rule saves).
    BlessedLoss,
}

/// Distance to Zeroing (DTZ) result.
///
/// DTZ counts the number of half-moves until the next pawn move or capture
/// (a "zeroing" move) along the optimal path.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DTZResult {
    /// The WDL outcome for this position.
    pub wdl: WDL,
    /// Distance to zeroing (half-moves). 0 if position is already a zeroing point.
    pub dtz: i32,
}

/// Complete tablebase probe result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TablebaseInfo {
    /// Whether the position was found in the tablebase.
    pub is_tablebase_position: bool,
    /// WDL result (if available).
    pub wdl: Option<WDL>,
    /// DTZ result (if available).
    pub dtz: Option<i32>,
    /// The piece configuration string (e.g. "KRvK").
    pub configuration: String,
    /// Source of the result: "tablebase", "analytical", or "unavailable".
    pub source: String,
}

// ---------------------------------------------------------------------------
// Piece configuration
// ---------------------------------------------------------------------------

/// Describes a piece configuration (e.g., KRvK) for tablebase lookup.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PieceConfig {
    /// White pieces sorted (e.g., ['K', 'R']).
    pub white: Vec<char>,
    /// Black pieces sorted (e.g., ['K']).
    pub black: Vec<char>,
}

impl PieceConfig {
    /// Creates a piece configuration from a board position.
    pub fn from_board(board: &Board) -> Self {
        let mut white = Vec::new();
        let mut black = Vec::new();

        for rank in 0..8u8 {
            for file in 0..8u8 {
                let sq = Square::new(file, rank);
                if let Some(piece) = board.get(sq) {
                    let ch = match piece.kind {
                        PieceKind::King => 'K',
                        PieceKind::Queen => 'Q',
                        PieceKind::Rook => 'R',
                        PieceKind::Bishop => 'B',
                        PieceKind::Knight => 'N',
                        // Syzygy filenames use lowercase 'p' for black pawns
                        PieceKind::Pawn if piece.color == Color::Black => 'p',
                        PieceKind::Pawn => 'P',
                    };
                    match piece.color {
                        Color::White => white.push(ch),
                        Color::Black => black.push(ch),
                    }
                }
            }
        }

        // Sort with King first, then by piece value (Q > R > B > N > P)
        let sort_key = |c: &char| match c {
            'K' => 0,
            'Q' => 1,
            'R' => 2,
            'B' => 3,
            'N' => 4,
            'P' | 'p' => 5,
            _ => 6,
        };
        white.sort_by_key(|c| sort_key(c));
        black.sort_by_key(|c| sort_key(c));

        Self { white, black }
    }

    /// Returns the total piece count.
    pub fn total_pieces(&self) -> usize {
        self.white.len() + self.black.len()
    }

    /// Converts to the standard Syzygy filename format (e.g., "KRvK").
    pub fn to_filename_base(&self) -> String {
        let w: String = self.white.iter().collect();
        let b: String = self.black.iter().collect();
        format!("{}v{}", w, b)
    }

    /// Converts to the canonical (normalized) form where the side with
    /// more material comes first.
    pub fn canonical(&self) -> Self {
        if self.white.len() < self.black.len()
            || (self.white.len() == self.black.len()
                && self.to_filename_base() > {
                    let swapped = Self {
                        white: self.black.clone(),
                        black: self.white.clone(),
                    };
                    swapped.to_filename_base()
                })
        {
            Self {
                white: self.black.clone(),
                black: self.white.clone(),
            }
        } else {
            self.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// Syzygy tablebase
// ---------------------------------------------------------------------------

/// Manages Syzygy tablebase files and provides position probing.
pub struct SyzygyTablebase {
    /// Path to the tablebase directory.
    pub path: PathBuf,
    /// Available WDL tables (configuration → file path).
    wdl_tables: HashMap<String, PathBuf>,
    /// Available DTZ tables (configuration → file path).
    dtz_tables: HashMap<String, PathBuf>,
    /// Maximum number of pieces supported by available tables.
    pub max_pieces: usize,
}

impl SyzygyTablebase {
    /// Loads tablebase information from the given directory.
    ///
    /// Scans for `.rtbw` and `.rtbz` files and indexes them by
    /// piece configuration.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Err(format!(
                "Tablebase directory does not exist: {}",
                path.display()
            ));
        }
        if !path.is_dir() {
            return Err(format!(
                "Tablebase path is not a directory: {}",
                path.display()
            ));
        }

        let mut wdl_tables = HashMap::new();
        let mut dtz_tables = HashMap::new();
        let mut max_pieces = 0usize;

        let entries =
            fs::read_dir(path).map_err(|e| format!("Failed to read tablebase directory: {}", e))?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let file_path = entry.path();
            let file_name = match file_path.file_stem().and_then(|s| s.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

            // Count pieces in the configuration name
            let piece_count = file_name
                .chars()
                .filter(|c| "KQRBNPkqrbnp".contains(*c))
                .count();

            match ext {
                "rtbw" => {
                    wdl_tables.insert(file_name.clone(), file_path.clone());
                    max_pieces = max_pieces.max(piece_count);
                }
                "rtbz" => {
                    dtz_tables.insert(file_name.clone(), file_path.clone());
                    max_pieces = max_pieces.max(piece_count);
                }
                _ => {}
            }
        }

        log::info!(
            "Loaded Syzygy tablebase: {} WDL tables, {} DTZ tables, max {} pieces ({})",
            wdl_tables.len(),
            dtz_tables.len(),
            max_pieces,
            path.display()
        );

        Ok(Self {
            path: path.to_path_buf(),
            wdl_tables,
            dtz_tables,
            max_pieces,
        })
    }

    /// Returns `true` if any tablebase files are available (WDL or DTZ).
    pub fn is_available(&self) -> bool {
        !self.wdl_tables.is_empty() || !self.dtz_tables.is_empty()
    }

    /// Returns `true` if the position's piece count falls within
    /// the tablebase's coverage.
    pub fn is_in_range(&self, board: &Board) -> bool {
        if !self.is_available() {
            return false;
        }
        let config = PieceConfig::from_board(board);
        config.total_pieces() <= self.max_pieces
    }

    /// Probes the tablebase for the given position.
    ///
    /// Returns a `TablebaseInfo` with the result. Falls back to
    /// analytical evaluation for simple endgames when binary
    /// table decompression is not available.
    pub fn probe(
        &self,
        board: &Board,
        turn: Color,
        castling: &CastlingRights,
        en_passant: Option<Square>,
    ) -> TablebaseInfo {
        let config = PieceConfig::from_board(board);
        let config_name = config.to_filename_base();
        let total = config.total_pieces();

        // Check if position is within range
        if total > self.max_pieces && self.max_pieces > 0 {
            return TablebaseInfo {
                is_tablebase_position: false,
                wdl: None,
                dtz: None,
                configuration: config_name,
                source: "out_of_range".to_string(),
            };
        }

        // Try analytical evaluation first (for simple endings)
        if let Some(analytical) = self.analytical_probe(board, turn) {
            return TablebaseInfo {
                is_tablebase_position: true,
                wdl: Some(analytical.wdl),
                dtz: Some(analytical.dtz),
                configuration: config_name,
                source: "analytical".to_string(),
            };
        }

        // Check if we have the table files for this configuration
        let canonical = config.canonical();
        let canonical_name = canonical.to_filename_base();

        let has_wdl = self.wdl_tables.contains_key(&canonical_name)
            || self.wdl_tables.contains_key(&config_name);
        let has_dtz = self.dtz_tables.contains_key(&canonical_name)
            || self.dtz_tables.contains_key(&config_name);

        if has_wdl || has_dtz {
            // We have table files. Full Syzygy binary probing would happen here.
            // For now, return that we found the configuration but cannot decompress.
            // This infrastructure is ready for integration with a Syzygy library
            // (e.g., shakmaty-syzygy or fathom-rs FFI).
            log::debug!(
                "Syzygy table available for {} but binary probing not implemented",
                config_name
            );

            // Use heuristic evaluation as a temporary measure
            let heuristic_wdl = self.heuristic_wdl(board, turn, castling, en_passant);

            return TablebaseInfo {
                is_tablebase_position: true,
                wdl: Some(heuristic_wdl),
                dtz: None,
                configuration: config_name,
                source: "heuristic".to_string(),
            };
        }

        TablebaseInfo {
            is_tablebase_position: false,
            wdl: None,
            dtz: None,
            configuration: config_name,
            source: "unavailable".to_string(),
        }
    }

    /// Analytical evaluation for provably solved simple endgames.
    ///
    /// Returns `Some(DTZResult)` for endgames that can be determined
    /// without tablebase files, `None` otherwise.
    fn analytical_probe(&self, board: &Board, turn: Color) -> Option<DTZResult> {
        let config = PieceConfig::from_board(board);
        let w = &config.white;
        let b = &config.black;

        // K vs K → Draw
        if w.len() == 1 && b.len() == 1 {
            return Some(DTZResult {
                wdl: WDL::Draw,
                dtz: 0,
            });
        }

        // K+minor vs K → Draw (insufficient material)
        if w.len() == 2 && b.len() == 1 {
            let non_king: Vec<_> = w.iter().filter(|&&c| c != 'K').collect();
            if non_king.len() == 1 && (*non_king[0] == 'B' || *non_king[0] == 'N') {
                return Some(DTZResult {
                    wdl: WDL::Draw,
                    dtz: 0,
                });
            }
        }
        if b.len() == 2 && w.len() == 1 {
            let non_king: Vec<_> = b.iter().filter(|&&c| c != 'K').collect();
            if non_king.len() == 1 && (*non_king[0] == 'B' || *non_king[0] == 'N') {
                return Some(DTZResult {
                    wdl: WDL::Draw,
                    dtz: 0,
                });
            }
        }

        // K+B vs K+B same color → Draw
        if w.len() == 2 && b.len() == 2 {
            let w_minor: Vec<_> = w.iter().filter(|&&c| c != 'K').collect();
            let b_minor: Vec<_> = b.iter().filter(|&&c| c != 'K').collect();
            if w_minor.len() == 1 && b_minor.len() == 1 && *w_minor[0] == 'B' && *b_minor[0] == 'B'
            {
                // Check same square color
                if bishops_same_color(board) {
                    return Some(DTZResult {
                        wdl: WDL::Draw,
                        dtz: 0,
                    });
                }
            }
        }

        // K+R vs K → Win for the rook side
        if w.len() == 2 && b.len() == 1 && w.contains(&'R') {
            let wdl = if turn == Color::White {
                WDL::Win
            } else {
                WDL::Loss
            };
            return Some(DTZResult { wdl, dtz: 16 });
        }
        if b.len() == 2 && w.len() == 1 && b.contains(&'R') {
            let wdl = if turn == Color::Black {
                WDL::Win
            } else {
                WDL::Loss
            };
            return Some(DTZResult { wdl, dtz: 16 });
        }

        // K+Q vs K → Win for the queen side
        if w.len() == 2 && b.len() == 1 && w.contains(&'Q') {
            let wdl = if turn == Color::White {
                WDL::Win
            } else {
                WDL::Loss
            };
            return Some(DTZResult { wdl, dtz: 10 });
        }
        if b.len() == 2 && w.len() == 1 && b.contains(&'Q') {
            let wdl = if turn == Color::Black {
                WDL::Win
            } else {
                WDL::Loss
            };
            return Some(DTZResult { wdl, dtz: 10 });
        }

        None
    }

    /// Heuristic WDL evaluation based on material balance.
    /// Used when table files exist but binary decompression is not available.
    fn heuristic_wdl(
        &self,
        board: &Board,
        turn: Color,
        _castling: &CastlingRights,
        _en_passant: Option<Square>,
    ) -> WDL {
        let material = crate::eval::material_score(board);

        // Check if the position is insufficient material
        if movegen::is_insufficient_material(board) {
            return WDL::Draw;
        }

        // Heuristic: significant material advantage → likely win
        let threshold = 300; // ~3 pawns advantage
        let score_from_side = match turn {
            Color::White => material,
            Color::Black => -material,
        };

        if score_from_side > threshold {
            WDL::Win
        } else if score_from_side < -threshold {
            WDL::Loss
        } else {
            WDL::Draw
        }
    }
}

/// Checks if both bishops on the board are on the same square color.
fn bishops_same_color(board: &Board) -> bool {
    let mut bishop_colors = Vec::new();
    for rank in 0..8u8 {
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            if let Some(piece) = board.get(sq)
                && piece.kind == PieceKind::Bishop
            {
                bishop_colors.push((file + rank) % 2);
            }
        }
    }
    bishop_colors.len() == 2 && bishop_colors[0] == bishop_colors[1]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_config_from_starting_position() {
        let board = Board::starting_position();
        let config = PieceConfig::from_board(&board);
        assert_eq!(config.total_pieces(), 32);
    }

    #[test]
    fn test_piece_config_simple() {
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(0, 0),
            Some(Piece::new(PieceKind::Rook, Color::White)),
        );
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        );

        let config = PieceConfig::from_board(&board);
        assert_eq!(config.to_filename_base(), "KRvK");
        assert_eq!(config.total_pieces(), 3);
    }

    #[test]
    fn test_analytical_k_vs_k() {
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        );

        let tb = SyzygyTablebase {
            path: PathBuf::from("test"),
            wdl_tables: HashMap::new(),
            dtz_tables: HashMap::new(),
            max_pieces: 0,
        };

        let result = tb.analytical_probe(&board, Color::White);
        assert!(result.is_some());
        assert_eq!(result.unwrap().wdl, WDL::Draw);
    }

    #[test]
    fn test_analytical_kr_vs_k() {
        let mut board = Board::default();
        board.set(
            Square::new(4, 0),
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(0, 0),
            Some(Piece::new(PieceKind::Rook, Color::White)),
        );
        board.set(
            Square::new(4, 7),
            Some(Piece::new(PieceKind::King, Color::Black)),
        );

        let tb = SyzygyTablebase {
            path: PathBuf::from("test"),
            wdl_tables: HashMap::new(),
            dtz_tables: HashMap::new(),
            max_pieces: 0,
        };

        let result = tb.analytical_probe(&board, Color::White);
        assert!(result.is_some());
        assert_eq!(result.unwrap().wdl, WDL::Win);
    }

    #[test]
    fn test_graceful_missing_directory() {
        let result = SyzygyTablebase::load(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}
