//! `checkai analyze` — engine analysis of a position or a whole game.
//!
//! Three modes:
//!
//! 1. **Position dive** (`--fen <FEN>`): iterative-deepening table with
//!    one row per depth (eval, mate distance, nodes, PV) and a final
//!    verdict. With `--multipv N` the best `N` lines are reported instead
//!    of just the top one.
//! 2. **Move list** (`--moves "e2e4 e7e5 …"`): replays coordinate or SAN
//!    moves (optionally from `--fen`) and annotates each one.
//! 3. **PGN file** (`--pgn game.pgn`): the same annotation pass over an
//!    imported game, including its `FEN`/`SetUp` start position.
//!
//! Every annotated move gets its centipawn loss, a `!`/`?!`/`?`/`??`
//! marker, and the better alternative when the played move loses more than
//! 50 cp. The report ends with per-side accuracy and an evaluation curve.

use std::path::PathBuf;

use clap::Args;
use colored::Colorize;

use super::engine::EngineArgs;
use super::fen;
use super::pgn;
use super::progress::bar;
use super::score::{accuracy_from_cp_loss, format_score, sparkline};
use super::{CliCommand, CliContext, CliResult, cli_error};
use crate::analysis::MoveQuality;
use crate::game::Game;
use crate::search::{MAX_DEPTH, SearchEngine, SearchLimits};
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
  checkai analyze --fen \"<FEN>\" --multipv 4     Show the four best lines\n\
  checkai analyze --moves \"e2e4 e5 Nf3\"         Annotate a move list\n\
  checkai analyze --pgn game.pgn                Annotate a PGN game\n\
  checkai analyze --pgn game.pgn --movetime 250 Faster, shallower verdicts\n\
  checkai analyze --pgn game.pgn --threads 4    Use four search threads\n\
\n\
Moves may be given in coordinate (e2e4, e7e8q) or standard algebraic\n\
(e4, Nf3, exd5, O-O) notation, space-separated.")]
pub struct AnalyzeArgs {
    /// Position to analyze (or the starting position for --moves).
    #[arg(long)]
    pub fen: Option<String>,

    /// Space-separated move list to annotate (coordinate or SAN).
    #[arg(long)]
    pub moves: Option<String>,

    /// PGN file to import and annotate.
    #[arg(long, value_name = "FILE")]
    pub pgn: Option<PathBuf>,

    /// Fixed search depth (replaces the default time budget).
    #[arg(long)]
    pub depth: Option<i32>,

    /// Thinking time in milliseconds (per move for game analysis).
    #[arg(long)]
    pub movetime: Option<u64>,

    /// Report this many principal variations in position mode.
    #[arg(long, value_name = "N")]
    pub multipv: Option<usize>,

    #[command(flatten)]
    pub engine: EngineArgs,
}

impl CliCommand for AnalyzeArgs {
    fn run(self, ctx: &CliContext) -> CliResult {
        match (&self.pgn, &self.moves, &self.fen) {
            (Some(path), _, _) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| cli_error(t!("play.pgn_read_failed", error = e).to_string()))?;
                let games = pgn::parse_pgn(&text).map_err(cli_error)?;
                let first = games.first().ok_or_else(|| cli_error("empty PGN"))?;
                let start = match first.tag("FEN") {
                    Some(fen_str) => fen::game_from_fen(fen_str)
                        .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?,
                    None => Game::new(),
                };
                analyze_game(ctx, &self, start, first.moves.clone())
            }
            (None, Some(moves), _) => {
                let start_fen = self.fen.as_deref().unwrap_or(fen::START_FEN);
                let start = fen::game_from_fen(start_fen)
                    .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?;
                let tokens = moves.split_whitespace().map(str::to_string).collect();
                analyze_game(ctx, &self, start, tokens)
            }
            (None, None, Some(_)) => analyze_position(ctx, &self),
            (None, None, None) => Err(cli_error(t!("analyze.need_input").to_string())),
        }
    }
}

/// Builds the search limits for this invocation.
fn limits_for(args: &AnalyzeArgs, default_ms: u64) -> SearchLimits {
    match (args.depth, args.movetime) {
        (Some(depth), None) => SearchLimits {
            max_depth: depth.clamp(1, MAX_DEPTH),
            max_nodes: args.engine.nodes,
            ..SearchLimits::default()
        },
        (depth, movetime) => SearchLimits {
            max_depth: depth.unwrap_or(MAX_DEPTH).clamp(1, MAX_DEPTH),
            move_time_ms: Some(movetime.unwrap_or(default_ms)),
            max_nodes: args.engine.nodes,
            ..SearchLimits::default()
        },
    }
}

