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

/// Display width of `text` in terminal columns (East-Asian wide glyphs
/// count as two). Use this — never `chars().count()` — for any box-drawing
/// or column-alignment math so CJK text lines up correctly.
pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Truncates `text` to at most `max` display columns, appending `…` when cut.
///
/// Operates on `char` boundaries (so multi-byte input can never panic) and
/// accounts for wide glyphs, so the result never exceeds `max` columns.
pub fn truncate_chars(text: &str, max: usize) -> String {
    if display_width(text) <= max {
        return text.to_string();
    }
    let budget = max.saturating_sub(1); // reserve one column for the '…'
    let mut out = String::new();
    let mut width = 0;
    for ch in text.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + cw > budget {
            break;
        }
        out.push(ch);
        width += cw;
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
}
