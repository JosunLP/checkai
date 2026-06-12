//! `checkai play` — play chess in the terminal, against the built-in
//! engine (default) or a second human.
//!
//! The engine opponent is configured through the difficulty ladder in
//! [`super::level`] (see its table for the exact depth/time/TT mapping)
//! and can be fine-tuned with `--movetime` / `--depth` overrides.
//! While the engine thinks, a live spinner shows depth, score, node
//! counts and the principal variation; finished moves are announced and
//! rendered with from/to highlighting plus a one-line evaluation bar.

use std::time::Instant;

use clap::{Args, ValueEnum};
use colored::Colorize;

use super::board_renderer::{BoardHighlights, BoardRenderer, piece_name};
use super::fen;
use super::level::{DEFAULT_LEVEL, LevelSettings, MAX_LEVEL, MIN_LEVEL};
use super::panel::{TableRow, render_table, result_panel};
use super::progress::{iteration_message, spinner};
use super::score::{eval_bar_line, format_score, white_pov};
use super::{CliCommand, CliContext, CliResult, cli_error};
use crate::game::Game;
use crate::search::{IterationInfo, SearchEngine, SearchResult};
use crate::terminal::{GameCommand, read_input_line};
use crate::types::{ActionJson, ChessMove, Color};

/// Who the human plays against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Opponent {
    /// Play against the built-in engine.
    Engine,
    /// Two humans sharing the terminal.
    Human,
}

/// Which side the human takes against the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SideChoice {
    /// Play the white pieces (engine answers as Black).
    White,
    /// Play the black pieces (engine opens as White).
    Black,
    /// Flip a coin.
    Random,
}

/// Arguments for `checkai play`.
#[derive(Args, Debug)]
#[command(after_help = "\
Examples:\n\
  checkai play                         Play White vs the engine (level 5)\n\
  checkai play --level 9               A much stronger opponent\n\
  checkai play --color black           Play Black; the engine opens\n\
  checkai play --color random --flip   Random side, flipped board\n\
  checkai play --movetime 500          Cap engine thinking at 500 ms\n\
  checkai play --fen \"<FEN>\"           Start from a custom position\n\
  checkai play --vs human --ascii      Two players, ASCII board\n\
\n\
In-game commands: e2e4, moves, board, history, fen, json, hint, undo,\n\
resign, draw, help, quit (single-letter aliases shown via `help`).")]
pub struct PlayArgs {
    /// Opponent type: the built-in engine or a second human.
    #[arg(long, value_enum, default_value_t = Opponent::Engine)]
    pub vs: Opponent,

    /// Your color against the engine: white, black, or random.
    #[arg(long, value_enum, default_value_t = SideChoice::White)]
    pub color: SideChoice,

    /// Engine difficulty level (1 = beginner … 10 = strongest).
    #[arg(long, default_value_t = DEFAULT_LEVEL,
          value_parser = clap::value_parser!(u8).range(MIN_LEVEL as i64..=MAX_LEVEL as i64))]
    pub level: u8,

    /// Override the engine's thinking time per move, in milliseconds.
    #[arg(long)]
    pub movetime: Option<u64>,

    /// Override the engine's maximum search depth.
    #[arg(long)]
    pub depth: Option<i32>,

    /// Start from a custom position (4–6 field FEN string).
    #[arg(long)]
    pub fen: Option<String>,

    /// Use ASCII piece letters instead of Unicode glyphs.
    #[arg(long)]
    pub ascii: bool,

    /// Render the board from Black's perspective.
    #[arg(long)]
    pub flip: bool,
}

impl CliCommand for PlayArgs {
    fn run(self, ctx: &CliContext) -> CliResult {
        let initial_fen = self
            .fen
            .clone()
            .unwrap_or_else(|| fen::START_FEN.to_string());
        let game = fen::game_from_fen(&initial_fen)
            .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?;

        let human_color = match self.vs {
            Opponent::Human => None,
            Opponent::Engine => Some(resolve_side(self.color)),
        };
        let settings = LevelSettings::for_level(self.level, self.movetime, self.depth);
        let flipped = self.flip || human_color == Some(Color::Black);

        let mut session = PlaySession {
            ctx,
            game,
            initial_fen,
            engine: SearchEngine::new(settings.tt_size_mb),
            engine_color: human_color.map(|c| c.opponent()),
            settings,
            renderer: BoardRenderer::new(self.ascii, flipped),
            started: Instant::now(),
        };
        session.run()
    }
}

/// Resolves the `--color` choice (coin flip for `random`).
fn resolve_side(choice: SideChoice) -> Color {
    match choice {
        SideChoice::White => Color::White,
        SideChoice::Black => Color::Black,
        SideChoice::Random => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            if nanos.is_multiple_of(2) {
                Color::White
            } else {
                Color::Black
            }
        }
    }
}

