//! `checkai eval` — inspect what the engine actually thinks about a position.
//!
//! Where `analyze` reports the *search* verdict, this command exposes the
//! layer underneath it: the static evaluation, the ordered move list with a
//! shallow score per move, the opening-book entries for the position, and the
//! endgame tablebase verdict when one is available. It is the quickest way to
//! answer "why does the engine like this?" — and it doubles as a debugging
//! window into every knowledge source the engine can consult.

use clap::Args;
use colored::Colorize;

use super::board_renderer::{BoardHighlights, BoardRenderer, BoardTheme, CapturedMaterial};
use super::engine::EngineArgs;
use super::fen;
use super::panel::{TableRow, render_table};
use super::score::{eval_bar_line, format_score, humanize_count, white_pov};
use super::{CliCommand, CliContext, CliResult, cli_error};
use crate::eval;
use crate::search::{SearchLimits, score_to_mate_in};
use crate::types::Color;

/// Default per-move search depth used to rank the move list.
const DEFAULT_MOVE_DEPTH: i32 = 6;

/// How many moves the ranked list shows by default.
const DEFAULT_TOP_MOVES: usize = 8;

/// Arguments for `checkai eval`.
#[derive(Args, Debug)]
#[command(after_help = "\
Examples:\n\
  checkai eval                            Evaluate the starting position\n\
  checkai eval --fen \"<FEN>\"              Evaluate a specific position\n\
  checkai eval --fen \"<FEN>\" --top 20     Rank the twenty best moves\n\
  checkai eval --fen \"<FEN>\" --depth 10   Rank moves with a deeper search\n\
  checkai eval --book book.bin            Also show the opening-book entries\n\
  checkai eval --tablebase tb/            Also show the tablebase verdict")]
pub struct EvalArgs {
    /// Position to evaluate (defaults to the starting position).
    #[arg(long)]
    pub fen: Option<String>,

    /// Search depth used to score each candidate move.
    #[arg(long, default_value_t = DEFAULT_MOVE_DEPTH)]
    pub depth: i32,

    /// How many ranked moves to list (0 = all legal moves).
    #[arg(long, default_value_t = DEFAULT_TOP_MOVES)]
    pub top: usize,

    /// Render the board from Black's perspective.
    #[arg(long)]
    pub flip: bool,

    /// Use ASCII piece letters instead of Unicode glyphs.
    #[arg(long)]
    pub ascii: bool,

    /// Board colour palette.
    #[arg(long, value_enum, default_value_t = BoardTheme::default())]
    pub board: BoardTheme,

    #[command(flatten)]
    pub engine: EngineArgs,
}

impl CliCommand for EvalArgs {
    fn run(self, ctx: &CliContext) -> CliResult {
        let fen_str = self.fen.as_deref().unwrap_or(fen::START_FEN);
        let game = fen::game_from_fen(fen_str)
            .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?;
        let position = fen::search_position(&game);

        let renderer =
            BoardRenderer::for_theme(ctx.theme.colors, self.ascii, self.flip, self.board);
        println!();
        print!(
            "{}",
            renderer.render(&game.board, &BoardHighlights::for_game(&game))
        );
        println!("  {}", fen::game_to_fen(&game).dimmed());
        println!();

        // 1. Static evaluation — no search involved.
        let static_cp = eval::evaluate(&game.board, game.turn);
        let static_white = white_pov(static_cp, game.turn);
        println!("{}", t!("eval.static_header").to_string().yellow().bold());
        println!("  {}", eval_bar_line(static_white));
        let captured = CapturedMaterial::for_board(&game.board);
        println!(
            "  {}",
            t!(
                "eval.material_line",
                balance = format!("{:+}", captured.balance),
                white = captured.glyphs(Color::White),
                black = captured.glyphs(Color::Black)
            )
            .to_string()
            .dimmed()
        );
        println!(
            "  {}",
            t!(
                "eval.phase_line",
                pieces = eval::piece_count(&game.board),
                material = eval::material_score(&game.board)
            )
            .to_string()
            .dimmed()
        );
        println!();

        // 2. Ranked moves — a shallow search per candidate.
        let legal = game.legal_moves();
        if legal.is_empty() {
            println!("{}", t!("eval.no_legal_moves").to_string().red());
            return Ok(());
        }

        let mut config = self.engine.build_config(super::engine::DEFAULT_HASH_MB);
        // Ranking *is* a MultiPV search — ask for as many lines as requested.
        config.multi_pv = if self.top == 0 {
            legal.len()
        } else {
            self.top.min(legal.len())
        }
        .clamp(1, crate::search::MAX_MULTI_PV);
        super::engine::print_engine_banner(&ctx.theme, &config);

        let mut engine = crate::search::SearchEngine::with_config(config);
        engine.set_game_history(&fen::history_hashes(&game));
        let limits = SearchLimits {
            max_depth: self.depth.clamp(1, crate::search::MAX_DEPTH),
            max_nodes: self.engine.nodes,
            ..SearchLimits::default()
        };
        let pb = super::progress::spinner(&ctx.theme, t!("eval.ranking").to_string());
        let result = engine.search_limited(&position, &limits, None);
        pb.finish_and_clear();

        println!(
            "{}",
            t!("eval.moves_header", depth = result.depth)
                .to_string()
                .yellow()
                .bold()
        );
        println!(
            "  {:>3}  {:<8} {:>8}  {}",
            "#",
            t!("analyze.col_move"),
            t!("analyze.col_eval"),
            t!("analyze.col_pv")
        );
        for line in &result.pv_lines {
            let score = match line.mate_in {
                Some(mate) => format!("#{mate}"),
                None => format_score(line.score),
            };
            let pv: Vec<String> = line.moves.iter().skip(1).map(|m| m.to_string()).collect();
            let mv = line
                .best_move()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "—".to_string());
            println!(
                "  {:>3}  {:<8} {:>8}  {}",
                line.rank,
                if line.rank == 1 {
                    mv.green().bold().to_string()
                } else {
                    mv
                },
                score,
                pv.join(" ").dimmed()
            );
        }
        println!();

