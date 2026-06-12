//! `checkai watch` — sit back and watch the engine play itself.
//!
//! Two independent engine instances (each with its own transposition
//! table sized by its level) alternate moves. Every move is announced
//! on a ticker line, the board is re-rendered with from/to highlights,
//! and a running evaluation bar tracks the game. Threefold/fifty-move
//! draws are claimed automatically so games always terminate; a
//! configurable move cap guards against endless shuffling.

use std::time::Instant;

use clap::Args;
use colored::Colorize;

use super::board_renderer::{BoardHighlights, BoardRenderer};
use super::fen;
use super::level::{DEFAULT_LEVEL, LevelSettings, MAX_LEVEL, MIN_LEVEL};
use super::panel::result_panel;
use super::play::{claim_available_draw, color_name, side_label};
use super::progress::{iteration_message, spinner};
use super::score::{eval_bar_line, format_score, white_pov};
use super::{CliCommand, CliContext, CliResult};
use crate::game::Game;
use crate::search::{IterationInfo, SearchEngine};
use crate::types::Color;

/// Arguments for `checkai watch`.
#[derive(Args, Debug)]
#[command(after_help = "\
Examples:\n\
  checkai watch                          Level 5 vs level 5 showcase\n\
  checkai watch --level 8                Both sides at level 8\n\
  checkai watch --level-white 9 --level-black 3   An uneven match\n\
  checkai watch --movetime 200 --delay 0          Fast-forward game\n\
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

    /// Safety cap: stop after this many full moves.
    #[arg(long, default_value_t = 200)]
    pub max_moves: u32,

    /// Pause between moves in milliseconds (0 = as fast as possible).
    #[arg(long, default_value_t = 800)]
    pub delay: u64,

    /// Use ASCII piece letters instead of Unicode glyphs.
    #[arg(long)]
    pub ascii: bool,
}

impl CliCommand for WatchArgs {
    fn run(self, ctx: &CliContext) -> CliResult {
        let white_level = self.level.unwrap_or(self.level_white);
        let black_level = self.level.unwrap_or(self.level_black);
        let white_settings = LevelSettings::for_level(white_level, self.movetime, None);
        let black_settings = LevelSettings::for_level(black_level, self.movetime, None);

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
        println!();

        let mut white_engine = SearchEngine::new(white_settings.tt_size_mb);
        let mut black_engine = SearchEngine::new(black_settings.tt_size_mb);
        let renderer = BoardRenderer::new(self.ascii, false);
        let mut game = Game::new();
        let started = Instant::now();

        print!(
            "{}",
            renderer.render(&game.board, &BoardHighlights::default())
        );
        println!();

        while !game.is_over() && game.fullmove_number <= self.max_moves {
            let side = game.turn;
            let (engine, settings) = match side {
                Color::White => (&mut white_engine, &white_settings),
                Color::Black => (&mut black_engine, &black_settings),
            };

            let label = t!(
                "watch.thinking",
                color = color_name(side),
                level = settings.level
            )
            .to_string();
            let pb = spinner(&ctx.theme, label.clone());
            engine.reset_abort();
            let pos = fen::search_position(&game);
            engine.set_game_history(&fen::history_hashes(&game));
            let mut on_iteration = |info: &IterationInfo| {
                pb.set_message(iteration_message(&label, info));
            };
            let result = engine.search_limited(&pos, &settings.limits, Some(&mut on_iteration));
            pb.finish_and_clear();

            let Some(best) = result.best_move else {
                // No move available — should be unreachable for a live game.
                break;
            };

            // Ticker line: move number, side, move, eval, depth, nodes.
            println!(
                "{}",
                t!(
                    "watch.move_ticker",
                    num = game.fullmove_number,
                    color = side_label(side),
                    mv = best.to_string().green().bold(),
                    score = format_score(result.score),
                    depth = result.depth,
                    nodes = super::score::humanize_count(result.stats.nodes)
                )
            );

            if let Err(e) = game.make_move(&best.to_json()) {
                println!(
                    "{}: {}",
                    t!("terminal.error_label").to_string().red().bold(),
                    e
                );
                break;
            }

            print!(
                "{}",
                renderer.render(&game.board, &BoardHighlights::for_game(&game))
            );
            println!();
            println!("  {}", eval_bar_line(white_pov(result.score, side)));
            println!();

            // End shuffled games: claim any available draw automatically.
            if !game.is_over() && claim_available_draw(&mut game) {
                break;
            }

            // Pacing between moves — interactive terminals only.
            if ctx.theme.interactive && self.delay > 0 && !game.is_over() {
                std::thread::sleep(std::time::Duration::from_millis(self.delay));
            }
        }

        if game.is_over() {
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
        println!();
        Ok(())
    }
}
