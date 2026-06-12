//! TTY-gated spinners and progress bars built on `indicatif`.
//!
//! Every animation in the CLI goes through this module so the rule
//! "animations only on interactive terminals" is enforced in one place:
//! when stdout is not a TTY the helpers return hidden progress bars,
//! whose method calls are no-ops, keeping piped output perfectly clean.

use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

use super::score::{format_score, humanize_count};
use super::theme::{Theme, term_width, truncate_chars};
use crate::search::IterationInfo;

/// Tick interval for spinners.
const SPINNER_TICK: Duration = Duration::from_millis(80);

/// Maximum number of PV moves shown on the thinking line.
const PV_PREVIEW_MOVES: usize = 6;

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
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✓"]),
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
        ProgressStyle::with_template("{msg} [{bar:30.cyan/blue}] {pos}/{len} ({eta})")
            .expect("static bar template must parse")
            .progress_chars("█▓░"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info() -> IterationInfo {
        IterationInfo {
            depth: 9,
            score_cp: 35,
            mate_in: None,
            nodes: 1_200_000,
            elapsed_ms: 1500,
            nps: 800_000,
            pv: vec![
                "e2e4".into(),
                "e7e5".into(),
                "g1f3".into(),
                "b8c6".into(),
                "f1b5".into(),
                "a7a6".into(),
                "b5a4".into(),
            ],
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
}
