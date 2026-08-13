//! Terminal theme detection and color control.
//!
//! Centralizes every decision about *how* the CLI is allowed to draw:
//!
//! - Colors are enabled only when stdout is a TTY, `--no-color` was not
//!   passed, and the `NO_COLOR` environment variable is unset/empty.
//! - Animations (spinners, progress bars, reveal effects) additionally
//!   require an interactive terminal — piped output always stays plain.
//!
//! When colors are disabled this module flips the global `colored`
//! override so every `.green()` / `.bold()` call in the codebase
//! degrades to plain text automatically.

use std::io::IsTerminal;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Fallback terminal width when the real width cannot be determined.
const DEFAULT_TERM_WIDTH: usize = 80;

/// Detected terminal capabilities for the current process.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// `true` when ANSI colors may be emitted.
    pub colors: bool,
    /// `true` when stdout is an interactive terminal (TTY).
    /// Animations and live redraws must be gated on this flag.
    pub interactive: bool,
}

impl Theme {
    /// Detects terminal capabilities and applies the global color override.
    ///
    /// `no_color` is the value of the global `--no-color` CLI flag; the
    /// `NO_COLOR` environment variable (any non-empty value) is honored
    /// as well, per <https://no-color.org/>.
    pub fn detect(no_color: bool) -> Self {
        let interactive = std::io::stdout().is_terminal();
        let env_no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        let colors = interactive && !no_color && !env_no_color;
        if !colors {
            colored::control::set_override(false);
        }
        Self {
            colors,
            interactive,
        }
    }

    /// Returns a theme with all capabilities disabled (plain output).
    /// Used by machine-facing modes such as UCI.
    pub fn plain() -> Self {
        colored::control::set_override(false);
        Self {
            colors: false,
            interactive: false,
        }
    }
}

/// Returns the current terminal width in columns (fallback: 80).
pub fn term_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(DEFAULT_TERM_WIDTH)
}

