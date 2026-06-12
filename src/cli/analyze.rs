//! `checkai analyze` — engine analysis of a position or a whole game.
//!
//! Two modes:
//!
//! 1. **Position dive** (`--fen <FEN>`): iterative-deepening table with
//!    one row per depth (eval, mate distance, nodes, PV) and a final
//!    verdict line with the best move.
//! 2. **Game analysis** (`--moves "e2e4 e7e5 ..."`): replays a list of
//!    long-algebraic moves (optionally starting from `--fen`), searches
//!    each position before and after the played move, and annotates each
//!    move with its centipawn loss, a `!`/`?!`/`?`/`??` marker and the
//!    better alternative when the played move loses more than 50 cp.
//!
//! There is no PGN importer in CheckAI yet, so game input is a plain
//! space-separated move list in coordinate notation (`e2e4`, `e7e8q`).

use clap::Args;
use colored::Colorize;

use super::fen;
use super::progress::bar;
use super::score::format_score;
use super::{CliCommand, CliContext, CliResult, cli_error};
use crate::analysis::MoveQuality;
use crate::search::{MAX_DEPTH, SearchEngine, SearchLimits};
use crate::terminal::parse_move_input;
use crate::types::Color;

/// Default per-move thinking time for game analysis (ms).
const DEFAULT_MOVE_ANALYSIS_MS: u64 = 1_000;
/// Default thinking time for a single-position dive (ms).
const DEFAULT_POSITION_ANALYSIS_MS: u64 = 5_000;
/// Centipawn loss above which the better alternative is displayed.
const SHOW_ALTERNATIVE_THRESHOLD: i32 = 50;
/// Transposition table size for analysis engines (MB).
const ANALYSIS_TT_MB: usize = 128;

/// Arguments for `checkai analyze`.
#[derive(Args, Debug)]
#[command(after_help = "\
Examples:\n\
  checkai analyze --fen \"r1bqkbnr/...\"          Deep-dive one position\n\
  checkai analyze --fen \"<FEN>\" --depth 16      Fixed-depth dive\n\
  checkai analyze --moves \"e2e4 e7e5 g1f3\"      Annotate a game\n\
  checkai analyze --fen \"<FEN>\" --moves \"...\"   Game from a custom start\n\
  checkai analyze --moves \"...\" --movetime 250  Faster, shallower verdicts\n\
\n\
Note: PGN import is not supported yet — pass moves in coordinate\n\
notation (e2e4, e7e8q), space-separated.")]
pub struct AnalyzeArgs {
    /// Position to analyze (or the starting position for --moves).
    #[arg(long)]
    pub fen: Option<String>,

    /// Space-separated long-algebraic move list to annotate.
    #[arg(long)]
    pub moves: Option<String>,

    /// Fixed search depth (replaces the default time budget).
    #[arg(long)]
    pub depth: Option<i32>,

    /// Thinking time in milliseconds (per move for --moves).
    #[arg(long)]
    pub movetime: Option<u64>,
}

impl CliCommand for AnalyzeArgs {
    fn run(self, ctx: &CliContext) -> CliResult {
        match (&self.moves, &self.fen) {
            (Some(moves), _) => analyze_game(ctx, &self, moves.clone()),
            (None, Some(_)) => analyze_position(&self),
            (None, None) => Err(cli_error(t!("analyze.need_input").to_string())),
        }
    }
}

/// Builds the search limits for this invocation.
fn limits_for(args: &AnalyzeArgs, default_ms: u64) -> SearchLimits {
    match (args.depth, args.movetime) {
        (Some(depth), None) => SearchLimits::depth(depth.clamp(1, MAX_DEPTH)),
        (depth, movetime) => SearchLimits {
            max_depth: depth.unwrap_or(MAX_DEPTH).clamp(1, MAX_DEPTH),
            move_time_ms: Some(movetime.unwrap_or(default_ms)),
            max_nodes: None,
        },
    }
}

/// Maps a centipawn loss to a chess annotation marker.
///
/// `!` best/near-best, empty for solid moves, `?!` inaccuracy,
/// `?` mistake, `??` blunder.
pub fn annotation_marker(cp_loss: i32) -> &'static str {
    match cp_loss {
        i32::MIN..=10 => "!",
        11..=50 => "",
        51..=100 => "?!",
        101..=300 => "?",
        _ => "??",
    }
}

