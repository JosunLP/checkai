//! Score formatting and the one-line evaluation bar.
//!
//! All functions here are pure (no I/O, no color) so they are easy to
//! unit-test; callers apply styling on top.

use colored::Colorize;

use crate::search::score_to_mate_in;
use crate::types::Color;

/// Default width (in cells) of the evaluation bar.
pub const EVAL_BAR_WIDTH: usize = 21;

/// Converts a side-to-move-relative centipawn score to White's
/// perspective (positive = White is better).
pub fn white_pov(score_cp: i32, side_to_move: Color) -> i32 {
    match side_to_move {
        Color::White => score_cp,
        Color::Black => -score_cp,
    }
}

/// Formats a centipawn score as pawns (`+1.23`) or mate distance (`#3`,
/// `#-2`). The score is interpreted from the perspective it was produced
/// in; mate signs follow that perspective.
pub fn format_score(score_cp: i32) -> String {
    match score_to_mate_in(score_cp) {
        Some(mate) => format!("#{mate}"),
        None => format!("{:+.2}", f64::from(score_cp) / 100.0),
    }
}

/// Formats a centipawn *loss* (evaluation drop) for the annotation table.
///
/// A loss derived from mate-range scores is not a meaningful pawn count
/// (it would print as a five-digit number next to a `#N` eval), so it
/// renders as an em dash instead.
pub fn format_cp_loss(loss_cp: i32) -> String {
    if score_to_mate_in(loss_cp).is_some() {
        "—".to_string()
    } else {
        loss_cp.to_string()
    }
}

/// Renders a pure-text evaluation bar of `width` cells.
///
/// The filled (`█`) portion is White's winning expectancy derived from
/// the centipawn score (White's perspective) through the standard
/// logistic curve `1 / (1 + 10^(-cp/400))`; the rest is `░`.
pub fn eval_bar(score_cp_white: i32, width: usize) -> String {
    let cp = f64::from(score_cp_white.clamp(-3000, 3000));
    let white_share = 1.0 / (1.0 + 10f64.powf(-cp / 400.0));
    let filled = ((white_share * width as f64).round() as usize).min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// Renders the full eval-bar line: labeled ends plus the formatted score.
///
/// Example: `W ███████████░░░░░░░░░░ B  +0.35`
pub fn eval_bar_line(score_cp_white: i32) -> String {
    format!(
        "{} {} {}  {}",
        "W".white().bold(),
        eval_bar(score_cp_white, EVAL_BAR_WIDTH),
        "B".blue().bold(),
        format_score(score_cp_white).cyan().bold()
    )
}

/// Formats a node/nps count in compact human form (`950`, `8.5k`, `1.2M`).
pub fn humanize_count(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}k", n as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.1}M", n as f64 / 1_000_000.0),
        _ => format!("{:.2}G", n as f64 / 1_000_000_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::MATE_SCORE;

    #[test]
    fn test_format_score_pawns() {
        assert_eq!(format_score(123), "+1.23");
        assert_eq!(format_score(-50), "-0.50");
        assert_eq!(format_score(0), "+0.00");
    }

    #[test]
    fn test_format_score_mate() {
        // Mate in 3 plies => 2 full moves.
        assert_eq!(format_score(MATE_SCORE - 3), "#2");
        assert_eq!(format_score(-(MATE_SCORE - 4)), "#-2");
    }

    #[test]
    fn test_white_pov_negates_for_black() {
        assert_eq!(white_pov(80, Color::White), 80);
        assert_eq!(white_pov(80, Color::Black), -80);
    }

    #[test]
    fn test_eval_bar_balanced() {
        let bar = eval_bar(0, EVAL_BAR_WIDTH);
        assert_eq!(bar.chars().count(), EVAL_BAR_WIDTH);
        let filled = bar.chars().filter(|&c| c == '█').count();
        // A dead-equal score must fill roughly half the bar.
        assert!((10..=11).contains(&filled), "got {filled} filled cells");
    }

    #[test]
    fn test_eval_bar_extremes() {
        let winning = eval_bar(MATE_SCORE, EVAL_BAR_WIDTH);
        assert_eq!(winning.chars().filter(|&c| c == '█').count(), 21);
        let losing = eval_bar(-MATE_SCORE, EVAL_BAR_WIDTH);
        assert_eq!(losing.chars().filter(|&c| c == '█').count(), 0);
    }

    #[test]
    fn test_eval_bar_monotonic() {
        let f = |cp| eval_bar(cp, EVAL_BAR_WIDTH).matches('█').count();
        assert!(f(-300) < f(0));
        assert!(f(0) < f(300));
    }

    #[test]
    fn test_format_cp_loss_dashes_mates() {
        assert_eq!(format_cp_loss(150), "150");
        assert_eq!(format_cp_loss(0), "0");
        // A loss in mate range is not a meaningful pawn count.
        assert_eq!(format_cp_loss(MATE_SCORE - 3), "—");
    }

    #[test]
    fn test_humanize_count() {
        assert_eq!(humanize_count(950), "950");
        assert_eq!(humanize_count(8_500), "8.5k");
        assert_eq!(humanize_count(1_200_000), "1.2M");
    }
}
