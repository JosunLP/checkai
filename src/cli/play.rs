//! `checkai play` — play chess in the terminal, against the built-in
//! engine (default) or a second human.
//!
//! The engine opponent is configured through the difficulty ladder in
//! [`super::level`] (see its table for the exact depth/time/skill mapping)
//! and can be fine-tuned with `--movetime` / `--depth` / `--threads`
//! overrides, an opening book, a tablebase and a real chess clock.
//!
//! While the engine thinks, the board and a live search panel repaint in
//! place — depth, score, node rate, a colour-graded evaluation bar and the
//! principal variation — and the chosen move is then animated across the
//! board. Every animation is TTY-gated, so piped output stays plain text.

use std::path::PathBuf;
use std::time::Instant;

use clap::{Args, ValueEnum};
use colored::Colorize;

use super::animate::{self, LiveRegion};
use super::board_renderer::{
    BoardHighlights, BoardRenderer, BoardTheme, CapturedMaterial, piece_name,
};
use super::clock::{GameClock, TimeControl};
use super::engine::EngineArgs;
use super::fen;
use super::level::{DEFAULT_LEVEL, LevelSettings, MAX_LEVEL, MIN_LEVEL};
use super::panel::{TableRow, render_table, result_panel};
use super::pgn::{self, PgnMetadata};
use super::progress::ThinkingView;
use super::score::{eval_bar_line, format_score, humanize_count, white_pov};
use super::{CliCommand, CliContext, CliResult, cli_error};
use crate::game::Game;
use crate::search::{IterationInfo, MoveSource, SearchEngine, SearchLimits, SearchResult};
use crate::terminal::{GameCommand, read_input_line};
use crate::types::{ActionJson, ChessMove, Color, MoveJson};

/// Default file name used by `save` when no path is given.
const DEFAULT_PGN_FILE: &str = "checkai-game.pgn";

/// Search depth used by the in-game `analyze` command.
const ANALYZE_DEPTH: i32 = 14;

/// Number of lines the in-game `analyze` command reports.
const ANALYZE_LINES: usize = 4;

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
  checkai play --time 5+3              Five minutes each, 3s increment\n\
  checkai play --threads 4 --hash 256  Give the engine more muscle\n\
  checkai play --book book.bin         Let the engine use an opening book\n\
  checkai play --board ice             A different board palette\n\
  checkai play --pgn game.pgn          Resume a game from a PGN file\n\
  checkai play --vs human --ascii      Two players, ASCII board\n\
\n\
In-game commands: e2e4 or Nf3, moves, board, history, fen, pgn, json, hint,\n\
analyze, eval, book, tb, undo, redo, flip, new, level N, save [file],\n\
load <file>, resign, draw, help, quit (aliases shown via `help`).")]
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

    /// Time control, e.g. `5+3` (5 minutes plus 3 seconds per move).
    #[arg(long, value_name = "SPEC")]
    pub time: Option<String>,

    /// Start from a custom position (4–6 field FEN string).
    #[arg(long)]
    pub fen: Option<String>,

    /// Resume a game from a PGN file.
    #[arg(long, value_name = "FILE")]
    pub pgn: Option<PathBuf>,

    /// Use ASCII piece letters instead of Unicode glyphs.
    #[arg(long)]
    pub ascii: bool,

    /// Board colour palette.
    #[arg(long, value_enum, default_value_t = BoardTheme::default())]
    pub board: BoardTheme,

    /// Render the board from Black's perspective.
    #[arg(long)]
    pub flip: bool,

    /// Disable the move animation (boards still repaint in place).
    #[arg(long)]
    pub no_animation: bool,

    #[command(flatten)]
    pub engine: EngineArgs,
}

