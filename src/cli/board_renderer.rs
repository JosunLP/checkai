//! The single board renderer used by every CLI command.
//!
//! Renders a [`Board`] to a `String` in one of two styles:
//!
//! - [`BoardStyle::Squares`] — solid coloured squares (24-bit colour), the
//!   default on terminals that advertise truecolor support. Four visual
//!   palettes ship with it, selectable via `--board`.
//! - [`BoardStyle::Frame`] — the classic `+---+` grid, used automatically
//!   when colours are unavailable, when `--ascii` is passed, or when the
//!   terminal does not support truecolor.
//!
//! Both styles support last-move and check highlighting, an optional target
//! overlay (hints, legal-move previews), flipped orientation, coordinates,
//! and a "piece in flight" override used by the move animation.

use colored::{ColoredString, Colorize};

use crate::game::Game;
use crate::movegen;
use crate::types::{Board, Color, Piece, PieceKind, Square};

/// Visual style of the rendered board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardStyle {
    /// Solid coloured squares (requires 24-bit colour).
    Squares,
    /// Classic `+---+` ASCII grid.
    Frame,
}

/// A named colour palette for [`BoardStyle::Squares`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum BoardTheme {
    /// Warm wooden board — the classic tournament look.
    #[default]
    Wood,
    /// Cool blue-grey, easy on the eyes in dark terminals.
    Ice,
    /// Green and cream, like a club vinyl board.
    Club,
    /// High-contrast greyscale.
    Mono,
    /// Plain `+---+` grid with no square colouring.
    Ascii,
}

/// RGB triple.
type Rgb = (u8, u8, u8);

/// The six colours that define a board palette.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Background of light squares.
    pub light: Rgb,
    /// Background of dark squares.
    pub dark: Rgb,
    /// Background of the squares of the last move.
    pub last_move: Rgb,
    /// Background of a king in check.
    pub check: Rgb,
    /// Background of an overlaid target square (hint / legal move).
    pub target: Rgb,
    /// Foreground of white pieces.
    pub white_piece: Rgb,
    /// Foreground of black pieces.
    pub black_piece: Rgb,
}

impl BoardTheme {
    /// The colours for this theme.
    pub fn palette(self) -> Palette {
        match self {
            BoardTheme::Wood | BoardTheme::Ascii => Palette {
                light: (232, 208, 170),
                dark: (176, 132, 95),
                last_move: (206, 194, 96),
                check: (214, 90, 78),
                target: (128, 178, 118),
                white_piece: (252, 252, 250),
                black_piece: (30, 26, 22),
            },
            BoardTheme::Ice => Palette {
                light: (206, 216, 228),
                dark: (114, 138, 166),
                last_move: (150, 182, 120),
                check: (206, 92, 92),
                target: (120, 172, 200),
                white_piece: (255, 255, 255),
                black_piece: (22, 26, 34),
            },
            BoardTheme::Club => Palette {
                light: (238, 238, 210),
                dark: (118, 150, 86),
                last_move: (206, 210, 106),
                check: (214, 88, 78),
                target: (90, 160, 130),
                white_piece: (255, 255, 255),
                black_piece: (24, 28, 24),
            },
            BoardTheme::Mono => Palette {
                light: (208, 208, 208),
                dark: (112, 112, 112),
                last_move: (168, 168, 120),
                check: (196, 96, 96),
                target: (150, 150, 150),
                white_piece: (255, 255, 255),
                black_piece: (16, 16, 16),
            },
        }
    }
}

/// Squares to visually emphasize when rendering a board.
#[derive(Debug, Clone, Default)]
pub struct BoardHighlights {
    /// From/to squares of the most recent move.
    pub last_move: Option<(Square, Square)>,
    /// Square of a king currently in check.
    pub check: Option<Square>,
    /// Extra squares to mark (hint destinations, legal-move previews).
    pub targets: Vec<Square>,
    /// A piece drawn at a square it does not occupy yet (move animation),
    /// together with the square it is travelling from.
    pub in_flight: Option<(Piece, Square, Square)>,
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
        Self {
            last_move,
            check,
            targets: Vec::new(),
            in_flight: None,
        }
    }

    /// Adds target squares to highlight (builder style).
    pub fn with_targets(mut self, targets: Vec<Square>) -> Self {
        self.targets = targets;
        self
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
    /// Which style to draw in.
    pub style: BoardStyle,
    /// Colour palette for [`BoardStyle::Squares`].
    pub palette: Palette,
}