/// Counts how often the current position occurred (threefold detection).
pub fn repetition_count(game: &Game) -> usize {
    match game.position_history.last() {
        Some(last) => game.position_history.iter().filter(|p| *p == last).count(),
        None => 0,
    }
}

/// Claims a threefold/fifty-move draw when eligible.
/// Returns `true` if the game ended through the claim.
pub fn claim_available_draw(game: &mut Game) -> bool {
    let reason = if repetition_count(game) >= 3 {
        Some("threefold_repetition")
    } else if game.halfmove_clock >= 100 {
        Some("fifty_move_rule")
    } else {
        None
    };
    if let Some(reason) = reason {
        let action = ActionJson {
            action: "claim_draw".to_string(),
            reason: Some(reason.to_string()),
        };
        let _ = game.process_action(&action);
    }
    game.is_over()
}

/// Returns the localized display name of a color.
pub fn color_name(color: Color) -> String {
    match color {
        Color::White => t!("types.white").to_string(),
        Color::Black => t!("types.black").to_string(),
    }
}

/// State of one interactive play session.
struct PlaySession<'a> {
    ctx: &'a CliContext,
    game: Game,
    initial_fen: String,
    engine: SearchEngine,
    /// `Some(color)` when the engine plays that side; `None` for
    /// human-vs-human (the engine is then only used for hints).
    engine_color: Option<Color>,
    settings: LevelSettings,
    renderer: BoardRenderer,
    started: Instant,
}