impl CliCommand for PlayArgs {
    fn run(self, ctx: &CliContext) -> CliResult {
        // A PGN file wins over --fen: it carries both a start position and
        // the moves played so far.
        let (game, initial_fen) = match &self.pgn {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| cli_error(t!("play.pgn_read_failed", error = e).to_string()))?;
                let games = pgn::parse_pgn(&text).map_err(cli_error)?;
                let first = games.first().ok_or_else(|| cli_error("empty PGN"))?;
                let game = first.to_game().map_err(cli_error)?;
                let start = fen::game_to_fen(&pgn::start_position_of(&game));
                (game, start)
            }
            None => {
                let initial_fen = self
                    .fen
                    .clone()
                    .unwrap_or_else(|| fen::START_FEN.to_string());
                let game = fen::game_from_fen(&initial_fen)
                    .map_err(|e| cli_error(t!("cli.invalid_fen", error = e).to_string()))?;
                (game, initial_fen)
            }
        };

        let human_color = match self.vs {
            Opponent::Human => None,
            Opponent::Engine => Some(resolve_side(self.color)),
        };
        let settings = LevelSettings::for_level(self.level, self.movetime, self.depth);
        let flipped = self.flip || human_color == Some(Color::Black);

        let clock = match &self.time {
            Some(spec) => Some(GameClock::new(TimeControl::parse(spec).map_err(|e| {
                cli_error(t!("play.bad_time_control", error = e).to_string())
            })?)),
            None => None,
        };

        // The level ladder supplies the hash size unless the user overrode it.
        let mut config = self.engine.build_config(settings.tt_size_mb);
        config.skill_level = settings.skill;

        let renderer = BoardRenderer::for_theme(ctx.theme.colors, self.ascii, flipped, self.board);

        let mut session = PlaySession {
            ctx,
            game,
            initial_fen,
            redo_stack: Vec::new(),
            engine: SearchEngine::with_config(config),
            engine_color: human_color.map(|c| c.opponent()),
            settings,
            renderer,
            clock,
            animate_moves: !self.no_animation,
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
/// Returns `Ok(true)` if the game ended through the claim, `Ok(false)` if no
/// draw was available, or `Err` if the claim action failed unexpectedly.
pub fn claim_available_draw(game: &mut Game) -> Result<bool, String> {
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
        game.process_action(&action).map_err(|e| e.to_string())?;
    }
    Ok(game.is_over())
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
    /// Moves taken back with `undo`, replayable with `redo`.
    redo_stack: Vec<MoveJson>,
    engine: SearchEngine,
    /// `Some(color)` when the engine plays that side; `None` for
    /// human-vs-human (the engine is then only used for hints).
    engine_color: Option<Color>,
    settings: LevelSettings,
    renderer: BoardRenderer,
    clock: Option<GameClock>,
    animate_moves: bool,
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
            if self.clock.as_ref().is_some_and(GameClock::is_flagged) {
                self.finish_on_time();
                break;
            }

            if self.engine_color == Some(self.game.turn) {
                self.engine_turn();
                continue;
            }

            if let Some(clock) = self.clock.as_mut() {
                clock.start(self.game.turn);
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
            GameCommand::Move(mj) => self.play_human_move(mj),
            GameCommand::San(token) => match pgn::san_to_move(&self.game, &token) {
                Some(mj) => self.play_human_move(mj),
                None => println!(
                    "{}: {}",
                    t!("terminal.illegal_move").to_string().red().bold(),
                    token
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
            GameCommand::Pgn => {
                println!("{}", pgn::write_pgn(&self.game, &self.pgn_metadata()));
            }
            GameCommand::Json => {
                let state = self.game.to_game_state_json();
                match serde_json::to_string_pretty(&state) {
                    Ok(json) => println!("{json}"),
                    Err(e) => self.print_error(&e.to_string()),
                }
                println!();
            }
            GameCommand::Hint => self.hint(),
            GameCommand::Analyze => self.analyze_position(),
            GameCommand::Eval => self.print_static_eval(),
            GameCommand::Book => self.print_book(),
            GameCommand::Tablebase => self.print_tablebase(),
            GameCommand::Undo => self.undo(),
            GameCommand::Redo => self.redo(),
            GameCommand::Flip => {
                self.renderer.flipped = !self.renderer.flipped;
                self.render_board();
                self.print_status();
            }
            GameCommand::New => self.restart(),
            GameCommand::Level(level) => self.set_level(level),
            GameCommand::Save(path) => self.save_pgn(path.as_deref()),
            GameCommand::Load(path) => self.load_pgn(&path),
            GameCommand::Resign => {
                let action = ActionJson {
                    action: "resign".to_string(),
                    reason: None,
                };
                match self.game.process_action(&action) {
                    Ok(()) => self.print_result(),
                    Err(e) => self.print_error(&e.to_string()),
                }
            }
            GameCommand::Draw => self.claim_draw(),
            GameCommand::Help => println!("{}", help_table()),
            GameCommand::Quit => unreachable!("quit is handled by the REPL loop"),
        }
    }

    /// Applies a human move, stopping the clock and animating the piece.
    fn play_human_move(&mut self, mj: MoveJson) {
        let before = self.game.board.clone();
        let squares = square_pair(&mj);
        match self.game.make_move(&mj) {
            Ok(()) => {
                if let Some(clock) = self.clock.as_mut() {
                    clock.stop();
                }
                self.redo_stack.clear();
                if let Some((from, to)) = squares {
                    self.animate(&before, from, to);
                }
                self.render_board();
                self.print_status();
            }
            Err(e) => println!(
                "{}: {}",
                t!("terminal.illegal_move").to_string().red().bold(),
                e
            ),
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
        super::engine::print_engine_banner(&self.ctx.theme, self.engine.config());
        if let Some(clock) = &self.clock {
            println!(
                "  {}",
                t!(
                    "play.clock_intro",
                    white = clock.format(Color::White),
                    black = clock.format(Color::Black)
                )
                .to_string()
                .dimmed()
            );
        }
        println!("{}", t!("play.intro_help_hint", help = "help".green()));
    }

    /// Lets the engine search and play its move, with a live search panel.
    fn engine_turn(&mut self) {
        let side = self.game.turn;
        let position = fen::search_position(&self.game);
        self.engine
            .set_game_history(&fen::history_hashes(&self.game));

        let mut limits = self.settings.limits.clone();
        limits.max_nodes = limits.max_nodes.or(None);
        if let Some(clock) = self.clock.as_mut() {
            clock.start(side);
            limits.move_time_ms = Some(clock.budget_ms(side));
        }

        let label = t!("play.engine_thinking", level = self.settings.level).to_string();
        let header = self.header_line();
        let board = self.board_block();
        let mut region = LiveRegion::new(&self.ctx.theme);
        let mut view = ThinkingView::new(label, side);
        region.frame(&animate::compose(&header, &board, &view.render()));

        self.engine.reset_abort();
        let result = {
            // The callback borrows the view; keep the borrow scoped so the
            // session stays usable afterwards.
            let region = &mut region;
            let view = &mut view;
            let header = header.as_str();
            let board = board.as_str();
            let mut on_iteration = |info: &IterationInfo| {
                view.tick += 1;
                view.info = Some(info.clone());
                region.frame(&animate::compose(header, board, &view.render()));
            };
            self.engine
                .search_limited(&position, &limits, Some(&mut on_iteration))
        };
        region.clear();

        if let Some(clock) = self.clock.as_mut() {
            clock.stop();
        }

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
        let before = self.game.board.clone();
        if let Err(e) = self.game.make_move(&best.to_json()) {
            self.print_error(&e.to_string());
            return;
        }
        self.redo_stack.clear();
        self.animate(&before, best.from, best.to);

        self.render_board();
        println!("  {}", eval_bar_line(white_pov(result.score, side)));
        println!();
        self.print_status();
    }

    /// Announces the engine's chosen move with search statistics.
    fn announce_engine_move(&self, mv: &ChessMove, result: &SearchResult) {
        if result.source == MoveSource::Book {
            println!(
                "{}",
                t!("play.engine_book_move", mv = mv.to_string().green().bold())
            );
            return;
        }
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
                mv = pgn::move_to_san(&self.game, mv).green().bold(),
                piece = piece,
                depth = result.depth,
                score = format_score(result.score),
                secs = format!("{:.1}", result.time_ms as f64 / 1000.0)
            )
        );
        println!(
            "  {}",
            t!(
                "play.engine_stats",
                nodes = humanize_count(result.stats.nodes),
                nps = humanize_count(result.nps()),
                seldepth = result.seldepth
            )
            .to_string()
            .dimmed()
        );
    }

    /// Runs an engine search for the current side and suggests a move.
    fn hint(&mut self) {
        let position = fen::search_position(&self.game);
        self.engine
            .set_game_history(&fen::history_hashes(&self.game));
        let label = t!("play.hint_thinking").to_string();
        let pb = super::progress::spinner(&self.ctx.theme, label.clone());
        self.engine.reset_abort();
        let mut on_iteration = |info: &IterationInfo| {
            pb.set_message(super::progress::iteration_message(&label, info));
        };
        let result =
            self.engine
                .search_limited(&position, &self.settings.limits, Some(&mut on_iteration));
        pb.finish_and_clear();

        match result.best_move {
            Some(mv) => {
                println!(
                    "{}",
                    t!(
                        "play.hint_result",
                        mv = pgn::move_to_san(&self.game, &mv).green().bold(),
                        score = format_score(result.score)
                    )
                );
                // Show the suggestion on the board itself.
                let highlights =
                    BoardHighlights::for_game(&self.game).with_targets(vec![mv.from, mv.to]);
                print!("{}", self.renderer.render(&self.game.board, &highlights));
            }
            None => println!("{}", t!("play.hint_none")),
        }
        println!();
    }

    /// Runs a deeper MultiPV analysis of the current position.
    fn analyze_position(&mut self) {
        let position = fen::search_position(&self.game);
        self.engine
            .set_game_history(&fen::history_hashes(&self.game));
        let previous_multipv = self.engine.config().multi_pv;
        self.engine.set_multi_pv(ANALYZE_LINES);

        let label = t!("play.analyze_thinking").to_string();
        let pb = super::progress::spinner(&self.ctx.theme, label.clone());
        self.engine.reset_abort();
        let limits = SearchLimits {
            max_depth: ANALYZE_DEPTH,
            move_time_ms: Some(
                self.settings
                    .limits
                    .move_time_ms
                    .unwrap_or(2_000)
                    .max(2_000),
            ),
            ..SearchLimits::default()
        };
        let mut on_iteration = |info: &IterationInfo| {
            pb.set_message(super::progress::iteration_message(&label, info));
        };
        let result = self
            .engine
            .search_limited(&position, &limits, Some(&mut on_iteration));
        pb.finish_and_clear();
        self.engine.set_multi_pv(previous_multipv);

        println!(
            "{}",
            t!("play.analyze_header", depth = result.depth)
                .to_string()
                .yellow()
                .bold()
        );
        for line in &result.pv_lines {
            let score = match line.mate_in {
                Some(mate) => format!("#{mate}"),
                None => format_score(line.score),
            };
            let moves: Vec<String> = line.moves.iter().map(|m| m.to_string()).collect();
            println!(
                "  {}. {:>8}  {}",
                line.rank,
                score.cyan(),
                moves.join(" ").dimmed()
            );
        }
        println!();
    }

    /// Prints the static evaluation of the current position.
    fn print_static_eval(&self) {
        let static_cp = crate::eval::evaluate(&self.game.board, self.game.turn);
        let captured = CapturedMaterial::for_board(&self.game.board);
        println!("{}", t!("play.eval_header").to_string().yellow().bold());
        println!("  {}", eval_bar_line(white_pov(static_cp, self.game.turn)));
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
        println!();
    }

    /// Prints the opening-book entries for the current position.
    fn print_book(&self) {
        let Some(book) = self.engine.config().book.as_ref() else {
            println!("{}", t!("play.book_not_loaded").to_string().dimmed());
            println!();
            return;
        };
        let entries = book.lookup(
            &self.game.board,
            self.game.turn,
            &self.game.castling,
            self.game.en_passant,
        );
        if entries.is_empty() {
            println!("{}", t!("eval.book_miss").to_string().dimmed());
        } else {
            let total: u32 = entries.iter().map(|e| u32::from(e.weight)).sum();
            for entry in &entries {
                let share = if total > 0 {
                    f64::from(entry.weight) * 100.0 / f64::from(total)
                } else {
                    0.0
                };
                println!(
                    "  {:<8} {:>6.1}%",
                    entry.chess_move.to_string().green(),
                    share
                );
            }
        }
        println!();
    }

    /// Prints the tablebase verdict for the current position.
    fn print_tablebase(&self) {
        let Some(tb) = self.engine.config().tablebase.as_ref() else {
            println!("{}", t!("play.tablebase_not_loaded").to_string().dimmed());
            println!();
            return;
        };
        let info = tb.probe(
            &self.game.board,
            self.game.turn,
            &self.game.castling,
            self.game.en_passant,
        );
        println!(
            "  {}",
            t!(
                "eval.tablebase_line",
                config = info.configuration,
                wdl = info
                    .wdl
                    .map(|w| format!("{w:?}"))
                    .unwrap_or_else(|| "—".to_string()),
                dtz = info
                    .dtz
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "—".to_string()),
                source = info.source
            )
        );
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
        let undone: Vec<MoveJson> = self.game.move_history[target..]
            .iter()
            .map(|record| record.move_json.clone())
            .collect();
        match self.rebuild_to(target) {
            Ok(rebuilt) => {
                self.game = rebuilt;
                for mv in undone.into_iter().rev() {
                    self.redo_stack.push(mv);
                }
                println!("{}", t!("play.undo_done"));
                self.render_board();
                self.print_status();
            }
            Err(e) => self.print_error(&e),
        }
    }

    /// Replays the moves most recently taken back.
    fn redo(&mut self) {
        let plies = if self.engine_color.is_some() { 2 } else { 1 };
        if self.redo_stack.len() < plies {
            println!("{}", t!("play.cannot_redo"));
            return;
        }
        for _ in 0..plies {
            let Some(mv) = self.redo_stack.pop() else {
                break;
            };
            if let Err(e) = self.game.make_move(&mv) {
                self.print_error(&e);
                self.redo_stack.clear();
                break;
            }
        }
        println!("{}", t!("play.redo_done"));
        self.render_board();
        self.print_status();
    }

    /// Replays the game from its initial FEN up to `target` half-moves.
    fn rebuild_to(&self, target: usize) -> Result<Game, String> {
        let mut rebuilt = fen::game_from_fen(&self.initial_fen)?;
        for record in self.game.move_history.iter().take(target) {
            rebuilt.make_move(&record.move_json)?;
        }
        Ok(rebuilt)
    }

    /// Starts a fresh game from the session's initial position.
    fn restart(&mut self) {
        match fen::game_from_fen(&self.initial_fen) {
            Ok(game) => {
                self.game = game;
                self.redo_stack.clear();
                self.started = Instant::now();
                self.engine.clear_memory();
                println!("{}", t!("play.new_game").to_string().green());
                self.render_board();
                self.print_status();
            }
            Err(e) => self.print_error(&e),
        }
    }

    /// Changes the engine difficulty mid-game.
    fn set_level(&mut self, level: u8) {
        let level = level.clamp(MIN_LEVEL, MAX_LEVEL);
        self.settings = LevelSettings::for_level(level, None, None);
        let mut config = self.engine.config().clone();
        config.skill_level = self.settings.skill;
        self.engine.set_config(config);
        println!(
            "{}",
            t!("play.level_changed", level = level).to_string().green()
        );
        println!();
    }

    /// Writes the game to a PGN file.
    fn save_pgn(&self, path: Option<&str>) {
        let path = path.unwrap_or(DEFAULT_PGN_FILE);
        let text = pgn::write_pgn(&self.game, &self.pgn_metadata());
        match std::fs::write(path, text) {
            Ok(()) => println!("{}", t!("play.pgn_saved", path = path).to_string().green()),
            Err(e) => self.print_error(t!("play.pgn_write_failed", error = e).as_ref()),
        }
        println!();
    }

    /// Replaces the current game with one loaded from a PGN file.
    fn load_pgn(&mut self, path: &str) {
        let loaded = std::fs::read_to_string(path)
            .map_err(|e| t!("play.pgn_read_failed", error = e).to_string())
            .and_then(|text| pgn::parse_pgn(&text))
            .and_then(|games| {
                games
                    .first()
                    .ok_or_else(|| "empty PGN".to_string())
                    .and_then(|g| g.to_game())
            });
        match loaded {
            Ok(game) => {
                self.initial_fen = fen::game_to_fen(&pgn::start_position_of(&game));
                self.game = game;
                self.redo_stack.clear();
                println!("{}", t!("play.pgn_loaded", path = path).to_string().green());
                self.render_board();
                self.print_status();
            }
            Err(e) => self.print_error(&e),
        }
    }

    /// PGN tags describing this session.
    fn pgn_metadata(&self) -> PgnMetadata {
        let engine_name = format!(
            "CheckAI {} (level {})",
            crate::update::version(),
            self.settings.level
        );
        let (white, black) = match self.engine_color {
            Some(Color::White) => (engine_name, "Human".to_string()),
            Some(Color::Black) => ("Human".to_string(), engine_name),
            None => ("Human".to_string(), "Human".to_string()),
        };
        PgnMetadata {
            event: "CheckAI CLI Game".to_string(),
            white,
            black,
            ..PgnMetadata::default()
        }
    }

    /// Claims a draw when eligible, otherwise explains why not.
    fn claim_draw(&mut self) {
        match claim_available_draw(&mut self.game) {
            Err(e) => eprintln!("error: draw claim failed: {e}"),
            Ok(true) => self.print_result(),
            Ok(false) => println!(
                "{}",
                t!(
                    "terminal.no_draw_available",
                    clock = self.game.halfmove_clock,
                    reps = repetition_count(&self.game)
                )
            ),
        }
    }

    /// Ends the game when a side has run out of time.
    fn finish_on_time(&mut self) {
        let Some(loser) = self.clock.as_ref().and_then(GameClock::flagged) else {
            return;
        };
        println!(
            "{}",
            t!("play.flag_fall", color = color_name(loser))
                .to_string()
                .red()
                .bold()
        );
        let action = ActionJson {
            action: "resign".to_string(),
            reason: Some("timeout".to_string()),
        };
        // The clock's loser is the side to move, so a resignation records the
        // correct winner.
        if self.game.turn == loser {
            let _ = self.game.process_action(&action);
        }
        self.print_result();
    }

    /// Animates a move across the board, when animations are enabled.
    fn animate(
        &self,
        before: &crate::types::Board,
        from: crate::types::Square,
        to: crate::types::Square,
    ) {
        if !self.animate_moves {
            return;
        }
        let mut region = LiveRegion::new(&self.ctx.theme);
        animate::animate_move(
            &self.ctx.theme,
            &mut region,
            &self.renderer,
            before,
            from,
            to,
            "",
            "",
        );
        region.clear();
    }

    /// The board as a repaintable block, including the captured-material row.
    fn board_block(&self) -> String {
        let highlights = BoardHighlights::for_game(&self.game);
        format!(
            "{}{}\n",
            self.renderer.render(&self.game.board, &highlights),
            self.material_line()
        )
    }

    /// Header line shown above the board while the engine thinks.
    fn header_line(&self) -> String {
        match &self.clock {
            Some(clock) => format!(
                "  {} {}   {} {}",
                "W".white().bold(),
                clock.format(Color::White),
                "B".blue().bold(),
                clock.format(Color::Black)
            ),
            None => String::new(),
        }
    }

    /// Captured pieces and material balance, as one line.
    fn material_line(&self) -> String {
        let captured = CapturedMaterial::for_board(&self.game.board);
        if captured.by_white.is_empty() && captured.by_black.is_empty() {
            return String::new();
        }
        format!(
            "  {} {}   {} {}   {}",
            "W".white().bold(),
            captured.glyphs(Color::White).dimmed(),
            "B".blue().bold(),
            captured.glyphs(Color::Black).dimmed(),
            format!("{:+}", captured.balance).cyan()
        )
    }

    /// Renders the board with last-move and check highlights.
    fn render_board(&self) {
        print!("{}", self.board_block());
        println!();
    }

    /// Prints the compact status line (move number, side to move, clocks).
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
        if let Some(clock) = &self.clock {
            print!(
                "  {}",
                format!(
                    "⏱ {} / {}",
                    clock.format(Color::White),
                    clock.format(Color::Black)
                )
                .dimmed()
            );
        }
        println!();
        println!();
    }

    /// Lists all legal moves in a compact grid, in SAN.
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
            if i > 0 && i.is_multiple_of(8) {
                println!();
            }
            print!("  {:<7}", pgn::move_to_san(&self.game, mv).green());
        }
        println!();
        println!();
    }

    /// Prints the move history as a numbered two-column SAN table.
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
        let san = pgn::history_to_san(&self.game);
        let start = pgn::start_position_of(&self.game);
        let mut number = start.fullmove_number;
        let mut side = start.turn;
        let mut white_cell = if side == Color::Black {
            "…".to_string()
        } else {
            String::new()
        };
        for token in san {
            if side == Color::White {
                white_cell = token;
            } else {
                println!("  {number:>3}. {white_cell:<12} {token:<12}");
                white_cell = String::new();
                number += 1;
            }
            side = side.opponent();
        }
        if !white_cell.is_empty() {
            println!("  {number:>3}. {white_cell:<12}");
        }
        println!();
    }

    /// Prints an error line in the standard style.
    fn print_error(&self, message: &str) {
        println!(
            "{}: {}",
            t!("terminal.error_label").to_string().red().bold(),
            message
        );
    }

    /// Prints the end-of-game result panel, with a checkmate flourish.
    fn print_result(&self) {
        if let Some(king) = self.game.board.find_king(self.game.turn)
            && crate::movegen::is_in_check(&self.game.board, self.game.turn)
        {
            let mut region = LiveRegion::new(&self.ctx.theme);
            let highlights = BoardHighlights::for_game(&self.game);
            animate::flash_squares(
                &self.ctx.theme,
                &mut region,
                &self.renderer,
                &self.game.board,
                &highlights,
                &[king],
                3,
                "",
                "",
            );
            region.clear();
        }
        self.render_board();
        println!("{}", result_panel(&self.game, self.started.elapsed()));
        println!();
    }
}