/// Returns the localized description of a move-quality class.
fn quality_label(quality: MoveQuality) -> String {
    match quality {
        MoveQuality::Best => t!("analysis.quality.best").to_string(),
        MoveQuality::Excellent => t!("analysis.quality.excellent").to_string(),
        MoveQuality::Good => t!("analysis.quality.good").to_string(),
        MoveQuality::Inaccuracy => t!("analysis.quality.inaccuracy").to_string(),
        MoveQuality::Mistake => t!("analysis.quality.mistake").to_string(),
        MoveQuality::Blunder => t!("analysis.quality.blunder").to_string(),
        MoveQuality::Book => t!("analysis.quality.book").to_string(),
    }
}

/// Single-position deep dive: prints one table row per completed depth
/// and a final verdict.
fn analyze_position(args: &AnalyzeArgs) -> CliResult {
    let fen_str = args.fen.as_deref().unwrap_or(fen::START_FEN);
    let game = fen::game_from_fen(fen_str)
        .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?;
    let pos = fen::search_position(&game);
    let limits = limits_for(args, DEFAULT_POSITION_ANALYSIS_MS);

    println!();
    println!(
        "{}",
        t!("analyze.position_header").to_string().yellow().bold()
    );
    println!("  {}", fen::game_to_fen(&game).dimmed());
    println!();
    println!(
        "  {:>5}  {:>7}  {:>6}  {:>10}  {}",
        t!("analyze.col_depth"),
        t!("analyze.col_eval"),
        t!("analyze.col_mate"),
        t!("analyze.col_nodes"),
        t!("analyze.col_pv")
    );

    let mut engine = SearchEngine::new(ANALYSIS_TT_MB);
    let mut on_iteration = |info: &crate::search::IterationInfo| {
        // The mate distance lives in its own column, so the eval column shows
        // a dash for mates instead of repeating `#N`.
        let (eval_cell, mate) = match info.mate_in {
            Some(m) => ("—".to_string(), format!("#{m}")),
            None => (format_score(info.score_cp), String::new()),
        };
        let pv: Vec<&str> = info.pv.iter().take(8).map(String::as_str).collect();
        println!(
            "  {:>5}  {:>7}  {:>6}  {:>10}  {}",
            info.depth,
            eval_cell,
            mate,
            info.nodes,
            pv.join(" ").cyan()
        );
    };
    let result = engine.search_limited(&pos, &limits, Some(&mut on_iteration));

    println!();
    match result.best_move {
        Some(mv) => println!(
            "{}",
            t!(
                "analyze.verdict",
                mv = mv.to_string().green().bold(),
                score = format_score(result.score).bold(),
                depth = result.depth
            )
        ),
        None => println!("{}", t!("analyze.no_best_move")),
    }
    println!();
    Ok(())
}

/// Per-move annotation produced by the game analyzer.
struct MoveReport {
    number: u32,
    side: Color,
    played: String,
    eval_after: i32,
    cp_loss: i32,
    quality: MoveQuality,
    best_alternative: Option<String>,
}

/// Replays and annotates a list of coordinate moves.
fn analyze_game(ctx: &CliContext, args: &AnalyzeArgs, moves: String) -> CliResult {
    let start_fen = args.fen.as_deref().unwrap_or(fen::START_FEN);
    let mut game = fen::game_from_fen(start_fen)
        .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?;

    let tokens: Vec<&str> = moves.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(cli_error(t!("analyze.no_moves").to_string()));
    }

    let limits = limits_for(args, DEFAULT_MOVE_ANALYSIS_MS);
    let mut engine = SearchEngine::new(ANALYSIS_TT_MB);
    let pb = bar(
        &ctx.theme,
        tokens.len() as u64,
        t!("analyze.progress_label").to_string(),
    );

    let mut reports: Vec<MoveReport> = Vec::with_capacity(tokens.len());
    for token in &tokens {
        let move_json = parse_move_input(token)
            .ok_or_else(|| cli_error(t!("analyze.bad_move", mv = *token).to_string()))?;

        let side = game.turn;
        let number = game.fullmove_number;
        let pos = fen::search_position(&game);

        // Engine's best move and evaluation before the played move.
        engine.set_game_history(&fen::history_hashes(&game));
        let best_result = engine.search_limited(&pos, &limits, None);

        // Apply the played move (validates legality).
        game.make_move(&move_json).map_err(|e| {
            cli_error(t!("analyze.illegal_move", mv = *token, error = e).to_string())
        })?;

        // Evaluation after the played move, from the mover's perspective.
        let played_pos = fen::search_position(&game);
        engine.set_game_history(&fen::history_hashes(&game));
        let played_result = engine.search_limited(&played_pos, &limits, None);
        let eval_after = -played_result.score;

        let cp_loss = (best_result.score - eval_after).max(0);
        let best_alternative = best_result.best_move.and_then(|mv| {
            let uci = mv.to_string();
            if cp_loss > SHOW_ALTERNATIVE_THRESHOLD && uci != *token {
                Some(uci)
            } else {
                None
            }
        });

        reports.push(MoveReport {
            number,
            side,
            played: (*token).to_string(),
            eval_after,
            cp_loss,
            quality: MoveQuality::from_cp_loss(cp_loss),
            best_alternative,
        });
        pb.inc(1);

        if game.is_over() {
            break;
        }
    }
    pb.finish_and_clear();

    print_game_report(&reports);
    Ok(())
}