impl Default for BoardRenderer {
    fn default() -> Self {
        Self {
            ascii: false,
            flipped: false,
            coords: true,
            style: BoardStyle::Frame,
            palette: BoardTheme::default().palette(),
        }
    }
}

/// `true` when the terminal advertises 24-bit colour support.
pub fn supports_truecolor() -> bool {
    std::env::var("COLORTERM")
        .map(|v| {
            let v = v.to_ascii_lowercase();
            v.contains("truecolor") || v.contains("24bit")
        })
        .unwrap_or(false)
}

impl BoardRenderer {
    /// Creates a renderer with the given glyph mode and orientation, using the
    /// classic framed style.
    pub fn new(ascii: bool, flipped: bool) -> Self {
        Self {
            ascii,
            flipped,
            ..Self::default()
        }
    }

    /// Creates the renderer a command should use, picking the richest style
    /// the terminal can actually display.
    ///
    /// Coloured squares need both colour support and truecolor; `--ascii` and
    /// the `ascii` board theme always force the framed style.
    pub fn for_theme(colors_enabled: bool, ascii: bool, flipped: bool, theme: BoardTheme) -> Self {
        let squares =
            colors_enabled && !ascii && theme != BoardTheme::Ascii && supports_truecolor();
        Self {
            ascii,
            flipped,
            coords: true,
            style: if squares {
                BoardStyle::Squares
            } else {
                BoardStyle::Frame
            },
            palette: theme.palette(),
        }
    }

    /// Number of terminal lines [`BoardRenderer::render`] produces.
    pub fn height(&self) -> usize {
        match self.style {
            // leading blank + 8 ranks + coordinate row
            BoardStyle::Squares => 1 + 8 + usize::from(self.coords),
            // leading blank + top rule + 8 * (rank + rule) + coordinate row
            BoardStyle::Frame => 2 + 16 + usize::from(self.coords),
        }
    }

    /// Renders the board to a multi-line string.
    pub fn render(&self, board: &Board, highlights: &BoardHighlights) -> String {
        match self.style {
            BoardStyle::Squares => self.render_squares(board, highlights),
            BoardStyle::Frame => self.render_frame(board, highlights),
        }
    }

