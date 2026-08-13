//! `checkai perft` — move-generation verification.
//!
//! Counts leaf nodes of the legal-move tree to the requested depth.
//! For the standard starting position the well-known reference totals
//! (20, 400, 8 902, …) are printed next to the computed values with
//! OK/FAIL markers, making this a one-command movegen sanity check.
//! `--divide` prints per-root-move subtotals, the standard tool for
//! pinpointing movegen bugs.

use std::time::Instant;

use clap::Args;
use colored::Colorize;

use super::fen;
use super::score::humanize_count;
use super::{CliCommand, CliContext, CliResult, cli_error};
use crate::search::SearchPosition;
use crate::types::ChessMove;

/// Known perft totals for the standard starting position, depths 1–6.
pub const STARTPOS_REFERENCE: [u64; 6] = [20, 400, 8_902, 197_281, 4_865_609, 119_060_324];

/// Arguments for `checkai perft`.
#[derive(Args, Debug)]
#[command(after_help = "\
Examples:\n\
  checkai perft                Verify startpos depths 1-5 vs references\n\
  checkai perft 6              Up to depth 6 (slow but exact)\n\
  checkai perft 4 --divide     Per-root-move counts at depth 4\n\
  checkai perft 6 --threads 0  Use every CPU core\n\
  checkai perft 5 --fen \"<FEN>\"   Count nodes for a custom position")]
pub struct PerftArgs {
    /// Search depth in plies (1-7).
    #[arg(default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..=7))]
    pub depth: u32,

    /// Position to count from (default: standard starting position).
    #[arg(long)]
    pub fen: Option<String>,

    /// Print per-root-move counts at the target depth.
    #[arg(long)]
    pub divide: bool,

    /// Worker threads for the count (`0` = one per CPU core).
    #[arg(long, value_name = "N")]
    pub threads: Option<usize>,
}

impl CliCommand for PerftArgs {
    fn run(self, _ctx: &CliContext) -> CliResult {
        let is_startpos = self.fen.is_none();
        let fen_str = self.fen.as_deref().unwrap_or(fen::START_FEN);
        let game = fen::game_from_fen(fen_str)
            .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?;
        let pos = fen::search_position(&game);
        let threads = super::engine::resolve_threads(self.threads);

        println!();
        println!("{}", t!("perft.header").to_string().yellow().bold());
        println!("  {}", fen::game_to_fen(&game).dimmed());
        println!();

        if self.divide {
            run_divide(&pos, self.depth);
            return Ok(());
        }

        println!(
            "  {:>5} {:>14} {:>14} {:>9} {:>10}  {}",
            t!("analyze.col_depth"),
            t!("perft.col_nodes"),
            t!("perft.col_reference"),
            t!("bench.col_time"),
            t!("bench.col_nps"),
            t!("perft.col_status")
        );

        let mut all_ok = true;
        for depth in 1..=self.depth {
            let start = Instant::now();
            let nodes = perft_parallel(&pos, depth, threads);
            let elapsed = start.elapsed();
            let ms = elapsed.as_millis() as u64;
            // `.max(1)` treats a sub-millisecond run as 1 ms (no div-by-zero).
            let nps = nodes * 1000 / ms.max(1);

            let reference = if is_startpos {
                STARTPOS_REFERENCE.get((depth - 1) as usize).copied()
            } else {
                None
            };
            let (ref_str, status) = match reference {
                Some(expected) if expected == nodes => (
                    expected.to_string(),
                    t!("perft.ok").to_string().green().bold(),
                ),
                Some(expected) => {
                    all_ok = false;
                    (
                        expected.to_string(),
                        t!("perft.fail").to_string().red().bold(),
                    )
                }
                None => (String::from("—"), "".normal()),
            };
            println!(
                "  {:>5} {:>14} {:>14} {:>8}ms {:>10}  {}",
                depth,
                nodes,
                ref_str,
                ms,
                humanize_count(nps),
                status
            );
        }

        println!();
        if is_startpos {
            if all_ok {
                println!("{}", t!("perft.all_ok").to_string().green().bold());
            } else {
                println!("{}", t!("perft.mismatch").to_string().red().bold());
            }
            println!();
        }
        Ok(())
    }
}

