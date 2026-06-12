//! The single board renderer used by every CLI command.
//!
//! Renders a [`Board`] to a `String` with:
//!
//! - Unicode chess glyphs or plain ASCII letters (`--ascii`),
//! - last-move highlighting (from/to squares),
//! - check highlighting (king square),
//! - optional coordinates,
//! - optional flipped view for playing Black (`--flip`).
//!
//! Colors degrade to plain text automatically through the global
//! `colored` override managed by [`super::theme::Theme`].

use colored::Colorize;

use crate::game::Game;
use crate::movegen;
use crate::types::{Board, Color, Piece, PieceKind, Square};

/// Squares to visually emphasize when rendering a board.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoardHighlights {
    /// From/to squares of the most recent move.
    pub last_move: Option<(Square, Square)>,
    /// Square of a king currently in check.
    pub check: Option<Square>,
}

impl BoardHighlights {
    /// Derives highlights from a live game: last move played and the
    /// side-to-move's king square when in check.
    pub fn for_game(game: &Game) -> Self {
        let last_move = game.move_history.last().and_then(|record| {
            let from = Square::from_algebraic(&record.move_json.from)?;
            let to = Square::from_algebraic(&record.move_json.to)?;
            Some((from, to))
        });
        let check = if movegen::is_in_check(&game.board, game.turn) {
            game.board.find_king(game.turn)
        } else {
            None
        };
        Self { last_move, check }
    }
}

/// Stateless board renderer with presentation options.
#[derive(Debug, Clone, Copy)]
pub struct BoardRenderer {
    /// Use ASCII piece letters instead of Unicode glyphs.
    pub ascii: bool,
    /// Render from Black's perspective (rank 1 at the top).
    pub flipped: bool,
    /// Print file/rank coordinates around the board.
    pub coords: bool,
}

impl Default for BoardRenderer {
    fn default() -> Self {
        Self {
            ascii: false,
            flipped: false,
            coords: true,
        }
    }
}

impl BoardRenderer {
    /// Creates a renderer with the given glyph mode and orientation.
    pub fn new(ascii: bool, flipped: bool) -> Self {
        Self {
            ascii,
            flipped,
            coords: true,
        }
    }

    /// Renders the board to a multi-line string.
    pub fn render(&self, board: &Board, highlights: &BoardHighlights) -> String {
        let mut out = String::new();
        let separator = "  +---+---+---+---+---+---+---+---+\n";

        out.push('\n');
        out.push_str(separator);

        for display_rank in 0..8u8 {
            let rank = if self.flipped {
                display_rank
            } else {
                7 - display_rank
            };
            if self.coords {
                out.push_str(&format!("{} ", rank + 1));
            } else {
                out.push_str("  ");
            }
            for display_file in 0..8u8 {
                let file = if self.flipped {
                    7 - display_file
                } else {
                    display_file
                };
                let sq = Square::new(file, rank);
                out.push('|');
                out.push_str(&self.render_cell(board, sq, highlights));
            }
            out.push_str("|\n");
            out.push_str(separator);
        }

        if self.coords {
            let files: String = (0..8u8)
                .map(|display_file| {
                    let file = if self.flipped {
                        7 - display_file
                    } else {
                        display_file
                    };
                    format!("  {} ", (b'a' + file) as char)
                })
                .collect();
            out.push_str(&format!("  {files}\n"));
        }
        out
    }

    /// Renders one 3-character cell (` X `), applying highlights.
    fn render_cell(&self, board: &Board, sq: Square, highlights: &BoardHighlights) -> String {
        let is_dark = (sq.file + sq.rank).is_multiple_of(2);
        let glyph = match board.get(sq) {
            Some(piece) => {
                let symbol = self.piece_symbol(piece);
                if piece.color == Color::White {
                    symbol.white().bold().to_string()
                } else {
                    symbol.blue().bold().to_string()
                }
            }
            None if is_dark => "·".dimmed().to_string(),
            None => " ".to_string(),
        };

        let cell = format!(" {glyph} ");
        if highlights.check == Some(sq) {
            cell.on_red().to_string()
        } else if highlights
            .last_move
            .is_some_and(|(from, to)| from == sq || to == sq)
        {
            cell.on_yellow().black().to_string()
        } else {
            cell
        }
    }

