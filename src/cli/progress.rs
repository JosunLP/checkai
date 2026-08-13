//! TTY-gated spinners, progress bars and the live "thinking" panel.
//!
//! Every animation in the CLI goes through this module so the rule
//! "animations only on interactive terminals" is enforced in one place:
//! when stdout is not a TTY the helpers return hidden progress bars,
//! whose method calls are no-ops, keeping piped output perfectly clean.

use std::time::Duration;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use super::score::{eval_bar_gradient, format_score, humanize_count, white_pov};
use super::theme::{Theme, term_width, truncate_chars};
use crate::search::IterationInfo;
use crate::types::Color;

/// Tick interval for spinners.
const SPINNER_TICK: Duration = Duration::from_millis(80);

/// Maximum number of PV moves shown on the thinking line.
const PV_PREVIEW_MOVES: usize = 6;

/// Maximum number of PV moves shown in the multi-line thinking panel.
const PV_PANEL_MOVES: usize = 12;

/// Width of the eval bar inside the thinking panel.
const PANEL_BAR_WIDTH: usize = 24;

/// Braille spinner frames, ending on a check mark for the finished state.
const SPINNER_FRAMES: [&str; 11] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✓"];

/// Rotating "engine is thinking" glyphs used by the panel header.
const PULSE_FRAMES: [&str; 8] = ["◐", "◓", "◑", "◒", "◐", "◓", "◑", "◒"];

/// Creates a "thinking" spinner with the given message.
///
/// Returns a hidden (no-op) progress bar on non-interactive terminals.
pub fn spinner(theme: &Theme, message: String) -> ProgressBar {
    if !theme.interactive {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .expect("static spinner template must parse")
            .tick_strings(&SPINNER_FRAMES),
    );
    pb.set_message(message);
    pb.enable_steady_tick(SPINNER_TICK);
    pb
}

/// Creates a determinate progress bar over `len` steps.
///
/// Returns a hidden (no-op) progress bar on non-interactive terminals.
pub fn bar(theme: &Theme, len: u64, message: String) -> ProgressBar {
    if !theme.interactive {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::with_template(
            "{msg} {bar:32.cyan/blue} {pos}/{len} · {percent:>3}% · eta {eta}",
        )
        .expect("static bar template must parse")
        .progress_chars("█▉▊▋▌▍▎▏ "),
    );
    pb.set_message(message);
    pb
}

/// Formats one iterative-deepening progress snapshot as a single line:
/// depth, score, nodes, nps and a truncated PV preview.
///
/// `prefix` is prepended (e.g. a localized "thinking" label); the result
/// is truncated to the current terminal width.
pub fn iteration_message(prefix: &str, info: &IterationInfo) -> String {
    let score = match info.mate_in {
        Some(mate) => format!("#{mate}"),
        None => format_score(info.score_cp),
    };
    let pv: Vec<&str> = info
        .pv
        .iter()
        .take(PV_PREVIEW_MOVES)
        .map(String::as_str)
        .collect();
    let line = format!(
        "{prefix} d{} {} {} {} {}n/s | {}",
        info.depth,
        score,
        humanize_count(info.nodes),
        t!("cli.nodes_suffix"),
        humanize_count(info.nps),
        pv.join(" "),
    );
    truncate_chars(&line, term_width().saturating_sub(3))
}

/// Live snapshot of one engine search, rendered as a compact panel.
///
/// Used inside an [`super::animate::LiveRegion`] so the board and the search
/// readout repaint together while the engine thinks.
#[derive(Debug, Clone)]
pub struct ThinkingView {
    /// Localized label shown before the statistics.
    pub label: String,
    /// Most recent iteration snapshot, if any.
    pub info: Option<IterationInfo>,
    /// Side the engine is searching for (for the White-relative eval bar).
    pub side: Color,
    /// Animation frame counter, advanced by the caller on every repaint.
    pub tick: usize,
}