/// Splits `text` into `(is_escape, segment)` runs.
///
/// ANSI escapes occupy no terminal columns, but every byte of them looks like
/// an ordinary character to `unicode-width`: one truecolor introducer,
/// `\x1b[38;2;69;113;191m`, measures 19 columns on its own, so a 24-cell
/// gradient bar "measures" over 500. Width and truncation maths has to skip
/// them — and a cut must never land inside one, because a half-emitted CSI
/// leaks its colour into everything printed afterwards and is never reset.
fn ansi_segments(text: &str) -> Vec<(bool, &str)> {
    const ESC: u8 = 0x1b;
    let bytes = text.as_bytes();
    let mut segments = Vec::new();
    let mut visible_start = 0;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != ESC {
            i += 1;
            continue;
        }
        if visible_start < i {
            segments.push((false, &text[visible_start..i]));
        }
        let escape_start = i;
        i += 1;
        match bytes.get(i) {
            // CSI: parameter bytes, then a final byte in 0x40..=0x7e.
            Some(b'[') => {
                i += 1;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
                i = (i + 1).min(bytes.len());
            }
            // OSC: runs until BEL or the ST terminator `ESC \`.
            Some(b']') => {
                i += 1;
                while i < bytes.len() && bytes[i] != 0x07 {
                    if bytes[i] == ESC && bytes.get(i + 1) == Some(&b'\\') {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                i = (i + 1).min(bytes.len());
            }
            // Any other two-character escape. Advance a whole `char`, not a
            // byte: a stray ESC in front of a multi-byte glyph would otherwise
            // leave `i` on a continuation byte and the slice below would panic.
            Some(_) => i += text[i..].chars().next().map_or(1, char::len_utf8),
            None => {}
        }
        segments.push((true, &text[escape_start..i]));
        visible_start = i;
    }

    if visible_start < bytes.len() {
        segments.push((false, &text[visible_start..]));
    }
    segments
}

/// Display width of `text` in terminal columns (East-Asian wide glyphs
/// count as two, ANSI escapes count as zero). Use this — never
/// `chars().count()` — for any box-drawing or column-alignment math so CJK
/// and coloured text line up correctly.
pub fn display_width(text: &str) -> usize {
    ansi_segments(text)
        .into_iter()
        .filter(|(is_escape, _)| !is_escape)
        .map(|(_, segment)| UnicodeWidthStr::width(segment))
        .sum()
}

/// Truncates `text` to at most `max` display columns, appending `…` when cut.
///
/// Operates on `char` boundaries (so multi-byte input can never panic),
/// accounts for wide glyphs, and passes ANSI escapes through without charging
/// them any width. A truncated string is always closed with a reset so a cut
/// inside a coloured run cannot bleed into the rest of the screen.
pub fn truncate_chars(text: &str, max: usize) -> String {
    if display_width(text) <= max {
        return text.to_string();
    }
    let budget = max.saturating_sub(1); // reserve one column for the '…'
    let mut out = String::new();
    let mut width = 0;
    let mut coloured = false;
    let mut cut = false;

    'outer: for (is_escape, segment) in ansi_segments(text) {
        if is_escape {
            coloured = true;
            out.push_str(segment);
            continue;
        }
        for ch in segment.chars() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if width + cw > budget {
                cut = true;
                break 'outer;
            }
            out.push(ch);
            width += cw;
        }
    }

    // Everything after the cut is dropped, including the styling's own reset.
    if cut && coloured {
        out.push_str("\u{1b}[0m");
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short_text_unchanged() {
        assert_eq!(truncate_chars("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long_text() {
        let out = truncate_chars("abcdefghij", 5);
        assert_eq!(out, "abcd…");
        assert_eq!(out.chars().count(), 5);
    }

    #[test]
    fn test_truncate_multibyte_safe() {
        let out = truncate_chars("♔♕♖♗♘♙♚♛", 4);
        assert_eq!(out.chars().count(), 4);
        assert!(out.ends_with('…'));
    }

    /// A truecolor introducer is 19 bytes wide and zero columns wide. Charging
    /// it as text made the thinking panel's 24-cell gradient bar "measure"
    /// over 500 columns, so it was cut after four cells.
    #[test]
    fn test_display_width_ignores_ansi_escapes() {
        let coloured = "\u{1b}[38;2;69;113;191m█\u{1b}[0m";
        assert_eq!(display_width(coloured), 1);
        assert_eq!(display_width("\u{1b}[1mbold\u{1b}[0m"), 4);
        assert_eq!(display_width("plain"), 5);
    }

    #[test]
    fn test_truncate_keeps_colour_and_never_cuts_inside_an_escape() {
        let bar: String = (0..24)
            .map(|_| "\u{1b}[38;2;69;113;191m█\u{1b}[0m")
            .collect();
        // Fits comfortably in 79 columns once escapes stop being counted.
        assert_eq!(display_width(&bar), 24);
        assert_eq!(truncate_chars(&bar, 79), bar, "no cut was needed at all");

        let cut = truncate_chars(&bar, 10);
        assert_eq!(display_width(&cut), 10, "9 cells plus the ellipsis");
        assert!(cut.ends_with("\u{1b}[0m…"), "a cut run must be closed");
        // Every escape in the output is complete.
        assert_eq!(
            cut.matches('\u{1b}').count(),
            cut.matches('m').count(),
            "a half-emitted CSI would leak colour into the rest of the screen"
        );
    }

    /// ESC followed by a UTF-8 lead byte used to advance a single byte, land
    /// on a continuation byte and panic on the slice. `truncate_chars` runs on
    /// text loaded from PGN files, so the panic was reachable from file input.
    ///
    /// The glyph after ESC is swallowed as the escape's second character —
    /// which is what a terminal does with `ESC c`, `ESC 7` and friends — so it
    /// contributes no width. The point of the test is that it does not panic.
    #[test]
    fn test_escape_before_multibyte_glyph_does_not_panic() {
        assert_eq!(display_width("\u{1b}\u{e9}"), 0);
        assert_eq!(display_width("\u{1b}\u{2654}king"), 4);
        // The escape is passed through, so the cut is closed with a reset.
        assert_eq!(
            truncate_chars("\u{1b}\u{e9}abcdefghij", 4),
            "\u{1b}\u{e9}abc\u{1b}[0m…"
        );
        // A lone trailing ESC must not run off the end either.
        assert_eq!(display_width("ok\u{1b}"), 2);
    }
}
