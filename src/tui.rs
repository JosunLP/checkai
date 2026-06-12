//! Terminal UI toolkit: animations, spinners, progress bars, and themed
//! output shared by the interactive CLI (welcome screen, `play`, `analyze`,
//! `bench`).
//!
//! Every animated effect degrades gracefully. When stdout is **not** an
//! interactive terminal (piped or redirected) or the user opts out via the
//! `NO_COLOR` environment variable, animations are skipped and plain text is
//! printed instead, so logs, pipes, and CI output stay clean and stable.
//!
//! The module is intentionally dependency-light — it builds on the existing
//! [`colored`] crate and the standard library only.

use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use colored::Colorize;

use crate::search::IterationInfo;

/// Smooth 10-frame braille spinner cycle.
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Per-frame delay for the spinner animation.
const SPINNER_INTERVAL: Duration = Duration::from_millis(80);

/// Per-row delay for the animated banner reveal.
const BANNER_ROW_DELAY: Duration = Duration::from_millis(28);

/// ANSI sequence: carriage return + "erase entire line".
const CLEAR_LINE: &str = "\r\x1b[2K";

/// Returns `true` when rich animations should be used: stdout is an
/// interactive terminal and the user has not opted out via `NO_COLOR`.
pub fn animations_enabled() -> bool {
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

// ---------------------------------------------------------------------------
// Spinner
// ---------------------------------------------------------------------------

/// A background, thread-driven spinner for indeterminate operations such as
/// network calls or loading data.
///
/// Construct with [`Spinner::start`]; always end it with [`Spinner::finish`]
/// or [`Spinner::fail`] (or simply drop it) so the worker thread is joined
/// and the line is cleared.
pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    enabled: bool,
}

impl Spinner {
    /// Starts a spinner with the given message.
    ///
    /// When animations are disabled the message is printed once as a plain
    /// line and no background thread is spawned.
    pub fn start(message: impl Into<String>) -> Self {
        let message = message.into();
        let enabled = animations_enabled();

        if !enabled {
            println!("{message}");
            return Self {
                running: Arc::new(AtomicBool::new(false)),
                handle: None,
                enabled,
            };
        }

        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        let handle = thread::spawn(move || {
            let mut frame = 0usize;
            while thread_running.load(Ordering::Relaxed) {
                print!(
                    "{CLEAR_LINE}{} {}",
                    SPINNER_FRAMES[frame % SPINNER_FRAMES.len()].cyan().bold(),
                    message
                );
                let _ = io::stdout().flush();
                frame += 1;
                thread::sleep(SPINNER_INTERVAL);
            }
        });

        Self {
            running,
            handle: Some(handle),
            enabled,
        }
    }

    /// Stops the spinner and prints a green success line.
    pub fn finish(mut self, message: impl AsRef<str>) {
        self.stop_thread();
        if self.enabled {
            println!("{CLEAR_LINE}{} {}", "✔".green().bold(), message.as_ref());
        } else {
            println!("{}", message.as_ref());
        }
    }

    /// Stops the spinner and prints a red failure line to stderr.
    pub fn fail(mut self, message: impl AsRef<str>) {
        self.stop_thread();
        if self.enabled {
            eprintln!("{CLEAR_LINE}{} {}", "✖".red().bold(), message.as_ref());
        } else {
            eprintln!("{}", message.as_ref());
        }
    }

    /// Signals the worker thread to stop and joins it.
    fn stop_thread(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop_thread();
    }
}

// ---------------------------------------------------------------------------
// Progress bar
// ---------------------------------------------------------------------------