/// Builds the analysis engine, honouring `--multipv` from either flag.
fn build_engine(args: &AnalyzeArgs, multi_pv: usize) -> SearchEngine {
    let mut config = args.engine.build_config(ANALYSIS_TT_MB);
    config.multi_pv = args
        .multipv
        .or(Some(multi_pv))
        .unwrap_or(1)
        .clamp(1, crate::search::MAX_MULTI_PV);
    // Analysis must be reproducible and exhaustive: never let the book
    // short-circuit a position the user explicitly asked about.
    config.use_book = false;
    SearchEngine::with_config(config)
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
/// and a final verdict (or the MultiPV ranking).
fn analyze_position(ctx: &CliContext, args: &AnalyzeArgs) -> CliResult {
    let fen_str = args.fen.as_deref().unwrap_or(fen::START_FEN);
    let game = fen::game_from_fen(fen_str)
        .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?;
    let position = fen::search_position(&game);
    let limits = limits_for(args, DEFAULT_POSITION_ANALYSIS_MS);

    println!();
    println!(
        "{}",
        t!("analyze.position_header").to_string().yellow().bold()
    );
    println!("  {}", fen::game_to_fen(&game).dimmed());
    let mut engine = build_engine(args, 1);
    super::engine::print_engine_banner(&ctx.theme, engine.config());
    engine.set_game_history(&fen::history_hashes(&game));
    let multi_pv = engine.config().multi_pv;
    println!();
    println!(
        "  {:>5}  {:>7}  {:>6}  {:>10}  {}",
        t!("analyze.col_depth"),
        t!("analyze.col_eval"),
        t!("analyze.col_mate"),
        t!("analyze.col_nodes"),
        t!("analyze.col_pv")
    );

    let mut on_iteration = |info: &crate::search::IterationInfo| {
        // Only the primary line drives the depth table; extra MultiPV lines
        // are reported once at the end so the table stays readable.
        if info.multipv != 1 {
            return;
        }
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
    let result = engine.search_limited(&position, &limits, Some(&mut on_iteration));

    println!();
    if multi_pv > 1 && result.pv_lines.len() > 1 {
        println!("{}", t!("analyze.lines_header").to_string().yellow().bold());
        for line in &result.pv_lines {
            let score = match line.mate_in {
                Some(mate) => format!("#{mate}"),
                None => format_score(line.score),
            };
            let moves: Vec<String> = line.moves.iter().map(|m| m.to_string()).collect();
            println!(
                "  {}. {:>8}  {}",
                line.rank,
                score.cyan().bold(),
                moves.join(" ").dimmed()
            );
        }
        println!();
    }

    match result.best_move {
        Some(mv) => println!(
            "{}",
            t!(
                "analyze.verdict",
                mv = pgn::move_to_san(&game, &mv).green().bold(),
                score = format_score(result.score).bold(),
                depth = result.depth
            )
        ),
        None => println!("{}", t!("analyze.no_best_move")),
    }
    if let Some(info) = &result.tablebase {
        println!(
            "  {}",
            t!(
                "eval.tablebase_line",
                config = info.configuration.clone(),
                wdl = info
                    .wdl
                    .map(|w| format!("{w:?}"))
                    .unwrap_or_else(|| "—".to_string()),
                dtz = info
                    .dtz
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "—".to_string()),
                source = info.source.clone()
            )
            .to_string()
            .dimmed()
        );
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

/// Replays and annotates a list of moves (coordinate or SAN).
fn analyze_game(
    ctx: &CliContext,
    args: &AnalyzeArgs,
    start: Game,
    tokens: Vec<String>,
) -> CliResult {
    if tokens.is_empty() {
        return Err(cli_error(t!("analyze.no_moves").to_string()));
    }

    let mut game = start;
    let limits = limits_for(args, DEFAULT_MOVE_ANALYSIS_MS);
    let mut engine = build_engine(args, 1);
    super::engine::print_engine_banner(&ctx.theme, engine.config());
    let pb = bar(
        &ctx.theme,
        tokens.len() as u64,
        t!("analyze.progress_label").to_string(),
    );

    let mut reports: Vec<MoveReport> = Vec::with_capacity(tokens.len());
    for (i, token) in tokens.iter().enumerate() {
        let move_json = pgn::san_to_move(&game, token)
            .ok_or_else(|| cli_error(t!("analyze.bad_move", mv = token.clone()).to_string()))?;

        let side = game.turn;
        let number = game.fullmove_number;
        let position = fen::search_position(&game);
        let san = crate::types::ChessMove::from_json(&move_json)
            .map(|mv| pgn::move_to_san(&game, &mv))
            .unwrap_or_else(|_| token.clone());

        // Engine's best move and evaluation before the played move.
        engine.set_game_history(&fen::history_hashes(&game));
        let best_result = engine.search_limited(&position, &limits, None);
        let best_san = best_result.best_move.map(|mv| pgn::move_to_san(&game, &mv));

        // Apply the played move (validates legality).
        game.make_move(&move_json).map_err(|e| {
            cli_error(t!("analyze.illegal_move", mv = token.clone(), error = e).to_string())
        })?;

        // Evaluation after the played move, from the mover's perspective.
        let played_pos = fen::search_position(&game);
        engine.set_game_history(&fen::history_hashes(&game));
        let played_result = engine.search_limited(&played_pos, &limits, None);
        let eval_after = -played_result.score;

        let cp_loss = (best_result.score - eval_after).max(0);
        let best_alternative = best_san
            .filter(|alternative| cp_loss > SHOW_ALTERNATIVE_THRESHOLD && *alternative != san);

        reports.push(MoveReport {
            number,
            side,
            played: san,
            eval_after,
            cp_loss,
            quality: MoveQuality::from_cp_loss(cp_loss),
            best_alternative,
        });
        pb.inc(1);

        if game.is_over() {
            if i + 1 < tokens.len() {
                pb.finish_and_clear();
                return Err(cli_error(
                    t!("analyze.moves_after_game_over", mv = tokens[i + 1].clone()).to_string(),
                ));
            }
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
            .map(|alt| t!("analyze.better_was", mv = alt.clone()).to_string())
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

    // Per-side accuracy, derived from the average centipawn loss.
    for side in [Color::White, Color::Black] {
        let losses: Vec<i32> = reports
            .iter()
            .filter(|r| r.side == side)
            .map(|r| r.cp_loss)
            .collect();
        if losses.is_empty() {
            continue;
        }
        let average = f64::from(losses.iter().sum::<i32>()) / losses.len() as f64;
        println!(
            "{}",
            t!(
                "analyze.accuracy",
                color = super::play::color_name(side),
                accuracy = format!("{:.1}", accuracy_from_cp_loss(average)),
                loss = format!("{average:.0}")
            )
        );
    }

    // Evaluation curve over the whole game, from White's perspective.
    let curve: Vec<i32> = reports
        .iter()
        .map(|r| super::score::white_pov(r.eval_after, r.side))
        .collect();
    if curve.len() > 1 {
        println!();
        println!(
            "  {} {}",
            t!("watch.eval_curve").to_string().dimmed(),
            sparkline(&curve).cyan()
        );
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_with(depth: Option<i32>, movetime: Option<u64>) -> AnalyzeArgs {
        AnalyzeArgs {
            fen: None,
            moves: None,
            pgn: None,
            depth,
            movetime,
            multipv: None,
            engine: EngineArgs::default(),
        }
    }

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
        let limits = limits_for(&args_with(Some(12), None), 1000);
        assert_eq!(limits.max_depth, 12);
        assert_eq!(limits.move_time_ms, None);
    }

    #[test]
    fn test_limits_default_movetime() {
        let limits = limits_for(&args_with(None, None), 1234);
        assert_eq!(limits.move_time_ms, Some(1234));
        assert_eq!(limits.max_depth, MAX_DEPTH);
    }

    #[test]
    fn test_analysis_engine_ignores_the_book() {
        let engine = build_engine(&args_with(None, None), 1);
        assert!(
            !engine.config().use_book,
            "analysis must search, not quote the book"
        );
    }

    #[test]
    fn test_multipv_flag_overrides_default() {
        let mut args = args_with(None, None);
        args.multipv = Some(5);
        assert_eq!(build_engine(&args, 1).config().multi_pv, 5);
    }
}
