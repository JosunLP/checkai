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

/// White's winning expectancy for a centipawn score, via the standard
/// logistic curve `1 / (1 + 10^(-cp/400))`. Always within `0.0..=1.0`.
pub fn win_probability(score_cp_white: i32) -> f64 {
    let cp = f64::from(score_cp_white.clamp(-3000, 3000));
    1.0 / (1.0 + 10f64.powf(-cp / 400.0))
}

/// Renders a pure-text evaluation bar of `width` cells.
///
/// The filled (`█`) portion is White's winning expectancy derived from
/// the centipawn score (White's perspective); the rest is `░`.
pub fn eval_bar(score_cp_white: i32, width: usize) -> String {
    let filled = ((win_probability(score_cp_white) * width as f64).round() as usize).min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// Renders a colour-graded evaluation bar.
///
/// The White share is drawn in a warm-to-cool gradient that tracks how
/// decisive the advantage is, so the bar reads at a glance even without
/// looking at the number.
///
/// Falls back to the plain `█`/`░` bar unless colours are actually going to be
/// emitted — a gradient bar with its escape codes stripped would be 100% solid
/// blocks and read as "White is winning" no matter the score.
pub fn eval_bar_gradient(score_cp_white: i32, width: usize) -> String {
    let colorized = colored::control::SHOULD_COLORIZE.should_colorize();
    if !colorized || !super::board_renderer::supports_truecolor() {
        return eval_bar(score_cp_white, width);
    }
    let share = win_probability(score_cp_white);
    let filled = ((share * width as f64).round() as usize).min(width);
    let mut out = String::with_capacity(width * 20);
    for cell in 0..width {
        // Position of this cell along the bar, 0.0 (Black) … 1.0 (White).
        let position = (cell as f64 + 0.5) / width as f64;
        let (r, g, b) = if cell < filled {
            gradient(position)
        } else {
            (58, 58, 66)
        };
        out.push_str(&"█".truecolor(r, g, b).to_string());
    }
    out
}

/// Colour ramp for the evaluation bar: deep blue (Black winning) through
/// neutral grey-green to bright amber (White winning).
fn gradient(position: f64) -> (u8, u8, u8) {
    let stops: [(f64, (u8, u8, u8)); 5] = [
        (0.00, (66, 110, 190)),
        (0.35, (110, 160, 200)),
        (0.50, (200, 200, 195)),
        (0.65, (222, 190, 120)),
        (1.00, (240, 200, 70)),
    ];
    let position = position.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let (p0, c0) = pair[0];
        let (p1, c1) = pair[1];
        if position <= p1 {
            let t = if (p1 - p0).abs() < f64::EPSILON {
                0.0
            } else {
                (position - p0) / (p1 - p0)
            };
            let mix =
                |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u8;
            return (mix(c0.0, c1.0), mix(c0.1, c1.1), mix(c0.2, c1.2));
        }
    }
    stops[stops.len() - 1].1
}

/// Renders the full eval-bar line: labeled ends plus the formatted score.
///
/// Example: `W ███████████░░░░░░░░░░ B  +0.35`
pub fn eval_bar_line(score_cp_white: i32) -> String {
    format!(
        "{} {} {}  {}",
        "W".white().bold(),
        eval_bar_gradient(score_cp_white, EVAL_BAR_WIDTH),
        "B".blue().bold(),
        format_score(score_cp_white).cyan().bold()
    )
}

/// Renders a compact sparkline of an evaluation curve.
///
/// Each value is a centipawn score from White's perspective; the glyph
/// height tracks White's winning expectancy, so the line reads like a
/// game-long momentum graph.
pub fn sparkline(scores: &[i32]) -> String {
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    scores
        .iter()
        .map(|cp| {
            let level = (win_probability(*cp) * (LEVELS.len() - 1) as f64).round() as usize;
            LEVELS[level.min(LEVELS.len() - 1)]
        })
        .collect()
}

