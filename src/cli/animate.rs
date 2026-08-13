//! Terminal animation primitives for the interactive CLI.
//!
//! Everything here is TTY-gated through [`Theme::interactive`]: on a real
//! terminal the CLI redraws in place and animates; when the output is piped
//! the same calls print a single, clean final frame (or nothing at all), so
//! logs and scripts never see cursor escapes or duplicated boards.
//!
//! The building blocks are:
//!
//! - [`LiveRegion`] — a block of lines that can be repainted in place.
//! - [`animate_move`] — slides a piece across the board, square by square.
//! - [`flash_squares`] — pulses squares to punctuate check and checkmate.
//! - [`reveal_lines`] — staggered line-by-line reveal for static screens.

use std::io::{Write, stdout};
use std::time::Duration;

use crossterm::{cursor, execute, terminal};

use super::board_renderer::{BoardHighlights, BoardRenderer};
use super::theme::Theme;
use crate::types::{Board, Square};

/// Frame interval of the piece-movement animation.
const MOVE_FRAME: Duration = Duration::from_millis(45);

/// Total budget for one piece movement; the per-frame delay shrinks so long
/// slides never feel slower than short ones.
const MOVE_BUDGET: Duration = Duration::from_millis(260);

/// Frame interval of the check/checkmate flash.
const FLASH_FRAME: Duration = Duration::from_millis(110);

/// A block of terminal lines that can be repainted in place.
///
/// Each [`LiveRegion::frame`] call rewinds the cursor over the previously
/// drawn content and paints the new frame, producing flicker-free animation
/// without a full-screen alternate buffer (so scrollback stays intact).
pub struct LiveRegion {
    /// Whether in-place redrawing is allowed at all.
    active: bool,
    /// Lines currently occupied by this region.
    lines: u16,
}

impl LiveRegion {
    /// Creates a region for the given theme. On non-interactive output the
    /// region becomes a no-op for intermediate frames.
    pub fn new(theme: &Theme) -> Self {
        Self {
            active: theme.interactive,
            lines: 0,
        }
    }

    /// Paints an intermediate frame. Does nothing when animations are off.
    pub fn frame(&mut self, content: &str) {
        if !self.active {
            return;
        }
        self.repaint(content);
    }

    /// Paints the final content and releases the region, leaving the output
    /// on screen. Always prints, even on non-interactive output.
    pub fn finish(&mut self, content: &str) {
        if self.active {
            self.repaint(content);
        } else {
            print!("{content}");
            let _ = stdout().flush();
        }
        self.lines = 0;
    }

    /// Erases the region entirely.
    pub fn clear(&mut self) {
        if self.active && self.lines > 0 {
            self.rewind();
            let _ = stdout().flush();
        }
        self.lines = 0;
    }

    /// Rewinds over the previously painted content and prints `content`.
    fn repaint(&mut self, content: &str) {
        if self.lines > 0 {
            self.rewind();
        }
        print!("{content}");
        let _ = stdout().flush();
        self.lines = content.lines().count() as u16;
    }

    /// Moves the cursor back to the top of the region and clears downwards.
    fn rewind(&self) {
        let mut out = stdout();
        let _ = execute!(
            out,
            cursor::MoveToPreviousLine(self.lines),
            terminal::Clear(terminal::ClearType::FromCursorDown)
        );
    }
}

/// Sleeps for `duration`, but only when animations are enabled.
pub fn pause(theme: &Theme, duration: Duration) {
    if theme.interactive && !duration.is_zero() {
        std::thread::sleep(duration);
    }
}

/// The squares a piece visually travels through, `from` exclusive,
/// `to` inclusive.
///
/// Sliding moves follow their own line; knight moves take the two-leg L so
/// the piece never appears to jump through the board's middle.
pub fn travel_path(from: Square, to: Square) -> Vec<Square> {
    let (df, dr) = (
        to.file as i8 - from.file as i8,
        to.rank as i8 - from.rank as i8,
    );
    let aligned = df == 0 || dr == 0 || df.abs() == dr.abs();
    let mut path = Vec::new();

    if aligned {
        let steps = df.abs().max(dr.abs());
        let (sf, sr) = (df.signum(), dr.signum());
        for step in 1..=steps {
            if let Some(sq) = from.offset(sf * step, sr * step) {
                path.push(sq);
            }
        }
    } else {
        // Knight: walk the long leg first, then the short one.
        let (first, second) = if df.abs() > dr.abs() {
            ((df, 0i8), (0i8, dr))
        } else {
            ((0i8, dr), (df, 0i8))
        };
        let mut current = from;
        for (fd, rd) in [first, second] {
            let steps = fd.abs().max(rd.abs());
            let (sf, sr) = (fd.signum(), rd.signum());
            for _ in 0..steps {
                if let Some(next) = current.offset(sf, sr) {
                    path.push(next);
                    current = next;
                }
            }
        }
    }

    if path.last() != Some(&to) {
        path.push(to);
    }
    path
}