/// Prints the annotated move table plus a quality summary.
fn print_game_report(reports: &[MoveReport]) {
    println!();
    println!("{}", t!("analyze.game_header").to_string().yellow().bold());
    println!();
    println!(
        "  {:>4} {:<6} {:<8} {:>7} {:>6}  {:<12} {}",
        "#",
        t!("analyze.col_side"),
        t!("analyze.col_move"),
        t!("analyze.col_eval"),
        t!("analyze.col_loss"),
        t!("analyze.col_quality"),
        t!("analyze.col_better")
    );

    for report in reports {
        let marker = annotation_marker(report.cp_loss);
        let move_label = format!("{}{}", report.played, marker);
        let styled_move = match marker {
            "??" => move_label.red().bold().to_string(),
            "?" => move_label.red().to_string(),
            "?!" => move_label.yellow().to_string(),
            "!" => move_label.green().to_string(),
            _ => move_label,
        };
        let alternative = report
            .best_alternative
            .as_ref()
            .map(|alt| t!("analyze.better_was", mv = alt).to_string())
            .unwrap_or_default();
        println!(
            "  {:>4} {:<6} {:<8} {:>7} {:>6}  {:<12} {}",
            report.number,
            super::play::color_name(report.side),
            styled_move,
            format_score(super::score::white_pov(report.eval_after, report.side)),
            super::score::format_cp_loss(report.cp_loss),
            quality_label(report.quality),
            alternative.dimmed()
        );
    }

    // Summary: count per quality bucket.
    println!();
    let count = |q: MoveQuality| reports.iter().filter(|r| r.quality == q).count();
    println!(
        "{}",
        t!(
            "analyze.summary",
            total = reports.len(),
            best = count(MoveQuality::Best) + count(MoveQuality::Excellent),
            inaccuracies = count(MoveQuality::Inaccuracy),
            mistakes = count(MoveQuality::Mistake),
            blunders = count(MoveQuality::Blunder)
        )
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_marker_thresholds() {
        assert_eq!(annotation_marker(0), "!");
        assert_eq!(annotation_marker(10), "!");
        assert_eq!(annotation_marker(11), "");
        assert_eq!(annotation_marker(50), "");
        assert_eq!(annotation_marker(51), "?!");
        assert_eq!(annotation_marker(100), "?!");
        assert_eq!(annotation_marker(101), "?");
        assert_eq!(annotation_marker(300), "?");
        assert_eq!(annotation_marker(301), "??");
        assert_eq!(annotation_marker(10_000), "??");
    }

    #[test]
    fn test_limits_depth_only() {
        let args = AnalyzeArgs {
            fen: None,
            moves: None,
            depth: Some(12),
            movetime: None,
        };
        let limits = limits_for(&args, 1000);
        assert_eq!(limits.max_depth, 12);
        assert_eq!(limits.move_time_ms, None);
    }

    #[test]
    fn test_limits_default_movetime() {
        let args = AnalyzeArgs {
            fen: None,
            moves: None,
            depth: None,
            movetime: None,
        };
        let limits = limits_for(&args, 1234);
        assert_eq!(limits.move_time_ms, Some(1234));
        assert_eq!(limits.max_depth, MAX_DEPTH);
    }
}
