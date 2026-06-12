//! The no-subcommand welcome screen.
//!
//! A box-drawn banner, version/locale line, data-driven command table,
//! quick-start section and docs link. On interactive terminals the
//! screen is revealed line by line (a subtle < 400 ms animation, no
//! flicker); piped output prints instantly and plainly.

use std::time::Duration;

use colored::Colorize;

use super::panel::{TableRow, boxed_panel, render_table};
use super::theme::Theme;

/// Per-line delay of the reveal animation. With ~18 lines this stays
/// well under the 400 ms budget.
const REVEAL_DELAY: Duration = Duration::from_millis(16);

/// One welcome-screen command entry: `(name, i18n description key)`.
const COMMANDS: [(&str, &str); 10] = [
    ("serve", "cli.cmd_serve_desc"),
    ("play", "cli.cmd_play_desc"),
    ("watch", "cli.cmd_watch_desc"),
    ("analyze", "cli.cmd_analyze_desc"),
    ("bench", "cli.cmd_bench_desc"),
    ("perft", "cli.cmd_perft_desc"),
    ("uci", "cli.cmd_uci_desc"),
    ("export", "cli.cmd_export_desc"),
    ("update", "cli.cmd_update_desc"),
    ("version", "cli.cmd_version_desc"),
];

/// Quick-start entries: `(shell line, i18n description key)`.
const QUICKSTART: [(&str, &str); 4] = [
    ("$ checkai play", "cli.quickstart_play"),
    ("$ checkai watch", "cli.quickstart_watch"),
    ("$ checkai serve", "cli.quickstart_serve"),
    ("$ checkai <cmd> --help", "cli.quickstart_help"),
];

/// Prints the welcome screen, animated on interactive terminals.
pub fn print_welcome(theme: &Theme) {
    let lines = build_lines();
    if theme.interactive {
        for line in lines {
            println!("{line}");
            std::thread::sleep(REVEAL_DELAY);
        }
    } else {
        println!("{}", lines.join("\n"));
    }
}

/// Builds every output line of the welcome screen.
fn build_lines() -> Vec<String> {
    let version = crate::update::version();
    let locale = rust_i18n::locale().to_string();

    let mut lines: Vec<String> = vec![String::new()];

    // Banner.
    let banner_lines = vec![format!(
        "{}  v{}   ·   {}",
        t!("cli.banner_tagline"),
        version,
        t!("terminal.banner_subtitle")
    )];
    for row in boxed_panel(&t!("cli.welcome_header"), &banner_lines, 53) {
        lines.push(row.cyan().to_string());
    }
    lines.push(String::new());

    // Version / locale line.
    lines.push(format!(
        "  {} {}     {} {}",
        t!("cli.version_label").to_string().bold(),
        version,
        t!("cli.locale_label").to_string().bold(),
        locale
    ));
    lines.push(String::new());

    // Command table (data-driven).
    lines.push(
        t!("cli.commands_header")
            .to_string()
            .yellow()
            .bold()
            .to_string(),
    );
    let rows: Vec<TableRow> = COMMANDS
        .iter()
        .map(|(name, key)| TableRow::new(*name, None, t!(*key).to_string()))
        .collect();
    lines.extend(render_table(&rows, 2).lines().map(str::to_string));
    lines.push(String::new());

    // Quick start.
    lines.push(
        t!("cli.quickstart_header")
            .to_string()
            .yellow()
            .bold()
            .to_string(),
    );
    let quick_rows: Vec<TableRow> = QUICKSTART
        .iter()
        .map(|(cmd, key)| TableRow {
            name: (*cmd).to_string(),
            alias: None,
            desc: t!(*key).to_string(),
        })
        .collect();
    let quick_width = quick_rows
        .iter()
        .map(|r| super::theme::display_width(&r.name))
        .max()
        .unwrap_or(0);
    for row in &quick_rows {
        lines.push(format!(
            "  {}{}  {}",
            row.name.dimmed(),
            " ".repeat(quick_width - super::theme::display_width(&row.name)),
            row.desc
        ));
    }
    lines.push(String::new());

    // Help hint + docs link.
    lines.push(format!(
        "  {}",
        t!("cli.run_help_hint", cmd = "--help".green())
    ));
    lines.push(format!(
        "  {} {}",
        t!("cli.docs_label"),
        "https://github.com/JosunLP/checkai".cyan().underline()
    ));
    lines.push(String::new());
    lines
}