/// Animates a piece sliding from `from` to `to` on `board`.
///
/// `board` must be the position *before* the move; the caller renders the
/// final position itself. On non-interactive output this is a no-op.
#[allow(clippy::too_many_arguments)]
pub fn animate_move(
    theme: &Theme,
    region: &mut LiveRegion,
    renderer: &BoardRenderer,
    board: &Board,
    from: Square,
    to: Square,
    header: &str,
    footer: &str,
) {
    if !theme.interactive {
        return;
    }
    let Some(piece) = board.get(from) else {
        return;
    };
    let path = travel_path(from, to);
    if path.is_empty() {
        return;
    }
    let delay = (MOVE_BUDGET / path.len().max(1) as u32).min(MOVE_FRAME);

    for square in &path {
        let highlights = BoardHighlights {
            last_move: Some((from, to)),
            in_flight: Some((piece, from, *square)),
            ..BoardHighlights::default()
        };
        region.frame(&compose(
            header,
            &renderer.render(board, &highlights),
            footer,
        ));
        pause(theme, delay);
    }
}

/// Pulses a set of squares `times` times — used for check and checkmate.
#[allow(clippy::too_many_arguments)]
pub fn flash_squares(
    theme: &Theme,
    region: &mut LiveRegion,
    renderer: &BoardRenderer,
    board: &Board,
    base: &BoardHighlights,
    squares: &[Square],
    times: usize,
    header: &str,
    footer: &str,
) {
    if !theme.interactive || squares.is_empty() {
        return;
    }
    for round in 0..times * 2 {
        let mut highlights = base.clone();
        if round.is_multiple_of(2) {
            highlights.targets = squares.to_vec();
        }
        region.frame(&compose(
            header,
            &renderer.render(board, &highlights),
            footer,
        ));
        pause(theme, FLASH_FRAME);
    }
}

/// Joins an optional header, the board, and an optional footer into one
/// repaintable block.
pub fn compose(header: &str, board: &str, footer: &str) -> String {
    let mut out = String::with_capacity(header.len() + board.len() + footer.len() + 2);
    if !header.is_empty() {
        out.push_str(header);
        if !header.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push_str(board);
    if !board.ends_with('\n') {
        out.push('\n');
    }
    if !footer.is_empty() {
        out.push_str(footer);
        if !footer.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// Prints lines with a small stagger so a static screen "wipes" into view.
///
/// Piped output prints everything at once.
pub fn reveal_lines(theme: &Theme, lines: &[String], delay: Duration) {
    if theme.interactive && !delay.is_zero() {
        for line in lines {
            println!("{line}");
            let _ = stdout().flush();
            std::thread::sleep(delay);
        }
    } else {
        println!("{}", lines.join("\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(name: &str) -> Square {
        Square::from_algebraic(name).expect("valid square")
    }

    #[test]
    fn test_travel_path_straight_line() {
        let path = travel_path(sq("a1"), sq("a4"));
        let names: Vec<String> = path.iter().map(|s| s.to_algebraic()).collect();
        assert_eq!(names, vec!["a2", "a3", "a4"]);
    }

    #[test]
    fn test_travel_path_diagonal() {
        let path = travel_path(sq("c1"), sq("f4"));
        let names: Vec<String> = path.iter().map(|s| s.to_algebraic()).collect();
        assert_eq!(names, vec!["d2", "e3", "f4"]);
    }

    #[test]
    fn test_travel_path_knight_uses_two_legs() {
        let path = travel_path(sq("g1"), sq("f3"));
        // Long leg is the rank change (2), so it moves up first, then across.
        let names: Vec<String> = path.iter().map(|s| s.to_algebraic()).collect();
        assert_eq!(names, vec!["g2", "g3", "f3"]);
        assert_eq!(path.last(), Some(&sq("f3")));
    }

    #[test]
    fn test_travel_path_single_step() {
        assert_eq!(travel_path(sq("e2"), sq("e3")), vec![sq("e3")]);
    }

    #[test]
    fn test_compose_normalizes_newlines() {
        let out = compose("head", "board", "foot");
        assert_eq!(out, "head\nboard\nfoot\n");
        assert_eq!(compose("", "board\n", ""), "board\n");
    }

    #[test]
    fn test_live_region_is_inert_without_tty() {
        let theme = Theme {
            colors: false,
            interactive: false,
        };
        let mut region = LiveRegion::new(&theme);
        // Intermediate frames must not print, and must not panic.
        region.frame("ignored\n");
        region.clear();
    }
}
