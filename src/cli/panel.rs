//! Data-driven box-drawing helpers shared by the welcome screen,
//! in-game help tables, and end-of-game result panels.
//!
//! All functions return plain `String`s (color applied by the caller or
//! via `colored`, which respects the global theme override), so output
//! stays clean when piped to a file or another process.

use colored::Colorize;
use std::time::Duration;

use crate::game::Game;
use crate::types::{Color, GameResult};

/// A single row of a two-column command table (`name [alias]  description`).
pub struct TableRow {
    /// Command / item name (left column).
    pub name: String,
    /// Optional short alias shown dimmed next to the name.
    pub alias: Option<String>,
    /// Description (right column).
    pub desc: String,
}

impl TableRow {
    /// Creates a table row from name, optional alias and description.
    pub fn new(name: impl Into<String>, alias: Option<&str>, desc: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            alias: alias.map(str::to_string),
            desc: desc.into(),
        }
    }
}

/// Renders a two-column table with aligned columns.
///
/// The left column (name + alias) is padded to the widest entry so the
/// descriptions line up; colors degrade to plain text automatically.
pub fn render_table(rows: &[TableRow], indent: usize) -> String {
    use super::theme::display_width;
    let left_width = rows
        .iter()
        .map(|r| display_width(&r.name) + r.alias.as_ref().map_or(0, |a| display_width(a) + 3))
        .max()
        .unwrap_or(0);

    let pad = " ".repeat(indent);
    let mut out = String::new();
    for row in rows {
        let raw_left = match &row.alias {
            Some(alias) => format!("{} [{}]", row.name, alias),
            None => row.name.clone(),
        };
        let fill = " ".repeat(left_width.saturating_sub(display_width(&raw_left)));
        let styled_left = match &row.alias {
            Some(alias) => format!(
                "{} {}",
                row.name.green().bold(),
                format!("[{}]", alias).dimmed()
            ),
            None => row.name.green().bold().to_string(),
        };
        out.push_str(&format!("{pad}{styled_left}{fill}  {}\n", row.desc));
    }
    out
}

/// Renders a box-drawn panel with a centered title and content lines.
///
/// `width` is the inner width in characters; lines longer than the inner
/// width are truncated. Returns one string per output row.
pub fn boxed_panel(title: &str, lines: &[String], width: usize) -> Vec<String> {
    let horizontal = "═".repeat(width);
    let mut out = Vec::with_capacity(lines.len() + 4);
    out.push(format!("╔{horizontal}╗"));
    out.push(format!("║{}║", center(title, width)));
    out.push(format!("╟{}╢", "─".repeat(width)));
    for line in lines {
        out.push(format!("║{}║", pad_line(line, width)));
    }
    out.push(format!("╚{horizontal}╝"));
    out
}

/// Centers `text` within `width` display columns (CJK-aware).
fn center(text: &str, width: usize) -> String {
    let len = super::theme::display_width(text);
    if len >= width {
        return super::theme::truncate_chars(text, width);
    }
    let left = (width - len) / 2;
    let right = width - len - left;
    format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
}

/// Left-aligns `text` within `width` display columns with a 1-char margin.
fn pad_line(text: &str, width: usize) -> String {
    let body = super::theme::truncate_chars(text, width.saturating_sub(2));
    let len = super::theme::display_width(&body);
    format!(" {}{}", body, " ".repeat(width.saturating_sub(len + 1)))
}

/// Formats a [`Duration`] as `MM:SS` (or `H:MM:SS` past one hour).
pub fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Renders the stylish end-of-game result panel: winner, reason,
/// move count and wall-clock duration.
pub fn result_panel(game: &Game, duration: Duration) -> String {
    let winner = match game.result {
        Some(GameResult::WhiteWins) => t!("play.winner_white").to_string(),
        Some(GameResult::BlackWins) => t!("play.winner_black").to_string(),
        Some(GameResult::Draw) => t!("play.winner_draw").to_string(),
        None => t!("play.game_unfinished").to_string(),
    };
    let reason = game
        .end_reason
        .as_ref()
        .map(|r| r.to_string())
        .unwrap_or_default();
    let full_moves = game
        .move_history
        .iter()
        .filter(|r| r.side == Color::White)
        .count()
        .max(game.fullmove_number.saturating_sub(1) as usize);

    let lines = vec![
        format!("{}  {}", t!("play.result_winner_label"), winner),
        format!("{}  {}", t!("play.result_reason_label"), reason),
        format!("{}  {}", t!("play.result_moves_label"), full_moves),
        format!(
            "{}  {}",
            t!("play.result_duration_label"),
            format_duration(duration)
        ),
    ];

    let width = lines
        .iter()
        .map(|l| super::theme::display_width(l) + 2)
        .chain(std::iter::once(
            super::theme::display_width(&t!("play.result_title")) + 4,
        ))
        .max()
        .unwrap_or(40)
        .max(36);

    boxed_panel(&t!("play.result_title"), &lines, width)
        .into_iter()
        .map(|l| l.yellow().bold().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(Duration::from_secs(75)), "01:15");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(3725)), "1:02:05");
    }

    #[test]
    fn test_boxed_panel_dimensions() {
        let lines = vec!["hello".to_string()];
        let panel = boxed_panel("T", &lines, 20);
        assert_eq!(panel.len(), 5);
        for row in &panel {
            assert_eq!(row.chars().count(), 22, "row width must be uniform");
        }
    }

    #[test]
    fn test_render_table_aligns_columns() {
        colored::control::set_override(false);
        let rows = vec![
            TableRow::new("a", Some("x"), "first"),
            TableRow::new("longer", None, "second"),
        ];
        let out = render_table(&rows, 2);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("a [x]"));
        assert!(lines[1].contains("longer"));
        // Both descriptions must start at the same column.
        let col0 = lines[0].find("first").unwrap();
        let col1 = lines[1].find("second").unwrap();
        assert_eq!(col0, col1);
    }
}