    /// Iterates the ranks in display order (top row first).
    fn ranks(&self) -> impl Iterator<Item = u8> + '_ {
        (0..8u8).map(move |row| if self.flipped { row } else { 7 - row })
    }

    /// Iterates the files in display order (left column first).
    fn files(&self) -> impl Iterator<Item = u8> + '_ {
        (0..8u8).map(move |col| if self.flipped { 7 - col } else { col })
    }

    /// Renders the coloured-squares style.
    fn render_squares(&self, board: &Board, highlights: &BoardHighlights) -> String {
        let mut out = String::from("\n");
        for rank in self.ranks() {
            if self.coords {
                out.push_str(&format!("{} ", (rank + 1).to_string().dimmed()));
            } else {
                out.push_str("  ");
            }
            for file in self.files() {
                let sq = Square::new(file, rank);
                let (r, g, b) = self.square_color(sq, highlights);
                let glyph = self.glyph_at(board, sq, highlights);
                let cell = match glyph {
                    Some((symbol, color)) => {
                        let (pr, pg, pb) = self.piece_color(color);
                        format!(" {symbol} ").truecolor(pr, pg, pb).bold()
                    }
                    None => ColoredString::from("   "),
                };
                out.push_str(&cell.on_truecolor(r, g, b).to_string());
            }
            out.push('\n');
        }
        if self.coords {
            out.push_str("  ");
            for file in self.files() {
                out.push_str(&format!(
                    " {} ",
                    ((b'a' + file) as char).to_string().dimmed()
                ));
            }
            out.push('\n');
        }
        out
    }

    /// Renders the classic framed style.
    fn render_frame(&self, board: &Board, highlights: &BoardHighlights) -> String {
        let mut out = String::new();
        let separator = "  +---+---+---+---+---+---+---+---+\n";

        out.push('\n');
        out.push_str(separator);

        for rank in self.ranks() {
            if self.coords {
                out.push_str(&format!("{} ", rank + 1));
            } else {
                out.push_str("  ");
            }
            for file in self.files() {
                let sq = Square::new(file, rank);
                out.push('|');
                out.push_str(&self.render_frame_cell(board, sq, highlights));
            }
            out.push_str("|\n");
            out.push_str(separator);
        }

        if self.coords {
            let files: String = self
                .files()
                .map(|file| format!("  {} ", (b'a' + file) as char))
                .collect();
            out.push_str(&format!("  {files}\n"));
        }
        out
    }

    /// Renders one 3-character cell (` X `) of the framed style.
    fn render_frame_cell(&self, board: &Board, sq: Square, highlights: &BoardHighlights) -> String {
        let is_dark = (sq.file + sq.rank).is_multiple_of(2);
        let glyph = match self.glyph_at(board, sq, highlights) {
            Some((symbol, Color::White)) => symbol.white().bold().to_string(),
            Some((symbol, Color::Black)) => symbol.blue().bold().to_string(),
            None if is_dark && !self.ascii => "·".dimmed().to_string(),
            None if is_dark => ".".to_string(),
            None => " ".to_string(),
        };

        let cell = format!(" {glyph} ");
        if highlights.check == Some(sq) {
            cell.on_red().to_string()
        } else if highlights.targets.contains(&sq) {
            cell.on_green().black().to_string()
        } else if highlights
            .last_move
            .is_some_and(|(from, to)| from == sq || to == sq)
        {
            cell.on_yellow().black().to_string()
        } else {
            cell
        }
    }

    /// Which piece (if any) to draw on a square, honouring the in-flight
    /// override used by the move animation.
    fn glyph_at(
        &self,
        board: &Board,
        sq: Square,
        highlights: &BoardHighlights,
    ) -> Option<(&'static str, Color)> {
        if let Some((piece, from, at)) = highlights.in_flight {
            if sq == at {
                return Some((self.piece_symbol(piece), piece.color));
            }
            if sq == from {
                return None;
            }
        }
        board
            .get(sq)
            .map(|piece| (self.piece_symbol(piece), piece.color))
    }

    /// Background colour of a square after applying every highlight.
    fn square_color(&self, sq: Square, highlights: &BoardHighlights) -> Rgb {
        let base = if (sq.file + sq.rank).is_multiple_of(2) {
            self.palette.dark
        } else {
            self.palette.light
        };
        if highlights.check == Some(sq) {
            return self.palette.check;
        }
        if highlights.targets.contains(&sq) {
            return self.palette.target;
        }
        if let Some((piece, from, at)) = highlights.in_flight {
            let _ = piece;
            if sq == at || sq == from {
                return blend(base, self.palette.last_move, 0.75);
            }
        }
        if highlights
            .last_move
            .is_some_and(|(from, to)| from == sq || to == sq)
        {
            return blend(base, self.palette.last_move, 0.65);
        }
        base
    }

    /// Foreground colour of a piece.
    fn piece_color(&self, color: Color) -> Rgb {
        match color {
            Color::White => self.palette.white_piece,
            Color::Black => self.palette.black_piece,
        }
    }

    /// Returns the display symbol for a piece in the configured glyph mode.
    fn piece_symbol(&self, piece: Piece) -> &'static str {
        if self.ascii {
            ascii_symbol(piece)
        } else if self.style == BoardStyle::Squares {
            // On coloured squares both sides use the solid glyph set and are
            // told apart by their fill colour — outline glyphs vanish on a
            // light background in most fonts.
            solid_symbol(piece.kind)
        } else {
            unicode_symbol(piece)
        }
    }
}