/// Counts leaf nodes of the legal-move tree at the given depth.
pub fn perft(pos: &SearchPosition, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = pos.legal_moves();
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut total = 0u64;
    for mv in moves {
        total += perft(&pos.make_move(&mv), depth - 1);
    }
    total
}

/// Counts leaf nodes using `threads` worker threads.
///
/// Root moves are dealt out round-robin, which keeps the split cheap and
/// balances well in practice because sibling subtrees are similar in size.
/// The result is identical to [`perft`] — only the wall-clock time changes.
pub fn perft_parallel(pos: &SearchPosition, depth: u32, threads: usize) -> u64 {
    let threads = threads.max(1);
    if threads == 1 || depth <= 1 {
        return perft(pos, depth);
    }
    let moves = pos.legal_moves();
    let workers = threads.min(moves.len().max(1));

    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let slice: Vec<_> = moves
                    .iter()
                    .skip(worker)
                    .step_by(workers)
                    .map(|mv| pos.make_move(mv))
                    .collect();
                scope.spawn(move || {
                    slice
                        .iter()
                        .map(|child| perft(child, depth - 1))
                        .sum::<u64>()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or(0))
            .sum()
    })
}

/// Computes per-root-move subtotals at the given depth.
pub fn perft_divide(pos: &SearchPosition, depth: u32) -> Vec<(ChessMove, u64)> {
    pos.legal_moves()
        .into_iter()
        .map(|mv| {
            let count = if depth <= 1 {
                1
            } else {
                perft(&pos.make_move(&mv), depth - 1)
            };
            (mv, count)
        })
        .collect()
}

/// Prints the divide table plus the grand total.
fn run_divide(pos: &SearchPosition, depth: u32) {
    let start = Instant::now();
    let entries = perft_divide(pos, depth);
    let total: u64 = entries.iter().map(|(_, n)| n).sum();
    let ms = start.elapsed().as_millis() as u64;

    for (mv, count) in &entries {
        println!("  {:<8} {:>14}", mv.to_string().green(), count);
    }
    println!();
    println!(
        "{}",
        t!(
            "perft.divide_total",
            total = total,
            moves = entries.len(),
            ms = ms
        )
        .to_string()
        .bold()
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn startpos() -> SearchPosition {
        let game = fen::game_from_fen(fen::START_FEN).unwrap();
        fen::search_position(&game)
    }

    #[test]
    fn test_perft_startpos_matches_references() {
        let pos = startpos();
        // Depths 1-3 only — fast enough for the default test run.
        for depth in 1..=3u32 {
            assert_eq!(
                perft(&pos, depth),
                STARTPOS_REFERENCE[(depth - 1) as usize],
                "perft({depth}) mismatch"
            );
        }
    }

    #[test]
    fn test_perft_divide_sums_to_perft() {
        let pos = startpos();
        let entries = perft_divide(&pos, 3);
        assert_eq!(entries.len(), 20);
        let total: u64 = entries.iter().map(|(_, n)| n).sum();
        assert_eq!(total, perft(&pos, 3));
    }

    #[test]
    fn test_perft_parallel_matches_serial() {
        let pos = startpos();
        for threads in [1usize, 2, 4, 64] {
            assert_eq!(
                perft_parallel(&pos, 4, threads),
                STARTPOS_REFERENCE[3],
                "parallel perft with {threads} threads must match the reference"
            );
        }
        // Depth 1 short-circuits to the serial path but must still be right.
        assert_eq!(perft_parallel(&pos, 1, 8), STARTPOS_REFERENCE[0]);
    }

    #[test]
    fn test_perft_kiwipete_depth_2() {
        let game = fen::game_from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        )
        .unwrap();
        let pos = fen::search_position(&game);
        assert_eq!(perft(&pos, 2), 2_039);
    }
}
