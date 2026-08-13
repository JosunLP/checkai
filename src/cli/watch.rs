//! `checkai watch` — sit back and watch the engine play itself.
//!
//! Two independent engine instances (each with its own transposition
//! table, difficulty level and optional opening book) alternate moves.
//! The board, a move ticker and a live search panel repaint in place, and
//! every move is animated across the board, so the whole game plays out in
//! a single screen instead of scrolling past.
//!
//! Threefold/fifty-move draws are claimed automatically so games always
//! terminate; a configurable move cap guards against endless shuffling, and
//! `--adjudicate` ends hopeless positions early. The finished game can be
//! written straight out as PGN with `--pgn-out`.

use std::time::Instant;

use clap::Args;
use colored::Colorize;

use super::animate::{self, LiveRegion};
use super::board_renderer::{BoardHighlights, BoardRenderer, BoardTheme, CapturedMaterial};
use super::clock::{GameClock, TimeControl};
use super::engine::EngineArgs;
use super::fen;
use super::level::{DEFAULT_LEVEL, LevelSettings, MAX_LEVEL, MIN_LEVEL};
use super::panel::result_panel;
use super::pgn::{self, PgnMetadata};
use super::play::{claim_available_draw, color_name, side_label};
use super::progress::ThinkingView;
use super::score::{eval_bar_line, format_score, humanize_count, sparkline, white_pov};
use super::{CliCommand, CliContext, CliResult, cli_error};
use crate::game::Game;
use crate::search::{IterationInfo, MoveSource, SearchEngine};
use crate::types::Color;

/// Arguments for `checkai watch`.
#[derive(Args, Debug)]
#[command(after_help = "\
Examples:\n\
  checkai watch                          Level 5 vs level 5 showcase\n\
  checkai watch --level 8                Both sides at level 8\n\
  checkai watch --level-white 9 --level-black 3   An uneven match\n\
  checkai watch --movetime 200 --delay 0          Fast-forward game\n\
  checkai watch --time 1+0               Play the game on a bullet clock\n\
  checkai watch --fen \"<FEN>\"            Start from a custom position\n\
  checkai watch --adjudicate 900         Stop once one side is +9\n\
  checkai watch --pgn-out game.pgn       Save the finished game as PGN\n\
  checkai watch --max-moves 60           Stop after 60 full moves")]
pub struct WatchArgs {
    /// Difficulty level for both engines (overrides the per-side flags).
    #[arg(long, value_parser = clap::value_parser!(u8).range(MIN_LEVEL as i64..=MAX_LEVEL as i64))]
    pub level: Option<u8>,

    /// Difficulty level for the white engine.
    #[arg(long, default_value_t = DEFAULT_LEVEL,
          value_parser = clap::value_parser!(u8).range(MIN_LEVEL as i64..=MAX_LEVEL as i64))]
    pub level_white: u8,

    /// Difficulty level for the black engine.
    #[arg(long, default_value_t = DEFAULT_LEVEL,
          value_parser = clap::value_parser!(u8).range(MIN_LEVEL as i64..=MAX_LEVEL as i64))]
    pub level_black: u8,

    /// Thinking time per move in milliseconds (overrides the ladder).
    #[arg(long)]
    pub movetime: Option<u64>,

    /// Time control for the whole game, e.g. `3+2` (overrides --movetime).
    #[arg(long, value_name = "SPEC")]
    pub time: Option<String>,

    /// Start from a custom position (4–6 field FEN string).
    #[arg(long)]
    pub fen: Option<String>,

    /// Safety cap: stop after this many full moves.
    #[arg(long, default_value_t = 200)]
    pub max_moves: u32,

    /// Adjudicate a win once the evaluation exceeds this many centipawns
    /// for four consecutive moves (0 = never adjudicate).
    #[arg(long, default_value_t = 0)]
    pub adjudicate: i32,

    /// Pause between moves in milliseconds (0 = as fast as possible).
    #[arg(long, default_value_t = 800)]
    pub delay: u64,

    /// Use ASCII piece letters instead of Unicode glyphs.
    #[arg(long)]
    pub ascii: bool,

    /// Board colour palette.
    #[arg(long, value_enum, default_value_t = BoardTheme::default())]
    pub board: BoardTheme,

    /// Write the finished game to a PGN file.
    #[arg(long, value_name = "FILE")]
    pub pgn_out: Option<String>,

    /// Print only the move ticker (no board, no animation).
    #[arg(long)]
    pub quiet: bool,

    #[command(flatten)]
    pub engine: EngineArgs,
}