/// Renders a unicode progress bar of the given character `width` for a
/// `fraction` in `[0.0, 1.0]`, e.g. `"████████░░░░  62%"`.
pub fn progress_bar(fraction: f64, width: usize) -> String {
    let fraction = fraction.clamp(0.0, 1.0);
    let filled = (fraction * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    let bar = format!(
        "{}{}",
        "█".repeat(filled).green(),
        "░".repeat(empty).dimmed()
    );
    format!("{bar} {:>3}%", (fraction * 100.0).round() as u32)
}

// ---------------------------------------------------------------------------
// Score formatting
// ---------------------------------------------------------------------------

/// Formats an engine score for display: `"#3"` / `"#-2"` for forced mates,
/// otherwise centipawns as a signed pawn value (`"+0.45"`, `"-1.20"`).
///
/// The score is from the side-to-move's perspective.
pub fn format_score(score_cp: i32, mate_in: Option<i32>) -> String {
    match mate_in {
        Some(n) => format!("#{n}"),
        None => format!("{:+.2}", score_cp as f64 / 100.0),
    }
}

/// Colours a score string: green when clearly winning for the side to move,
/// red when clearly losing, neutral otherwise.
pub fn colorize_score(score_cp: i32, mate_in: Option<i32>) -> String {
    let text = format_score(score_cp, mate_in);
    let winning = mate_in.map(|n| n > 0).unwrap_or(score_cp >= 50);
    let losing = mate_in.map(|n| n < 0).unwrap_or(score_cp <= -50);
    if winning {
        text.green().bold().to_string()
    } else if losing {
        text.red().bold().to_string()
    } else {
        text.yellow().to_string()
    }
}

/// Formats a large count compactly: `1234` → `1.2K`, `3_400_000` → `3.4M`.
pub fn humanize_count(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}G", n as f64 / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

// ---------------------------------------------------------------------------
// Live search progress
// ---------------------------------------------------------------------------

/// A live, single-line view of an in-progress engine search. Each completed
/// iterative-deepening iteration overwrites the previous line via a carriage
/// return; when animations are disabled, every iteration prints its own line.
pub struct SearchProgressView {
    enabled: bool,
}

impl Default for SearchProgressView {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchProgressView {
    /// Creates a new progress view, detecting terminal capabilities once.
    pub fn new() -> Self {
        Self {
            enabled: animations_enabled(),
        }
    }

    /// Renders one iteration snapshot.
    pub fn update(&self, info: &IterationInfo) {
        let depth = format!("{} {:>2}", t!("engine.label_depth"), info.depth);
        let score = colorize_score(info.score_cp, info.mate_in);
        let nodes = humanize_count(info.nodes);
        let nps = humanize_count(info.nps);
        let time = format!("{:.1}s", info.elapsed_ms as f64 / 1000.0);
        let pv = Self::truncate_pv(&info.pv, 6);

        let line = format!(
            "  {}  {} {}  {} {}  {} {}  {} {}  {} {}",
            depth.cyan().bold(),
            t!("engine.label_score"),
            score,
            t!("engine.label_nodes"),
            nodes.dimmed(),
            t!("engine.label_nps"),
            nps.dimmed(),
            t!("engine.label_time"),
            time.dimmed(),
            t!("engine.label_pv"),
            pv.normal(),
        );

        if self.enabled {
            print!("{CLEAR_LINE}{line}");
            let _ = io::stdout().flush();
        } else {
            println!("{line}");
        }
    }

    /// Ends the live line, leaving the final iteration visible.
    pub fn finish(&self) {
        if self.enabled {
            println!();
        }
    }

    /// Joins the first `max` principal-variation moves into a string.
    fn truncate_pv(pv: &[String], max: usize) -> String {
        if pv.is_empty() {
            return "—".to_string();
        }
        let shown = pv.iter().take(max).cloned().collect::<Vec<_>>().join(" ");
        if pv.len() > max {
            format!("{shown} …")
        } else {
            shown
        }
    }
}

// ---------------------------------------------------------------------------
// Banners & dividers
// ---------------------------------------------------------------------------

/// Prints a block of pre-formatted lines as an animated reveal (one row at a
/// time) when animations are enabled, or instantly otherwise. The `colorize`
/// closure styles each row.
pub fn animated_banner<F>(lines: &[String], colorize: F)
where
    F: Fn(&str) -> String,
{
    let animate = animations_enabled();
    for line in lines {
        println!("{}", colorize(line));
        if animate {
            thread::sleep(BANNER_ROW_DELAY);
        }
    }
}

/// Returns a horizontal divider of the given `width` using a light box rule.
pub fn divider(width: usize) -> String {
    "─".repeat(width).dimmed().to_string()
}
