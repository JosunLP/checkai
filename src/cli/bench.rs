//! `checkai bench` — quick engine strength/regression probe.
//!
//! Runs the search over a fixed suite of six positions (opening,
//! middlegame, tactics, endgame) at a fixed depth or time budget and
//! reports nodes, time and NPS per position plus totals. Because the
//! suite never changes, total node counts are directly comparable
//! between engine versions.

use clap::Args;
use colored::Colorize;

use super::fen;
use super::progress::bar;
use super::score::{format_score, humanize_count};
use super::{CliCommand, CliContext, CliResult, cli_error};
use crate::search::{MAX_DEPTH, SearchEngine, SearchLimits};

/// Transposition table size used for every benchmark run (MB).
const BENCH_TT_MB: usize = 64;

/// The fixed benchmark suite: `(name, FEN)`.
const BENCH_SUITE: [(&str, &str); 6] = [
    ("startpos", fen::START_FEN),
    (
        "kiwipete",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    ),
    ("rook-endgame", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
    (
        "promotion-tactics",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    ),
    (
        "sharp-middlegame",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    ),
    (
        "quiet-middlegame",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    ),
];

/// Arguments for `checkai bench`.
#[derive(Args, Debug)]
#[command(after_help = "\
Examples:\n\
  checkai bench                Run the suite at depth 12\n\
  checkai bench --depth 8      Faster, shallower run\n\
  checkai bench --movetime 1000   1 second per position instead")]
pub struct BenchArgs {
    /// Fixed search depth per position.
    #[arg(long, default_value_t = 12)]
    pub depth: i32,

    /// Time budget per position in milliseconds (replaces --depth).
    #[arg(long)]
    pub movetime: Option<u64>,
}

impl CliCommand for BenchArgs {
    fn run(self, ctx: &CliContext) -> CliResult {
        let limits = match self.movetime {
            Some(ms) => SearchLimits::move_time(ms),
            None => SearchLimits::depth(self.depth.clamp(1, MAX_DEPTH)),
        };

        println!();
        println!("{}", t!("bench.header").to_string().yellow().bold());
        println!();
        println!(
            "  {:<20} {:>6} {:>8} {:>12} {:>9} {:>10}",
            t!("bench.col_position"),
            t!("analyze.col_depth"),
            t!("analyze.col_eval"),
            t!("analyze.col_nodes"),
            t!("bench.col_time"),
            t!("bench.col_nps")
        );

        let pb = bar(
            &ctx.theme,
            BENCH_SUITE.len() as u64,
            t!("bench.progress_label").to_string(),
        );

        let mut total_nodes: u64 = 0;
        let mut total_ms: u64 = 0;
        for (name, fen_str) in BENCH_SUITE {
            let game = fen::game_from_fen(fen_str)
                .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?;
            let pos = fen::search_position(&game);

            // Fresh engine per position: identical conditions every run.
            let mut engine = SearchEngine::new(BENCH_TT_MB);
            let result = engine.search_limited(&pos, &limits, None);

            let nodes = result.stats.nodes;
            // `.max(1)` treats a sub-millisecond search as 1 ms, avoiding a
            // division-by-zero without a separate branch.
            let nps = nodes * 1000 / result.time_ms.max(1);
            total_nodes += nodes;
            total_ms += result.time_ms;

            pb.suspend(|| {
                println!(
                    "  {:<20} {:>6} {:>8} {:>12} {:>8}ms {:>10}",
                    name.cyan(),
                    result.depth,
                    format_score(result.score),
                    nodes,
                    result.time_ms,
                    humanize_count(nps)
                );
            });
            pb.inc(1);
        }
        pb.finish_and_clear();

        let total_nps = total_nodes * 1000 / total_ms.max(1);
        println!();
        println!(
            "{}",
            t!(
                "bench.totals",
                nodes = total_nodes,
                secs = format!("{:.2}", total_ms as f64 / 1000.0),
                nps = humanize_count(total_nps)
            )
            .to_string()
            .bold()
        );
        println!();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bench_suite_fens_are_valid() {
        for (name, fen_str) in BENCH_SUITE {
            let game = fen::game_from_fen(fen_str);
            assert!(game.is_ok(), "bench position '{name}' must parse");
            assert!(
                !game.unwrap().legal_moves().is_empty(),
                "bench position '{name}' must have legal moves"
            );
        }
    }
}