/// Linearly blends two colours; `weight` is the share of `b`.
fn blend(a: Rgb, b: Rgb, weight: f32) -> Rgb {
    let mix = |x: u8, y: u8| (f32::from(x) * (1.0 - weight) + f32::from(y) * weight) as u8;
    (mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
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

/// The solid (filled) glyph for a piece kind, used on coloured squares where
/// the piece's own colour carries the side information.
pub fn solid_symbol(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::King => "♚",
        PieceKind::Queen => "♛",
        PieceKind::Rook => "♜",
        PieceKind::Bishop => "♝",
        PieceKind::Knight => "♞",
        PieceKind::Pawn => "♟",
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

// ---------------------------------------------------------------------------
// Captured material
// ---------------------------------------------------------------------------

/// Material each side has captured, derived by diffing against a full army.
#[derive(Debug, Clone, Default)]
pub struct CapturedMaterial {
    /// Black pieces White has taken.
    pub by_white: Vec<PieceKind>,
    /// White pieces Black has taken.
    pub by_black: Vec<PieceKind>,
    /// Material balance in pawns, positive when White is ahead.
    pub balance: i32,
}

/// Standard starting counts, indexed like [`CapturedMaterial`].
const FULL_ARMY: [(PieceKind, usize); 5] = [
    (PieceKind::Queen, 1),
    (PieceKind::Rook, 2),
    (PieceKind::Bishop, 2),
    (PieceKind::Knight, 2),
    (PieceKind::Pawn, 8),
];

/// Point value of a piece kind for the material balance.
fn material_value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Queen => 9,
        PieceKind::Rook => 5,
        PieceKind::Bishop | PieceKind::Knight => 3,
        PieceKind::Pawn => 1,
        PieceKind::King => 0,
    }
}

impl CapturedMaterial {
    /// Computes captured material for a board, assuming a standard start.
    ///
    /// Promotions can make a side appear to have "captured" its own piece
    /// (a promoted pawn leaves a pawn deficit); those artefacts are clamped
    /// away so the display never shows a negative count.
    pub fn for_board(board: &Board) -> Self {
        let count = |color: Color, kind: PieceKind| -> usize {
            (0..64)
                .filter(|i| {
                    let sq = Square::new((i % 8) as u8, (i / 8) as u8);
                    board
                        .get(sq)
                        .is_some_and(|p| p.color == color && p.kind == kind)
                })
                .count()
        };

        let mut by_white = Vec::new();
        let mut by_black = Vec::new();
        for (kind, full) in FULL_ARMY {
            for _ in 0..full.saturating_sub(count(Color::Black, kind)) {
                by_white.push(kind);
            }
            for _ in 0..full.saturating_sub(count(Color::White, kind)) {
                by_black.push(kind);
            }
        }
        let balance = by_white.iter().map(|k| material_value(*k)).sum::<i32>()
            - by_black.iter().map(|k| material_value(*k)).sum::<i32>();
        Self {
            by_white,
            by_black,
            balance,
        }
    }

    /// Renders the captured pieces of one side as a glyph run.
    pub fn glyphs(&self, side: Color) -> String {
        let list = match side {
            Color::White => &self.by_white,
            Color::Black => &self.by_black,
        };
        list.iter().map(|kind| solid_symbol(*kind)).collect()
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

    #[test]
    fn test_square_style_renders_eight_rows() {
        colored::control::set_override(false);
        let renderer = BoardRenderer {
            style: BoardStyle::Squares,
            ..BoardRenderer::default()
        };
        let out = renderer.render(&Board::starting_position(), &BoardHighlights::default());
        // leading blank line + 8 ranks + coordinates
        assert_eq!(out.lines().count(), renderer.height());
        assert!(out.contains('♟'), "solid glyphs on coloured squares");
    }

    #[test]
    fn test_frame_height_matches_render() {
        colored::control::set_override(false);
        let renderer = BoardRenderer::new(true, false);
        let out = renderer.render(&Board::starting_position(), &BoardHighlights::default());
        assert_eq!(out.lines().count(), renderer.height());
    }

    #[test]
    fn test_in_flight_piece_moves_glyph() {
        colored::control::set_override(false);
        let board = Board::starting_position();
        let from = Square::from_algebraic("e2").unwrap();
        let at = Square::from_algebraic("e3").unwrap();
        let piece = board.get(from).expect("pawn on e2");
        let highlights = BoardHighlights {
            in_flight: Some((piece, from, at)),
            ..BoardHighlights::default()
        };
        let renderer = BoardRenderer::new(true, false);
        let out = renderer.render(&board, &highlights);
        let rank3 = out.lines().find(|l| l.starts_with("3 ")).unwrap();
        let rank2 = out.lines().find(|l| l.starts_with("2 ")).unwrap();
        assert!(
            rank3.contains('P'),
            "pawn must be drawn in flight on rank 3"
        );
        assert_eq!(
            rank2.matches('P').count(),
            7,
            "the e2 pawn must be lifted off its origin square"
        );
    }

    #[test]
    fn test_captured_material_starting_position_is_empty() {
        let captured = CapturedMaterial::for_board(&Board::starting_position());
        assert!(captured.by_white.is_empty());
        assert!(captured.by_black.is_empty());
        assert_eq!(captured.balance, 0);
    }

    #[test]
    fn test_captured_material_counts_and_balance() {
        // White is a full queen and a pawn up; Black has taken one knight.
        let board = crate::game::Game::from_fen("4k3/pppppp1p/8/8/8/8/PPPPPPPP/R1BQKBNR w - - 0 1")
            .unwrap()
            .board;
        let captured = CapturedMaterial::for_board(&board);
        assert!(captured.by_white.contains(&PieceKind::Queen));
        assert!(captured.by_black.contains(&PieceKind::Knight));
        assert!(captured.balance > 0, "White is clearly ahead");
        assert!(!captured.glyphs(Color::White).is_empty());
    }
}