/// Upper bound on a single move's centipawn loss when averaging.
///
/// A move that walks into mate produces a loss in the tens of thousands.
/// Averaged in raw, one such move swamps every other move in the game — a
/// four-move miniature reports an "average loss" of 15 000 cp — and pins
/// accuracy at 0.0% no matter how the rest was played. Past a few pawns the
/// magnitude carries no information anyway: the move simply loses.
pub const MAX_COUNTED_CP_LOSS: i32 = 1_000;

/// Clamps one move's loss to the range the accuracy average can represent.
///
/// This is the aggregate counterpart to [`format_cp_loss`], which already
/// refuses to print a mate-range loss as a pawn count.
pub fn counted_cp_loss(loss_cp: i32) -> i32 {
    loss_cp.clamp(0, MAX_COUNTED_CP_LOSS)
}

/// Converts an average centipawn loss into a 0–100 accuracy percentage.
///
/// Uses the exponential decay `100 * e^(-loss / 90)`, which maps a flawless
/// game to 100%, ~10 cp average loss to ~89%, and 100 cp to ~33% — close to
/// the scale players know from online analysis boards.
pub fn accuracy_from_cp_loss(average_cp_loss: f64) -> f64 {
    (100.0 * (-average_cp_loss.max(0.0) / 90.0).exp()).clamp(0.0, 100.0)
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

    #[test]
    fn test_win_probability_is_symmetric() {
        assert!((win_probability(0) - 0.5).abs() < 1e-9);
        assert!((win_probability(200) + win_probability(-200) - 1.0).abs() < 1e-9);
        assert!(win_probability(3000) > 0.99);
        assert!(win_probability(-3000) < 0.01);
    }

    #[test]
    fn test_sparkline_tracks_the_curve() {
        let line = sparkline(&[-2000, 0, 2000]);
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(chars.len(), 3);
        assert_eq!(chars[0], '▁');
        assert_eq!(chars[2], '█');
        assert!(chars[1] != chars[0] && chars[1] != chars[2]);
    }

    #[test]
    fn test_counted_cp_loss_caps_mate_range_losses() {
        assert_eq!(counted_cp_loss(150), 150);
        assert_eq!(counted_cp_loss(-20), 0, "a gain is not a loss");
        // Walking into mate must not swamp the game average on its own.
        assert_eq!(counted_cp_loss(MATE_SCORE), MAX_COUNTED_CP_LOSS);
        let mated_game = accuracy_from_cp_loss(f64::from(counted_cp_loss(MATE_SCORE)) / 4.0);
        assert!(
            mated_game > 0.0 && mated_game < 10.0,
            "a four-move miniature ending in mate should read as a very low \
             accuracy, not as an average loss of 15000 cp, got {mated_game}"
        );
    }

    #[test]
    fn test_accuracy_scale() {
        assert!((accuracy_from_cp_loss(0.0) - 100.0).abs() < 1e-6);
        let good = accuracy_from_cp_loss(10.0);
        let bad = accuracy_from_cp_loss(150.0);
        assert!(good > 85.0 && good < 95.0, "got {good}");
        assert!(bad < 25.0, "got {bad}");
        assert!(accuracy_from_cp_loss(-5.0) <= 100.0);
    }

    #[test]
    fn test_gradient_bar_degrades_when_colors_are_off() {
        // Without colour the gradient bar must fall back to the plain bar:
        // a gradient bar with its escape codes stripped would be all solid
        // blocks and read as "White is winning" at every score.
        colored::control::set_override(false);
        let balanced = eval_bar_gradient(0, EVAL_BAR_WIDTH);
        assert_eq!(balanced.chars().count(), EVAL_BAR_WIDTH);
        let filled = balanced.chars().filter(|&c| c == '█').count();
        assert!(
            (10..=11).contains(&filled),
            "a dead-equal score must fill about half the bar, got {filled}"
        );
        assert!(eval_bar_gradient(-2000, EVAL_BAR_WIDTH).starts_with('░'));
    }
}