impl PlaySession<'_> {
    /// Runs the interactive REPL until the game ends or the user quits.
    fn run(&mut self) -> CliResult {
        self.print_intro();

        if self.game.legal_moves().is_empty() && !self.game.is_over() {
            println!("{}", t!("play.position_terminal").to_string().red());
            self.render_board();
            return Ok(());
        }

        self.render_board();
        self.print_status();

        loop {
            if self.game.is_over() {
                self.print_result();
                break;
            }

            if self.engine_color == Some(self.game.turn) {
                self.engine_turn();
                continue;
            }

            let prompt = format!("{} > ", side_label(self.game.turn));
            let Some(input) = read_input_line(&prompt) else {
                println!("{}", t!("terminal.goodbye"));
                break;
            };
            if input.is_empty() {
                continue;
            }

            match GameCommand::parse(&input) {
                Some(GameCommand::Quit) => {
                    println!("{}", t!("terminal.goodbye"));
                    break;
                }
                Some(cmd) => self.execute(cmd),
                None => println!(
                    "{}",
                    t!(
                        "terminal.unknown_cmd_hint",
                        cmd = &input,
                        help = "help".green()
                    )
                ),
            }
        }
        Ok(())
    }

    /// Executes a parsed in-game command (everything except `quit`).
    fn execute(&mut self, cmd: GameCommand) {
        match cmd {
            GameCommand::Move(mj) => match self.game.make_move(&mj) {
                Ok(()) => {
                    self.render_board();
                    self.print_status();
                }
                Err(e) => println!(
                    "{}: {}",
                    t!("terminal.illegal_move").to_string().red().bold(),
                    e
                ),
            },
            GameCommand::Moves => self.print_moves(),
            GameCommand::Board => {
                self.render_board();
                self.print_status();
            }
            GameCommand::History => self.print_history(),
            GameCommand::Fen => {
                println!("  {}", fen::game_to_fen(&self.game).green());
                println!();
            }
            GameCommand::Json => {
                let state = self.game.to_game_state_json();
                match serde_json::to_string_pretty(&state) {
                    Ok(json) => println!("{json}"),
                    Err(e) => println!(
                        "{}: {}",
                        t!("terminal.error_label").to_string().red().bold(),
                        e
                    ),
                }
                println!();
            }
            GameCommand::Hint => self.hint(),
            GameCommand::Undo => self.undo(),
            GameCommand::Resign => {
                let action = ActionJson {
                    action: "resign".to_string(),
                    reason: None,
                };
                match self.game.process_action(&action) {
                    Ok(()) => self.print_result(),
                    Err(e) => println!(
                        "{}: {}",
                        t!("terminal.error_label").to_string().red().bold(),
                        e
                    ),
                }
            }
            GameCommand::Draw => self.claim_draw(),
            GameCommand::Help => println!("{}", help_table()),
            GameCommand::Quit => unreachable!("quit is handled by the REPL loop"),
        }
    }

    /// Prints the session intro: mode, level, side and help hint.
    fn print_intro(&self) {
        println!();
        match self.engine_color {
            Some(engine_color) => println!(
                "{}",
                t!(
                    "play.intro_engine",
                    level = self.settings.level,
                    color = color_name(engine_color.opponent()).bold()
                )
            ),
            None => println!("{}", t!("play.intro_human")),
        }
        println!("{}", t!("play.intro_help_hint", help = "help".green()));
    }

    /// Lets the engine search and play its move, with a live spinner.
    fn engine_turn(&mut self) {
        let side = self.game.turn;
        let pos = fen::search_position(&self.game);
        self.engine
            .set_game_history(&fen::history_hashes(&self.game));
        let label = t!("play.engine_thinking", level = self.settings.level).to_string();
        let pb = spinner(&self.ctx.theme, label.clone());

        self.engine.reset_abort();
        let mut on_iteration = |info: &IterationInfo| {
            pb.set_message(iteration_message(&label, info));
        };
        let result =
            self.engine
                .search_limited(&pos, &self.settings.limits, Some(&mut on_iteration));
        pb.finish_and_clear();

        let Some(best) = result.best_move else {
            // No move found despite legal moves existing — concede.
            let action = ActionJson {
                action: "resign".to_string(),
                reason: None,
            };
            let _ = self.game.process_action(&action);
            println!("{}", t!("play.engine_resigns").to_string().yellow().bold());
            return;
        };

        self.announce_engine_move(&best, &result);
        if let Err(e) = self.game.make_move(&best.to_json()) {
            println!(
                "{}: {}",
                t!("terminal.error_label").to_string().red().bold(),
                e
            );
            return;
        }

        self.render_board();
        println!("  {}", eval_bar_line(white_pov(result.score, side)));
        println!();
        self.print_status();
    }

    /// Announces the engine's chosen move with search statistics.
    fn announce_engine_move(&self, mv: &ChessMove, result: &SearchResult) {
        let piece = self
            .game
            .board
            .get(mv.from)
            .map(|p| piece_name(p.kind))
            .unwrap_or_default();
        println!(
            "{}",
            t!(
                "play.engine_played",
                mv = mv.to_string().green().bold(),
                piece = piece,
                depth = result.depth,
                score = format_score(result.score),
                secs = format!("{:.1}", result.time_ms as f64 / 1000.0)
            )
        );
    }

    /// Runs an engine search for the current side and suggests a move.
    fn hint(&mut self) {
        let pos = fen::search_position(&self.game);
        self.engine
            .set_game_history(&fen::history_hashes(&self.game));
        let label = t!("play.hint_thinking").to_string();
        let pb = spinner(&self.ctx.theme, label.clone());
        self.engine.reset_abort();
        let mut on_iteration = |info: &IterationInfo| {
            pb.set_message(iteration_message(&label, info));
        };
        let result =
            self.engine
                .search_limited(&pos, &self.settings.limits, Some(&mut on_iteration));
        pb.finish_and_clear();

        match result.best_move {
            Some(mv) => println!(
                "{}",
                t!(
                    "play.hint_result",
                    mv = mv.to_string().green().bold(),
                    score = format_score(result.score)
                )
            ),
            None => println!("{}", t!("play.hint_none")),
        }
        println!();
    }

    /// Takes back the last full move by replaying from the start position.
    fn undo(&mut self) {
        // Against the engine, remove the engine's reply plus the human
        // move; in human-vs-human mode, remove a single half-move.
        let plies = if self.engine_color.is_some() { 2 } else { 1 };
        if self.game.move_history.len() < plies {
            println!("{}", t!("play.cannot_undo"));
            return;
        }
        let target = self.game.move_history.len() - plies;
        match self.rebuild_to(target) {
            Ok(rebuilt) => {
                self.game = rebuilt;
                println!("{}", t!("play.undo_done"));
                self.render_board();
                self.print_status();
            }
            Err(e) => println!(
                "{}: {}",
                t!("terminal.error_label").to_string().red().bold(),
                e
            ),
        }
    }

    /// Replays the game from its initial FEN up to `target` half-moves.
    fn rebuild_to(&self, target: usize) -> Result<Game, String> {
        let mut rebuilt = fen::game_from_fen(&self.initial_fen)?;
        for record in self.game.move_history.iter().take(target) {
            rebuilt.make_move(&record.move_json)?;
        }
        Ok(rebuilt)
    }

    /// Claims a draw when eligible, otherwise explains why not.
    fn claim_draw(&mut self) {
        if claim_available_draw(&mut self.game) {
            self.print_result();
        } else {
            println!(
                "{}",
                t!(
                    "terminal.no_draw_available",
                    clock = self.game.halfmove_clock,
                    reps = repetition_count(&self.game)
                )
            );
        }
    }

    /// Renders the board with last-move and check highlights.
    fn render_board(&self) {
        let highlights = BoardHighlights::for_game(&self.game);
        print!("{}", self.renderer.render(&self.game.board, &highlights));
        println!();
    }

    /// Prints the compact status line (move number, side to move, check).
    fn print_status(&self) {
        print!(
            "{}",
            t!(
                "terminal.move_status",
                num = self.game.fullmove_number,
                color = side_label(self.game.turn)
            )
        );
        if crate::movegen::is_in_check(&self.game.board, self.game.turn) {
            print!("  {}", t!("terminal.check").to_string().red().bold());
        }
        println!();
        println!();
    }

    /// Lists all legal moves in a compact grid.
    fn print_moves(&self) {
        let moves = self.game.legal_moves();
        println!(
            "{} {}",
            t!("terminal.legal_moves_header")
                .to_string()
                .yellow()
                .bold(),
            t!("terminal.moves_count", count = moves.len())
        );
        for (i, mv) in moves.iter().enumerate() {
            if i > 0 && i % 8 == 0 {
                println!();
            }
            print!("  {}", mv.to_string().green());
        }
        println!();
        println!();
    }

    /// Prints the move history as a numbered two-column table.
    fn print_history(&self) {
        if self.game.move_history.is_empty() {
            println!("{}", t!("terminal.no_moves_yet"));
            println!();
            return;
        }
        println!(
            "{}",
            t!("terminal.move_history_label")
                .to_string()
                .yellow()
                .bold()
        );
        println!(
            "  {:>4} {:<12} {:<12}",
            "#",
            t!("export.white_label"),
            t!("export.black_label")
        );
        let mut iter = self.game.move_history.iter().peekable();
        while let Some(record) = iter.next() {
            let white = if record.side == Color::White {
                record.notation.clone()
            } else {
                "…".to_string()
            };
            let black = if record.side == Color::White {
                match iter.peek() {
                    Some(next) if next.side == Color::Black => {
                        let n = next.notation.clone();
                        iter.next();
                        n
                    }
                    _ => String::new(),
                }
            } else {
                record.notation.clone()
            };
            println!("  {:>3}. {white:<12} {black:<12}", record.move_number);
        }
        println!();
    }

    /// Prints the end-of-game result panel.
    fn print_result(&self) {
        self.render_board();
        println!("{}", result_panel(&self.game, self.started.elapsed()));
        println!();
    }
}