    /// Returns the display symbol for a piece in the configured glyph mode.
    fn piece_symbol(&self, piece: Piece) -> &'static str {
        if self.ascii {
            ascii_symbol(piece)
        } else {
            unicode_symbol(piece)
        }
    }
}

/// ASCII (FEN-letter) symbol for a piece.
fn ascii_symbol(piece: Piece) -> &'static str {
    match (piece.color, piece.kind) {
        (Color::White, PieceKind::King) => "K",
        (Color::White, PieceKind::Queen) => "Q",
        (Color::White, PieceKind::Rook) => "R",
        (Color::White, PieceKind::Bishop) => "B",
        (Color::White, PieceKind::Knight) => "N",
        (Color::White, PieceKind::Pawn) => "P",
        (Color::Black, PieceKind::King) => "k",
        (Color::Black, PieceKind::Queen) => "q",
        (Color::Black, PieceKind::Rook) => "r",
        (Color::Black, PieceKind::Bishop) => "b",
        (Color::Black, PieceKind::Knight) => "n",
        (Color::Black, PieceKind::Pawn) => "p",
    }
}

/// Unicode chess glyph for a piece. White uses outline glyphs, Black the
/// filled set; sides are additionally distinguished by color.
fn unicode_symbol(piece: Piece) -> &'static str {
    match (piece.color, piece.kind) {
        (Color::White, PieceKind::King) => "♔",
        (Color::White, PieceKind::Queen) => "♕",
        (Color::White, PieceKind::Rook) => "♖",
        (Color::White, PieceKind::Bishop) => "♗",
        (Color::White, PieceKind::Knight) => "♘",
        (Color::White, PieceKind::Pawn) => "♙",
        (Color::Black, PieceKind::King) => "♚",
        (Color::Black, PieceKind::Queen) => "♛",
        (Color::Black, PieceKind::Rook) => "♜",
        (Color::Black, PieceKind::Bishop) => "♝",
        (Color::Black, PieceKind::Knight) => "♞",
        (Color::Black, PieceKind::Pawn) => "♟",
    }
}

/// Returns the localized human-readable name of a piece kind.
pub fn piece_name(kind: PieceKind) -> String {
    match kind {
        PieceKind::King => t!("play.piece_king").to_string(),
        PieceKind::Queen => t!("play.piece_queen").to_string(),
        PieceKind::Rook => t!("play.piece_rook").to_string(),
        PieceKind::Bishop => t!("play.piece_bishop").to_string(),
        PieceKind::Knight => t!("play.piece_knight").to_string(),
        PieceKind::Pawn => t!("play.piece_pawn").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain_render(renderer: BoardRenderer) -> String {
        colored::control::set_override(false);
        renderer.render(&Board::starting_position(), &BoardHighlights::default())
    }

    #[test]
    fn test_ascii_render_contains_pieces_and_coords() {
        let out = plain_render(BoardRenderer::new(true, false));
        assert!(out.contains("K"), "white king letter expected");
        assert!(out.contains("k"), "black king letter expected");
        assert!(out.contains("a   b   c"), "file coordinates expected");
        // White perspective: rank 8 (black pieces) is rendered first.
        let r8 = out.find("8 ").expect("rank 8 label");
        let r1 = out.find("1 ").expect("rank 1 label");
        assert!(r8 < r1, "rank 8 must precede rank 1 in white view");
    }

    #[test]
    fn test_flipped_render_reverses_ranks() {
        let out = plain_render(BoardRenderer::new(true, true));
        let r8 = out.find("8 ").expect("rank 8 label");
        let r1 = out.find("1 ").expect("rank 1 label");
        assert!(r1 < r8, "rank 1 must precede rank 8 in flipped view");
        assert!(out.contains("h   g   f"), "files must be reversed");
    }

    #[test]
    fn test_unicode_render_uses_glyphs() {
        let out = plain_render(BoardRenderer::new(false, false));
        assert!(out.contains('♔'));
        assert!(out.contains('♚'));
    }
}
