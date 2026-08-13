//! Shared engine configuration for every engine-backed CLI command.
//!
//! `play`, `watch`, `analyze`, `bench` and `eval` all drive the same
//! [`SearchEngine`], so they all accept the same knobs: thread count, hash
//! size, MultiPV width, an optional Polyglot opening book and an optional
//! Syzygy tablebase directory. Collecting them in one [`EngineArgs`] group
//! keeps the flags identical everywhere and puts book/tablebase loading —
//! including the "what did I actually load" report — in a single place.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Args;
use colored::Colorize;

use super::theme::Theme;
use crate::opening_book::OpeningBook;
use crate::search::{EngineConfig, MAX_MULTI_PV, MAX_THREADS, SearchEngine};
use crate::tablebase::SyzygyTablebase;

/// Hash size used when a command does not specify its own default.
pub const DEFAULT_HASH_MB: usize = 64;

/// Engine knobs shared by every command that runs a search.
#[derive(Args, Debug, Clone, Default)]
pub struct EngineArgs {
    /// Search threads (Lazy SMP). Defaults to 1; `0` = one per CPU core.
    #[arg(long, value_name = "N", help_heading = "Engine")]
    pub threads: Option<usize>,

    /// Transposition table size in MB.
    #[arg(long, value_name = "MB", help_heading = "Engine")]
    pub hash: Option<usize>,

    /// Node budget per search (overrides the time budget when smaller).
    #[arg(long, value_name = "N", help_heading = "Engine")]
    pub nodes: Option<u64>,

    /// Report this many principal variations (1–16).
    #[arg(long, value_name = "N", help_heading = "Engine")]
    pub multipv: Option<usize>,

    /// Polyglot opening book to consult at the root (`.bin`).
    #[arg(long, value_name = "FILE", help_heading = "Knowledge")]
    pub book: Option<PathBuf>,

    /// Always play the most popular book move instead of sampling by weight.
    #[arg(long, help_heading = "Knowledge")]
    pub book_best: bool,

    /// Syzygy tablebase directory to probe in the endgame.
    #[arg(long, value_name = "DIR", help_heading = "Knowledge")]
    pub tablebase: Option<PathBuf>,
}

/// Resolves `--threads`: `None` → 1, `Some(0)` → one per available core.
pub fn resolve_threads(requested: Option<usize>) -> usize {
    match requested {
        None => 1,
        Some(0) => available_cores(),
        Some(n) => n.clamp(1, MAX_THREADS),
    }
}

/// Number of usable CPU cores, or 1 when the platform will not say.
pub fn available_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, MAX_THREADS)
}

impl EngineArgs {
    /// Builds an [`EngineConfig`], loading the opening book and tablebase.
    ///
    /// `default_hash_mb` is the command's own preferred table size, used when
    /// the user did not pass `--hash`. Loading failures are reported on stderr
    /// and degrade to "no book" / "no tablebase" rather than aborting the
    /// command — a missing book should never stop a game.
    pub fn build_config(&self, default_hash_mb: usize) -> EngineConfig {
        let book = self
            .book
            .as_ref()
            .and_then(|path| match OpeningBook::load(path) {
                Ok(book) => Some(Arc::new(book)),
                Err(err) => {
                    eprintln!(
                        "{}: {}",
                        t!("engine.book_load_failed").to_string().yellow(),
                        err
                    );
                    None
                }
            });

        let tablebase =
            self.tablebase
                .as_ref()
                .and_then(|path| match SyzygyTablebase::load(path) {
                    Ok(tb) => Some(Arc::new(tb)),
                    Err(err) => {
                        eprintln!(
                            "{}: {}",
                            t!("engine.tablebase_load_failed").to_string().yellow(),
                            err
                        );
                        None
                    }
                });

        EngineConfig {
            tt_size_mb: self.hash.unwrap_or(default_hash_mb).max(1),
            threads: resolve_threads(self.threads),
            multi_pv: self.multipv.unwrap_or(1).clamp(1, MAX_MULTI_PV),
            move_overhead_ms: 0,
            book,
            use_book: true,
            book_variety: !self.book_best,
            tablebase,
            skill_level: None,
        }
    }

    /// Convenience: build a ready-to-use engine for this command.
    pub fn build_engine(&self, default_hash_mb: usize) -> SearchEngine {
        SearchEngine::with_config(self.build_config(default_hash_mb))
    }
}

/// Prints a one-line summary of the active engine configuration.
///
/// Skipped entirely on non-interactive output so piped results stay clean.
pub fn print_engine_banner(theme: &Theme, config: &EngineConfig) {
    if !theme.interactive {
        return;
    }
    let mut parts = vec![
        format!("{} {}", t!("engine.threads_label"), config.threads),
        format!("{} {} MB", t!("engine.hash_label"), config.tt_size_mb),
    ];
    if config.multi_pv > 1 {
        parts.push(format!(
            "{} {}",
            t!("engine.multipv_label"),
            config.multi_pv
        ));
    }
    if let Some(book) = &config.book {
        parts.push(t!("engine.book_label", entries = book.len()).to_string());
    }
    if let Some(tb) = &config.tablebase {
        parts.push(t!("engine.tablebase_label", pieces = tb.max_pieces).to_string());
    }
    println!("  {}", parts.join("  ·  ").dimmed());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_threads() {
        assert_eq!(resolve_threads(None), 1);
        assert_eq!(resolve_threads(Some(4)), 4);
        assert_eq!(resolve_threads(Some(0)), available_cores());
        assert_eq!(resolve_threads(Some(usize::MAX)), MAX_THREADS);
    }

    #[test]
    fn test_build_config_defaults() {
        let args = EngineArgs::default();
        let config = args.build_config(32);
        assert_eq!(config.tt_size_mb, 32);
        assert_eq!(config.threads, 1);
        assert_eq!(config.multi_pv, 1);
        assert!(config.book.is_none());
        assert!(config.tablebase.is_none());
        // Sampling is the default so book play is not perfectly repetitive.
        assert!(config.book_variety);
    }

    #[test]
    fn test_build_config_clamps_multipv() {
        let args = EngineArgs {
            multipv: Some(999),
            hash: Some(0),
            ..EngineArgs::default()
        };
        let config = args.build_config(64);
        assert_eq!(config.multi_pv, MAX_MULTI_PV);
        assert_eq!(config.tt_size_mb, 1, "hash must stay at least 1 MB");
    }

    #[test]
    fn test_missing_book_degrades_gracefully() {
        let args = EngineArgs {
            book: Some(PathBuf::from("/nonexistent/book.bin")),
            tablebase: Some(PathBuf::from("/nonexistent/tb")),
            ..EngineArgs::default()
        };
        let config = args.build_config(16);
        assert!(config.book.is_none());
        assert!(config.tablebase.is_none());
    }
}
