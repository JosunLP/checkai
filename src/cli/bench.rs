//! `checkai bench` — engine strength/regression probe.
//!
//! Runs the search over a fixed suite of twelve positions (opening,
//! middlegame, tactics, endgame) at a fixed depth or time budget and
//! reports nodes, time and NPS per position plus totals. Because the
//! suite never changes, single-threaded total node counts are directly
//! comparable between engine versions — that number is the regression
//! signal, so it is printed as a "signature" at the end.
//!
//! `--threads` measures the Lazy SMP speed-up instead; those runs are
//! non-deterministic by design and should be compared on time, not nodes.

use clap::Args;
use colored::Colorize;

use super::engine::EngineArgs;
use super::fen;
use super::progress::bar;
use super::score::{format_score, humanize_count};
use super::{CliCommand, CliContext, CliResult, cli_error};
use crate::search::{MAX_DEPTH, SearchLimits};

/// Transposition table size used for every benchmark run (MB).
const BENCH_TT_MB: usize = 64;

/// The fixed benchmark suite: `(name, FEN)`.
pub const BENCH_SUITE: [(&str, &str); 12] = [
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
    (
        "closed-centre",
        "r1bqk2r/pp2bppp/2n1pn2/2pp4/3P1B2/2PBPN2/PP1N1PPP/R2QK2R w KQkq - 0 8",
    ),
    (
        "open-tactics",
        "2rq1rk1/pp1bppbp/3p1np1/8/2BNP3/2N1BP2/PPPQ2PP/2KR3R w - - 0 12",
    ),
    (
        "pawn-endgame",
        "8/pp3p1k/2p2q1p/3r1P2/5R2/7P/P1P1QP2/7K b - - 0 30",
    ),
    (
        "knight-outpost",
        "r2q1rk1/1b1nbppp/p2ppn2/1p6/3NPP2/1BN1B3/PPPQ2PP/2KR3R w - - 0 13",
    ),
    ("mate-hunt", "6k1/5ppp/8/8/8/8/1Q3PPP/6K1 w - - 0 1"),
    ("fortress", "8/8/4kpp1/3p1b2/p6P/2B5/6P1/6K1 w - - 0 1"),
];

/// Arguments for `checkai bench`.
#[derive(Args, Debug)]
#[command(after_help = "\
Examples:\n\
  checkai bench                Run the suite at depth 12\n\
  checkai bench --depth 8      Faster, shallower run\n\
  checkai bench --movetime 1000   1 second per position instead\n\
  checkai bench --threads 4    Measure the Lazy SMP speed-up\n\
  checkai bench --hash 256     Benchmark with a bigger table")]
pub struct BenchArgs {
    /// Fixed search depth per position.
    #[arg(long, default_value_t = 12)]
    pub depth: i32,

    /// Time budget per position in milliseconds (replaces --depth).
    #[arg(long)]
    pub movetime: Option<u64>,

    #[command(flatten)]
    pub engine: EngineArgs,
}

impl CliCommand for BenchArgs {
    fn run(self, ctx: &CliContext) -> CliResult {
        let limits = match self.movetime {
            Some(ms) => SearchLimits {
                move_time_ms: Some(ms),
                max_nodes: self.engine.nodes,
                ..SearchLimits::default()
            },
            None => SearchLimits {
                max_depth: self.depth.clamp(1, MAX_DEPTH),
                max_nodes: self.engine.nodes,
                ..SearchLimits::default()
            },
        };
        let mut config = self.engine.build_config(BENCH_TT_MB);
        // A book would answer opening positions instantly and make the
        // benchmark meaningless.
        config.use_book = false;
        config.multi_pv = 1;

        println!();
        println!("{}", t!("bench.header").to_string().yellow().bold());
        super::engine::print_engine_banner(&ctx.theme, &config);
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
            let position = fen::search_position(&game);

            // Fresh engine per position: identical conditions every run.
            let mut engine = crate::search::SearchEngine::with_config(config.clone());
            let result = engine.search_limited(&position, &limits, None);

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
                    format!("{}/{}", result.depth, result.seldepth),
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
        if config.threads == 1 {
            println!(
                "{}",
                t!("bench.signature", nodes = total_nodes)
                    .to_string()
                    .dimmed()
            );
        } else {
            println!(
                "{}",
                t!("bench.threaded_note", threads = config.threads)
                    .to_string()
                    .dimmed()
            );
        }
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
            assert!(game.is_ok(), "bench position '{name}' must parse: {game:?}");
            assert!(
                !game.unwrap().legal_moves().is_empty(),
                "bench position '{name}' must have legal moves"
            );
        }
    }

    #[test]
    fn test_bench_suite_names_are_unique() {
        let mut names: Vec<&str> = BENCH_SUITE.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(
            names.len(),
            count,
            "benchmark position names must be unique"
        );
    }
}
