//! Command-line interface for CheckAI.
//!
//! This module hosts every interactive CLI command (one submodule per
//! command) plus the shared infrastructure they build on:
//!
//! - [`theme`] — color/TTY detection (`--no-color`, `NO_COLOR`) and styling.
//! - [`panel`] — data-driven box-drawing helpers (banners, result panels).
//! - [`board_renderer`] — the single board renderer used everywhere
//!   (coloured squares or ASCII frame, highlights, flipped view, coordinates).
//! - [`animate`] — in-place redrawing, move animation and reveal effects.
//! - [`score`] — score formatting (`+1.23`, `#3`), eval bars and sparklines.
//! - [`level`] — the difficulty ladder mapping levels to search limits.
//! - [`engine`] — the shared `--threads/--hash/--book/…` argument group.
//! - [`clock`] — time controls and the two-sided game clock.
//! - [`progress`] — TTY-gated spinners, bars and the live thinking panel.
//! - [`fen`] — FEN import/export for [`crate::game::Game`].
//! - [`pgn`] — SAN rendering/parsing and PGN reading/writing.
//!
//! Commands implement the [`CliCommand`] trait and are dispatched from
//! `main.rs`, which stays a thin parser + locale setup layer.

pub mod analyze;
pub mod animate;
pub mod bench;
pub mod board_renderer;
pub mod clock;
pub mod engine;
pub mod eval;
pub mod fen;
pub mod level;
pub mod panel;
pub mod perft;
pub mod pgn;
pub mod play;
pub mod progress;
pub mod score;
pub mod theme;
pub mod uci;
pub mod watch;
pub mod welcome;

use theme::Theme;

/// Result type shared by all CLI commands.
///
/// Uses standard error boxing — commands bubble up any error and
/// `main.rs` converts it into a non-zero exit code.
pub type CliResult = Result<(), Box<dyn std::error::Error>>;

/// Shared context passed to every CLI command.
///
/// Carries the detected [`Theme`] (colors / interactivity) so commands
/// never have to probe the terminal themselves.
pub struct CliContext {
    /// Detected terminal theme (colors enabled, TTY or not).
    pub theme: Theme,
}

impl CliContext {
    /// Creates a context from CLI flags, applying global color overrides.
    pub fn new(no_color: bool) -> Self {
        Self {
            theme: Theme::detect(no_color),
        }
    }
}

/// Command pattern trait implemented by each CLI subcommand's argument
/// struct. `main.rs` parses clap arguments, builds a [`CliContext`] and
/// dispatches via this trait.
pub trait CliCommand {
    /// Executes the command, consuming its parsed arguments.
    fn run(self, ctx: &CliContext) -> CliResult;
}

/// Convenience constructor for a string-based CLI error.
pub fn cli_error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::<dyn std::error::Error>::from(message.into())
}