/// Extracts the from/to squares of a move for animation.
fn square_pair(mj: &MoveJson) -> Option<(crate::types::Square, crate::types::Square)> {
    Some((
        crate::types::Square::from_algebraic(&mj.from)?,
        crate::types::Square::from_algebraic(&mj.to)?,
    ))
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
        TableRow::new("e2e4 · Nf3", None, t!("terminal.cmd_move").to_string()),
        TableRow::new("moves", Some("m"), t!("terminal.cmd_moves").to_string()),
        TableRow::new("board", Some("b"), t!("terminal.cmd_board").to_string()),
        TableRow::new("flip", None, t!("play.cmd_flip").to_string()),
        TableRow::new(
            "history",
            Some("hist"),
            t!("terminal.cmd_history").to_string(),
        ),
        TableRow::new("fen", Some("f"), t!("terminal.cmd_fen").to_string()),
        TableRow::new("pgn", None, t!("play.cmd_pgn").to_string()),
        TableRow::new("json", Some("j"), t!("terminal.cmd_json").to_string()),
        TableRow::new("hint", Some("i"), t!("play.cmd_hint").to_string()),
        TableRow::new("analyze", Some("a"), t!("play.cmd_analyze").to_string()),
        TableRow::new("eval", Some("e"), t!("play.cmd_eval").to_string()),
        TableRow::new("book", None, t!("play.cmd_book").to_string()),
        TableRow::new("tb", None, t!("play.cmd_tablebase").to_string()),
        TableRow::new("undo", Some("u"), t!("play.cmd_undo").to_string()),
        TableRow::new("redo", None, t!("play.cmd_redo").to_string()),
        TableRow::new("level N", Some("l"), t!("play.cmd_level").to_string()),
        TableRow::new("save [file]", Some("s"), t!("play.cmd_save").to_string()),
        TableRow::new("load <file>", Some("o"), t!("play.cmd_load").to_string()),
        TableRow::new("new", None, t!("play.cmd_new").to_string()),
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
        assert!(!claim_available_draw(&mut game).expect("draw claim should not fail"));
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

    #[test]
    fn test_square_pair_parses_move_json() {
        let mj = MoveJson {
            from: "e2".into(),
            to: "e4".into(),
            promotion: None,
        };
        let (from, to) = square_pair(&mj).expect("valid squares");
        assert_eq!(from.to_algebraic(), "e2");
        assert_eq!(to.to_algebraic(), "e4");
        let bad = MoveJson {
            from: "z9".into(),
            to: "e4".into(),
            promotion: None,
        };
        assert!(square_pair(&bad).is_none());
    }

    #[test]
    fn test_help_table_lists_every_command() {
        colored::control::set_override(false);
        let table = help_table();
        for command in ["moves", "analyze", "eval", "book", "redo", "save", "load"] {
            assert!(table.contains(command), "help must mention '{command}'");
        }
    }
}