/// Returns the colored display label for a side.
pub fn side_label(color: Color) -> colored::ColoredString {
    match color {
        Color::White => color_name(Color::White).white().bold(),
        Color::Black => color_name(Color::Black).blue().bold(),
    }
}

/// Builds the styled, data-driven in-game help table.
fn help_table() -> String {
    let rows = vec![
        TableRow::new("e2e4", None, t!("terminal.cmd_move").to_string()),
        TableRow::new("moves", Some("m"), t!("terminal.cmd_moves").to_string()),
        TableRow::new("board", Some("b"), t!("terminal.cmd_board").to_string()),
        TableRow::new(
            "history",
            Some("hist"),
            t!("terminal.cmd_history").to_string(),
        ),
        TableRow::new("fen", Some("f"), t!("terminal.cmd_fen").to_string()),
        TableRow::new("json", Some("j"), t!("terminal.cmd_json").to_string()),
        TableRow::new("hint", Some("i"), t!("play.cmd_hint").to_string()),
        TableRow::new("undo", Some("u"), t!("play.cmd_undo").to_string()),
        TableRow::new("resign", Some("r"), t!("terminal.cmd_resign").to_string()),
        TableRow::new("draw", Some("d"), t!("terminal.cmd_draw").to_string()),
        TableRow::new("help", Some("h"), t!("terminal.cmd_help").to_string()),
        TableRow::new("quit", Some("q"), t!("terminal.cmd_quit").to_string()),
    ];
    format!(
        "{}\n{}",
        t!("terminal.cmd_header").to_string().yellow().bold(),
        render_table(&rows, 2)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repetition_count_initial_position() {
        let game = Game::new();
        assert_eq!(repetition_count(&game), 1);
    }

    #[test]
    fn test_claim_draw_not_available_at_start() {
        let mut game = Game::new();
        assert!(!claim_available_draw(&mut game));
        assert!(!game.is_over());
    }

    #[test]
    fn test_resolve_side_fixed_choices() {
        assert_eq!(resolve_side(SideChoice::White), Color::White);
        assert_eq!(resolve_side(SideChoice::Black), Color::Black);
        // Random must return one of the two without panicking.
        let c = resolve_side(SideChoice::Random);
        assert!(c == Color::White || c == Color::Black);
    }
}