impl ThinkingView {
    /// Creates a view for one search.
    pub fn new(label: String, side: Color) -> Self {
        Self {
            label,
            info: None,
            side,
            tick: 0,
        }
    }

    /// Renders the panel: a header line plus an eval bar and PV.
    ///
    /// Always two lines tall (even before the first iteration lands), so the
    /// enclosing live region never jumps as the search progresses.
    pub fn render(&self) -> String {
        let pulse = PULSE_FRAMES[self.tick % PULSE_FRAMES.len()];
        let Some(info) = &self.info else {
            return format!("  {} {}\n\n", pulse.cyan(), self.label.clone().dimmed());
        };

        let score = match info.mate_in {
            Some(mate) => format!("#{mate}"),
            None => format_score(info.score_cp),
        };
        let header = format!(
            "  {} {}  {}  {}  {}  {}",
            pulse.cyan(),
            self.label.clone().bold(),
            t!(
                "progress.depth_short",
                depth = info.depth,
                seldepth = info.seldepth
            )
            .to_string()
            .cyan(),
            score.yellow().bold(),
            t!(
                "progress.nodes_short",
                nodes = humanize_count(info.nodes),
                nps = humanize_count(info.nps)
            )
            .to_string()
            .dimmed(),
            t!("progress.hash_short", permille = info.hashfull)
                .to_string()
                .dimmed(),
        );

        let pv: Vec<&str> = info
            .pv
            .iter()
            .take(PV_PANEL_MOVES)
            .map(String::as_str)
            .collect();
        let bar = eval_bar_gradient(white_pov(info.score_cp, self.side), PANEL_BAR_WIDTH);
        let line = format!("  {bar}  {}", pv.join(" ").dimmed());

        format!(
            "{}\n{}\n",
            truncate_chars(&header, term_width().saturating_sub(1)),
            truncate_chars(&line, term_width().saturating_sub(1))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info() -> IterationInfo {
        IterationInfo {
            depth: 9,
            seldepth: 17,
            score_cp: 35,
            nodes: 1_200_000,
            elapsed_ms: 1500,
            nps: 800_000,
            hashfull: 421,
            pv: vec![
                "e2e4".into(),
                "e7e5".into(),
                "g1f3".into(),
                "b8c6".into(),
                "f1b5".into(),
                "a7a6".into(),
                "b5a4".into(),
            ],
            ..IterationInfo::default()
        }
    }

    #[test]
    fn test_iteration_message_contents() {
        let msg = iteration_message("thinking", &sample_info());
        assert!(msg.contains("d9"));
        assert!(msg.contains("+0.35"));
        assert!(msg.contains("1.2M"));
        assert!(msg.contains("e2e4"));
        // The 7th PV move must be cut off.
        assert!(!msg.contains("b5a4"));
    }

    #[test]
    fn test_iteration_message_mate_notation() {
        let mut info = sample_info();
        info.mate_in = Some(3);
        let msg = iteration_message("", &info);
        assert!(msg.contains("#3"));
    }

    #[test]
    fn test_hidden_bar_for_non_tty() {
        let theme = Theme {
            colors: false,
            interactive: false,
        };
        assert!(spinner(&theme, "x".into()).is_hidden());
        assert!(bar(&theme, 10, "x".into()).is_hidden());
    }

    #[test]
    fn test_thinking_view_is_always_two_lines() {
        colored::control::set_override(false);
        let mut view = ThinkingView::new("thinking".into(), Color::White);
        assert_eq!(view.render().lines().count(), 2, "empty view");
        view.info = Some(sample_info());
        let rendered = view.render();
        assert_eq!(rendered.lines().count(), 2, "populated view");
        assert!(rendered.contains("e2e4"));
    }

    #[test]
    fn test_thinking_view_shows_mate_score() {
        colored::control::set_override(false);
        let mut info = sample_info();
        info.mate_in = Some(-4);
        let view = ThinkingView {
            label: "t".into(),
            info: Some(info),
            side: Color::Black,
            tick: 3,
        };
        assert!(view.render().contains("#-4"));
    }
}