        // 3. Search statistics.
        let rows = vec![
            TableRow::new(
                t!("eval.stat_nodes").to_string(),
                None,
                format!(
                    "{} ({} n/s)",
                    humanize_count(result.stats.nodes),
                    humanize_count(result.nps())
                ),
            ),
            TableRow::new(
                t!("eval.stat_depth").to_string(),
                None,
                format!("{} / {}", result.depth, result.seldepth),
            ),
            TableRow::new(
                t!("eval.stat_tt").to_string(),
                None,
                format!(
                    "{} hits, {} cutoffs, {}‰ full",
                    humanize_count(result.stats.tt_hits),
                    humanize_count(result.stats.tt_cutoffs),
                    result.hashfull
                ),
            ),
            TableRow::new(
                t!("eval.stat_pruning").to_string(),
                None,
                format!(
                    "{} null, {} LMR, {} singular",
                    humanize_count(result.stats.null_cutoffs),
                    humanize_count(result.stats.lmr_searches),
                    humanize_count(result.stats.singular_extensions)
                ),
            ),
        ];
        println!("{}", t!("eval.stats_header").to_string().yellow().bold());
        print!("{}", render_table(&rows, 2));
        println!();

        // 4. Opening book entries for this exact position.
        if let Some(book) = &engine.config().book {
            let entries = book.lookup(&game.board, game.turn, &game.castling, game.en_passant);
            println!("{}", t!("eval.book_header").to_string().yellow().bold());
            if entries.is_empty() {
                println!("  {}", t!("eval.book_miss").to_string().dimmed());
            } else {
                let total: u32 = entries.iter().map(|e| u32::from(e.weight)).sum();
                for entry in &entries {
                    let share = if total > 0 {
                        f64::from(entry.weight) * 100.0 / f64::from(total)
                    } else {
                        0.0
                    };
                    println!(
                        "  {:<8} {:>6.1}%  {}",
                        entry.chess_move.to_string().green(),
                        share,
                        "▇".repeat(((share / 5.0).round() as usize).max(1)).dimmed()
                    );
                }
            }
            println!();
        }

        // 5. Tablebase verdict.
        if let Some(info) = &result.tablebase {
            println!(
                "{}",
                t!("eval.tablebase_header").to_string().yellow().bold()
            );
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
            );
            println!();
        }

        Ok(())
    }
}

/// Formats the mate distance of a score for display, if it is a mate.
pub fn mate_label(score_cp: i32) -> Option<String> {
    score_to_mate_in(score_cp).map(|mate| format!("#{mate}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mate_label() {
        assert_eq!(mate_label(0), None);
        assert_eq!(mate_label(crate::eval::MATE_SCORE - 3), Some("#2".into()));
    }

    #[test]
    fn test_default_args_use_start_position() {
        let args = EvalArgs {
            fen: None,
            depth: 2,
            top: 3,
            flip: false,
            ascii: true,
            board: BoardTheme::Ascii,
            engine: EngineArgs::default(),
        };
        assert!(args.fen.is_none());
        let game = fen::game_from_fen(fen::START_FEN).unwrap();
        assert_eq!(game.legal_moves().len(), 20);
    }
}