/// How many consecutive decisive evaluations trigger adjudication.
const ADJUDICATION_STREAK: u32 = 4;

impl CliCommand for WatchArgs {
    fn run(self, ctx: &CliContext) -> CliResult {
        let white_level = self.level.unwrap_or(self.level_white);
        let black_level = self.level.unwrap_or(self.level_black);
        let white_settings = LevelSettings::for_level(white_level, self.movetime, None);
        let black_settings = LevelSettings::for_level(black_level, self.movetime, None);

        let mut clock = match &self.time {
            Some(spec) => Some(GameClock::new(TimeControl::parse(spec).map_err(|e| {
                cli_error(t!("play.bad_time_control", error = e).to_string())
            })?)),
            None => None,
        };

        let mut game = match &self.fen {
            Some(fen_str) => fen::game_from_fen(fen_str)
                .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?,
            None => Game::new(),
        };

        println!();
        println!(
            "{}",
            t!(
                "watch.intro",
                white = white_settings.level,
                black = black_settings.level
            )
            .to_string()
            .yellow()
            .bold()
        );

        let mut white_config = self.engine.build_config(white_settings.tt_size_mb);
        white_config.skill_level = white_settings.skill;
        let mut black_config = self.engine.build_config(black_settings.tt_size_mb);
        black_config.skill_level = black_settings.skill;
        super::engine::print_engine_banner(&ctx.theme, &white_config);
        println!();

        let mut white_engine = SearchEngine::with_config(white_config);
        let mut black_engine = SearchEngine::with_config(black_config);
        let renderer = BoardRenderer::for_theme(ctx.theme.colors, self.ascii, false, self.board);
        let started = Instant::now();
        let show_board = !self.quiet;
        let mut curve: Vec<i32> = Vec::new();
        let mut decisive_streak = 0u32;
        let mut adjudicated: Option<Color> = None;

        if show_board {
            print!(
                "{}",
                renderer.render(&game.board, &BoardHighlights::default())
            );
            println!();
        }

        while !game.is_over() && game.fullmove_number <= self.max_moves {
            let side = game.turn;
            let (engine, settings) = match side {
                Color::White => (&mut white_engine, &white_settings),
                Color::Black => (&mut black_engine, &black_settings),
            };

            let mut limits = settings.limits.clone();
            limits.max_nodes = self.engine.nodes;
            if let Some(clock) = clock.as_mut() {
                clock.start(side);
                limits.move_time_ms = Some(clock.budget_ms(side));
            }

            let label = t!(
                "watch.thinking",
                color = color_name(side),
                level = settings.level
            )
            .to_string();

            engine.reset_abort();
            let position = fen::search_position(&game);
            engine.set_game_history(&fen::history_hashes(&game));

            let board_block = if show_board {
                renderer.render(&game.board, &BoardHighlights::for_game(&game))
            } else {
                String::new()
            };
            let mut region = LiveRegion::new(&ctx.theme);
            let mut view = ThinkingView::new(label, side);
            if show_board {
                region.frame(&animate::compose("", &board_block, &view.render()));
            }

            let result = {
                let region = &mut region;
                let view = &mut view;
                let board_block = board_block.as_str();
                let mut on_iteration = |info: &IterationInfo| {
                    if !show_board {
                        return;
                    }
                    view.tick += 1;
                    view.info = Some(info.clone());
                    region.frame(&animate::compose("", board_block, &view.render()));
                };
                engine.search_limited(&position, &limits, Some(&mut on_iteration))
            };
            region.clear();

            if let Some(clock) = clock.as_mut() {
                clock.stop();
                if clock.is_flagged() {
                    println!(
                        "{}",
                        t!("play.flag_fall", color = color_name(side))
                            .to_string()
                            .red()
                            .bold()
                    );
                    break;
                }
            }

            let Some(best) = result.best_move else {
                // No move available — should be unreachable for a live game.
                break;
            };

            // Ticker line: move number, side, move, eval, depth, nodes.
            let san = pgn::move_to_san(&game, &best);
            println!(
                "{}",
                t!(
                    "watch.move_ticker",
                    num = game.fullmove_number,
                    color = side_label(side),
                    mv = san.green().bold(),
                    score = if result.source == MoveSource::Book {
                        t!("watch.book_tag").to_string()
                    } else {
                        format_score(result.score)
                    },
                    depth = result.depth,
                    nodes = humanize_count(result.stats.nodes)
                )
            );

            let before = game.board.clone();
            if let Err(e) = game.make_move(&best.to_json()) {
                println!(
                    "{}: {}",
                    t!("terminal.error_label").to_string().red().bold(),
                    e
                );
                break;
            }

            if show_board {
                let mut region = LiveRegion::new(&ctx.theme);
                animate::animate_move(
                    &ctx.theme,
                    &mut region,
                    &renderer,
                    &before,
                    best.from,
                    best.to,
                    "",
                    "",
                );
                region.clear();
                print!(
                    "{}",
                    renderer.render(&game.board, &BoardHighlights::for_game(&game))
                );
                let captured = CapturedMaterial::for_board(&game.board);
                println!();
                println!("  {}", eval_bar_line(white_pov(result.score, side)));
                if !captured.by_white.is_empty() || !captured.by_black.is_empty() {
                    println!(
                        "  {} {}   {} {}   {}",
                        "W".white().bold(),
                        captured.glyphs(Color::White).dimmed(),
                        "B".blue().bold(),
                        captured.glyphs(Color::Black).dimmed(),
                        format!("{:+}", captured.balance).cyan()
                    );
                }
                println!();
            }

            // Evaluation curve, from White's perspective.
            let white_score = white_pov(result.score, side);
            curve.push(white_score);

            // Optional adjudication of hopeless positions.
            if self.adjudicate > 0 {
                if white_score.abs() >= self.adjudicate {
                    decisive_streak += 1;
                    if decisive_streak >= ADJUDICATION_STREAK {
                        adjudicated = Some(if white_score > 0 {
                            Color::White
                        } else {
                            Color::Black
                        });
                        break;
                    }
                } else {
                    decisive_streak = 0;
                }
            }

            // End shuffled games: claim any available draw automatically.
            if !game.is_over() {
                match claim_available_draw(&mut game) {
                    Ok(true) => break,
                    Ok(false) => {}
                    Err(e) => eprintln!("error: draw claim failed: {e}"),
                }
            }

            // Pacing between moves — interactive terminals only.
            if ctx.theme.interactive && self.delay > 0 && !game.is_over() {
                std::thread::sleep(std::time::Duration::from_millis(self.delay));
            }
        }

        if let Some(winner) = adjudicated {
            println!(
                "{}",
                t!("watch.adjudicated", color = color_name(winner))
                    .to_string()
                    .yellow()
                    .bold()
            );
        } else if game.is_over() {
            println!("{}", result_panel(&game, started.elapsed()));
        } else {
            println!(
                "{}",
                t!("watch.move_cap_reached", cap = self.max_moves)
                    .to_string()
                    .yellow()
                    .bold()
            );
        }

        if curve.len() > 1 {
            println!();
            println!(
                "  {} {}",
                t!("watch.eval_curve").to_string().dimmed(),
                sparkline(&curve).cyan()
            );
        }

        if let Some(path) = &self.pgn_out {
            let meta = PgnMetadata {
                event: "CheckAI Engine Match".to_string(),
                white: format!("CheckAI level {}", white_settings.level),
                black: format!("CheckAI level {}", black_settings.level),
                ..PgnMetadata::default()
            };
            match std::fs::write(path, pgn::write_pgn(&game, &meta)) {
                Ok(()) => println!(
                    "{}",
                    t!("play.pgn_saved", path = path.clone())
                        .to_string()
                        .green()
                ),
                Err(e) => eprintln!("{}", t!("play.pgn_write_failed", error = e)),
            }
        }

        println!();
        Ok(())
    }
}
