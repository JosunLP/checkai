//! `checkai uci` — the Universal Chess Interface protocol.
//!
//! Speaks plain UCI on stdin/stdout so CheckAI can be plugged into any
//! chess GUI or match runner (cutechess-cli, fastchess, Arena, …).
//!
//! Supported commands: `uci`, `isready`, `setoption`, `ucinewgame`,
//! `position [startpos | fen <FEN>] [moves ...]`,
//! `go [depth N | movetime MS | nodes N | mate N | searchmoves ... |
//! wtime/btime/winc/binc/movestogo | ponder | infinite]`, `ponderhit`,
//! `stop`, `quit`, plus the conventional `d` / `eval` debug commands.
//!
//! Supported options:
//!
//! | Option              | Type   | Effect                                  |
//! |---------------------|--------|-----------------------------------------|
//! | `Hash`              | spin   | Transposition table size in MB          |
//! | `Threads`           | spin   | Lazy SMP search threads                 |
//! | `MultiPV`           | spin   | Number of reported principal variations |
//! | `Move Overhead`     | spin   | Latency subtracted from every budget    |
//! | `Ponder`            | check  | Advertises pondering support            |
//! | `OwnBook`           | check  | Use the configured opening book         |
//! | `BookFile`          | string | Path to a Polyglot `.bin` book          |
//! | `SyzygyPath`        | string | Path to a Syzygy tablebase directory    |
//! | `UCI_LimitStrength` | check  | Enable artificial strength limiting     |
//! | `UCI_Elo`           | spin   | Target strength when limiting           |
//! | `Skill Level`       | spin   | Direct 0–20 skill limit                 |
//! | `Clear Hash`        | button | Drop all learned tables                 |
//!
//! Searches run on a dedicated `std::thread` with the engine's abort
//! token wired to `stop`, emitting `info` lines from the iterative-
//! deepening callback and a final `bestmove`.
//!
//! This module is machine-facing: it deliberately bypasses i18n and
//! colors — output is pure protocol text.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use clap::Args;

use super::board_renderer::{BoardHighlights, BoardRenderer};
use super::fen;
use super::{CliCommand, CliContext, CliResult};
use crate::game::Game;
use crate::opening_book::OpeningBook;
use crate::search::{
    EngineConfig, IterationInfo, MAX_DEPTH, MAX_MULTI_PV, MAX_SKILL, MAX_THREADS, SearchEngine,
    SearchLimits,
};
use crate::tablebase::SyzygyTablebase;
use crate::types::{ChessMove, Color, MoveJson};

/// Default transposition table size (MB), matching the advertised option.
const DEFAULT_HASH_MB: usize = 64;
/// Minimum accepted Hash size (MB).
const MIN_HASH_MB: usize = 1;
/// Maximum accepted Hash size (MB).
const MAX_HASH_MB: usize = 4096;
/// Fraction of the remaining clock budgeted per move (1/25th).
const CLOCK_DIVISOR: u64 = 25;
/// Minimum time budget per move (ms) when playing on a clock.
const MIN_BUDGET_MS: u64 = 10;
/// Lowest `UCI_Elo` the strength limiter accepts.
const MIN_UCI_ELO: u32 = 800;
/// Highest `UCI_Elo` the strength limiter accepts (full strength above it).
const MAX_UCI_ELO: u32 = 2850;

/// Arguments for `checkai uci` (none — pure stdio protocol).
#[derive(Args, Debug)]
#[command(after_help = "\
Example session:\n\
  $ checkai uci\n\
  uci\n\
  setoption name Threads value 4\n\
  setoption name MultiPV value 3\n\
  position startpos moves e2e4\n\
  go movetime 1000\n\
  quit")]
pub struct UciArgs {}

impl CliCommand for UciArgs {
    fn run(self, _ctx: &CliContext) -> CliResult {
        run_uci_loop();
        Ok(())
    }
}

/// Maps a `UCI_Elo` target onto the engine's 0–20 skill scale.
///
/// The mapping is linear between [`MIN_UCI_ELO`] and [`MAX_UCI_ELO`]; ratings
/// at or above the maximum play at full strength.
pub fn elo_to_skill(elo: u32) -> u8 {
    if elo >= MAX_UCI_ELO {
        return MAX_SKILL;
    }
    let span = f64::from(MAX_UCI_ELO - MIN_UCI_ELO);
    let above = f64::from(elo.max(MIN_UCI_ELO) - MIN_UCI_ELO);
    ((above / span) * f64::from(MAX_SKILL)).round() as u8
}

// ---------------------------------------------------------------------------
// Command parsing (pure functions — unit tested)
// ---------------------------------------------------------------------------

/// `go` sub-parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoParams {
    /// Fixed search depth in plies.
    pub depth: Option<i32>,
    /// Fixed time per move in milliseconds.
    pub movetime: Option<u64>,
    /// Node budget.
    pub nodes: Option<u64>,
    /// Stop once a mate in at most this many moves is proven.
    pub mate: Option<i32>,
    /// White's remaining clock time (ms).
    pub wtime: Option<u64>,
    /// Black's remaining clock time (ms).
    pub btime: Option<u64>,
    /// White's increment per move (ms).
    pub winc: Option<u64>,
    /// Black's increment per move (ms).
    pub binc: Option<u64>,
    /// Moves until the next time control.
    pub movestogo: Option<u64>,
    /// Restrict the search to these root moves.
    pub search_moves: Vec<String>,
    /// Search on the opponent's clock until `ponderhit` or `stop`.
    pub ponder: bool,
    /// Search until `stop`.
    pub infinite: bool,
}

/// A parsed UCI command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UciCommand {
    /// `uci` — identify the engine.
    Uci,
    /// `isready` — handshake.
    IsReady,
    /// `setoption name <name> [value <value>]`.
    SetOption {
        /// Option name (verbatim, case preserved).
        name: String,
        /// Option value, if present.
        value: Option<String>,
    },
    /// `ucinewgame` — reset state between games.
    UciNewGame,
    /// `position [startpos | fen <FEN>] [moves <m1> <m2> ...]`.
    Position {
        /// FEN string, or `None` for the starting position.
        fen: Option<String>,
        /// Long-algebraic moves to apply after the base position.
        moves: Vec<String>,
    },
    /// `go [...]` — start searching.
    Go(GoParams),
    /// `ponderhit` — the pondered move was played; switch to real time.
    PonderHit,
    /// `stop` — abort the current search.
    Stop,
    /// `d` — print the current board (a de-facto standard debug command).
    Display,
    /// `eval` — print the static evaluation of the current position.
    Eval,
    /// `quit` — terminate.
    Quit,
    /// Anything unrecognized (ignored, per UCI convention).
    Unknown(String),
}

/// Parses one line of UCI input into a [`UciCommand`].
pub fn parse_command(line: &str) -> UciCommand {
    let mut tokens = line.split_whitespace();
    match tokens.next() {
        Some("uci") => UciCommand::Uci,
        Some("isready") => UciCommand::IsReady,
        Some("ucinewgame") => UciCommand::UciNewGame,
        Some("ponderhit") => UciCommand::PonderHit,
        Some("stop") => UciCommand::Stop,
        Some("d") | Some("board") => UciCommand::Display,
        Some("eval") => UciCommand::Eval,
        Some("quit") => UciCommand::Quit,
        Some("setoption") => parse_setoption(&tokens.collect::<Vec<_>>()),
        Some("position") => parse_position(&tokens.collect::<Vec<_>>()),
        Some("go") => UciCommand::Go(parse_go(&tokens.collect::<Vec<_>>())),
        Some(other) => UciCommand::Unknown(other.to_string()),
        None => UciCommand::Unknown(String::new()),
    }
}

/// Parses `setoption name <name...> [value <value...>]`.
fn parse_setoption(tokens: &[&str]) -> UciCommand {
    let mut name_parts: Vec<&str> = Vec::new();
    let mut value_parts: Vec<&str> = Vec::new();
    let mut target: Option<&mut Vec<&str>> = None;
    for &token in tokens {
        match token {
            "name" => target = Some(&mut name_parts),
            "value" => target = Some(&mut value_parts),
            other => {
                if let Some(t) = target.as_mut() {
                    t.push(other);
                }
            }
        }
    }
    UciCommand::SetOption {
        name: name_parts.join(" "),
        value: if value_parts.is_empty() {
            None
        } else {
            Some(value_parts.join(" "))
        },
    }
}

/// Parses `position [startpos | fen <6 fields>] [moves ...]`.
fn parse_position(tokens: &[&str]) -> UciCommand {
    let mut fen: Option<String> = None;
    let mut moves: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "startpos" => i += 1,
            "fen" => {
                let mut fields: Vec<&str> = Vec::new();
                i += 1;
                while i < tokens.len() && tokens[i] != "moves" {
                    fields.push(tokens[i]);
                    i += 1;
                }
                fen = Some(fields.join(" "));
            }
            "moves" => {
                moves = tokens[i + 1..].iter().map(|s| s.to_string()).collect();
                break;
            }
            _ => i += 1,
        }
    }
    UciCommand::Position { fen, moves }
}

/// Parses the parameters of a `go` command.
fn parse_go(tokens: &[&str]) -> GoParams {
    let mut params = GoParams::default();
    let mut i = 0;
    while i < tokens.len() {
        let value = tokens.get(i + 1);
        let parse_u64 = || value.and_then(|v| v.parse::<u64>().ok());
        match tokens[i] {
            "depth" => {
                params.depth = value.and_then(|v| v.parse::<i32>().ok());
                i += 2;
            }
            "movetime" => {
                params.movetime = parse_u64();
                i += 2;
            }
            "nodes" => {
                params.nodes = parse_u64();
                i += 2;
            }
            "mate" => {
                params.mate = value.and_then(|v| v.parse::<i32>().ok());
                i += 2;
            }
            "wtime" => {
                params.wtime = parse_u64();
                i += 2;
            }
            "btime" => {
                params.btime = parse_u64();
                i += 2;
            }
            "winc" => {
                params.winc = parse_u64();
                i += 2;
            }
            "binc" => {
                params.binc = parse_u64();
                i += 2;
            }
            "movestogo" => {
                params.movestogo = parse_u64();
                i += 2;
            }
            "ponder" => {
                params.ponder = true;
                i += 1;
            }
            "infinite" => {
                params.infinite = true;
                i += 1;
            }
            // `searchmoves` swallows every following token that still looks
            // like a move, per the UCI spec.
            "searchmoves" => {
                i += 1;
                while i < tokens.len() && crate::terminal::parse_move_input(tokens[i]).is_some() {
                    params.search_moves.push(tokens[i].to_string());
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    params
}

/// Converts `go` parameters into [`SearchLimits`] for the given side.
///
/// Clock allocation: `remaining / movestogo.clamp(2, 25) + increment / 2`,
/// capped so at least 50 ms stays on the clock, with a 10 ms floor. The soft
/// limit is derived from the same budget, letting the engine finish early on
/// stable positions and stretch on unstable ones.
pub fn limits_from_go(params: &GoParams, side: Color) -> SearchLimits {
    let mut limits = SearchLimits {
        max_depth: params.depth.unwrap_or(MAX_DEPTH).clamp(1, MAX_DEPTH),
        move_time_ms: params.movetime,
        max_nodes: params.nodes,
        mate_in: params.mate,
        ..SearchLimits::default()
    };

    // Pondering searches the opponent's time: run until told to stop.
    if params.ponder {
        limits.move_time_ms = None;
        return limits;
    }

    if limits.move_time_ms.is_none() && !params.infinite {
        let (time, inc) = match side {
            Color::White => (params.wtime, params.winc),
            Color::Black => (params.btime, params.binc),
        };
        if let Some(remaining) = time {
            let divisor = params
                .movestogo
                .unwrap_or(CLOCK_DIVISOR)
                .clamp(2, CLOCK_DIVISOR);
            let budget = remaining / divisor + inc.unwrap_or(0) / 2;
            let capped = budget.min(remaining.saturating_sub(50)).max(MIN_BUDGET_MS);
            limits.move_time_ms = Some(capped);
        }
    }
    limits
}

/// Formats a move in pure UCI notation (`e2e4`, `e7e8q`).
pub fn move_to_uci(mv: &ChessMove) -> String {
    let mut out = format!("{}{}", mv.from.to_algebraic(), mv.to.to_algebraic());
    if let Some(promo) = mv.promotion {
        out.push(match promo {
            crate::types::PieceKind::Queen => 'q',
            crate::types::PieceKind::Rook => 'r',
            crate::types::PieceKind::Bishop => 'b',
            crate::types::PieceKind::Knight => 'n',
            _ => 'q',
        });
    }
    out
}

/// Converts an internal move display string (`e7e8=Q`) to UCI (`e7e8q`).
fn display_to_uci(display: &str) -> String {
    display.replace('=', "").to_lowercase()
}

/// Parses a UCI move string (`e2e4`, `e7e8q`) into a [`MoveJson`].
pub fn uci_to_move_json(uci: &str) -> Option<MoveJson> {
    crate::terminal::parse_move_input(uci)
}

/// Formats one `info` line from an iteration snapshot.
fn format_info_line(info: &IterationInfo) -> String {
    let score = match info.mate_in {
        Some(mate) => format!("score mate {mate}"),
        None => format!("score cp {}", info.score_cp),
    };
    let pv: Vec<String> = info.pv.iter().map(|m| display_to_uci(m)).collect();
    let mut line = format!("info depth {}", info.depth);
    if info.seldepth > info.depth {
        line.push_str(&format!(" seldepth {}", info.seldepth));
    }
    if info.multipv > 1 {
        line.push_str(&format!(" multipv {}", info.multipv));
    }
    line.push_str(&format!(
        " {} nodes {} nps {} hashfull {} time {}",
        score, info.nodes, info.nps, info.hashfull, info.elapsed_ms
    ));
    if info.tb_hits > 0 {
        line.push_str(&format!(" tbhits {}", info.tb_hits));
    }
    if !pv.is_empty() {
        line.push_str(" pv ");
        line.push_str(&pv.join(" "));
    }
    line
}

/// The mutable half of the engine configuration, driven by `setoption`.
#[derive(Debug, Clone)]
struct UciOptions {
    hash_mb: usize,
    threads: usize,
    multi_pv: usize,
    move_overhead_ms: u64,
    ponder: bool,
    own_book: bool,
    book_file: Option<PathBuf>,
    syzygy_path: Option<PathBuf>,
    limit_strength: bool,
    uci_elo: u32,
    skill_level: u8,
}

impl Default for UciOptions {
    fn default() -> Self {
        Self {
            hash_mb: DEFAULT_HASH_MB,
            threads: 1,
            multi_pv: 1,
            move_overhead_ms: 10,
            ponder: false,
            own_book: false,
            book_file: None,
            syzygy_path: None,
            limit_strength: false,
            uci_elo: MAX_UCI_ELO,
            skill_level: MAX_SKILL,
        }
    }
}

impl UciOptions {
    /// Prints the `option name …` block advertised in response to `uci`.
    fn advertise() {
        println!(
            "option name Hash type spin default {DEFAULT_HASH_MB} min {MIN_HASH_MB} max {MAX_HASH_MB}"
        );
        println!("option name Threads type spin default 1 min 1 max {MAX_THREADS}");
        println!("option name MultiPV type spin default 1 min 1 max {MAX_MULTI_PV}");
        println!("option name Move Overhead type spin default 10 min 0 max 5000");
        println!("option name Ponder type check default false");
        println!("option name OwnBook type check default false");
        println!("option name BookFile type string default <empty>");
        println!("option name SyzygyPath type string default <empty>");
        println!("option name UCI_LimitStrength type check default false");
        println!(
            "option name UCI_Elo type spin default {MAX_UCI_ELO} min {MIN_UCI_ELO} max {MAX_UCI_ELO}"
        );
        println!("option name Skill Level type spin default {MAX_SKILL} min 0 max {MAX_SKILL}");
        println!("option name Clear Hash type button");
    }

    /// Builds the engine configuration these options describe.
    ///
    /// Book and tablebase files are loaded here (and re-loaded when the path
    /// changes); failures are reported as `info string` lines, since a GUI
    /// must never be left waiting for a crashed engine.
    fn to_config(&self) -> EngineConfig {
        let book = self
            .book_file
            .as_ref()
            .filter(|_| self.own_book)
            .and_then(|path| match OpeningBook::load(path) {
                Ok(book) => {
                    println!("info string opening book loaded: {} entries", book.len());
                    Some(Arc::new(book))
                }
                Err(err) => {
                    println!("info string opening book failed to load: {err}");
                    None
                }
            });
        let tablebase =
            self.syzygy_path
                .as_ref()
                .and_then(|path| match SyzygyTablebase::load(path) {
                    Ok(tb) => {
                        println!(
                            "info string tablebase loaded: up to {} pieces",
                            tb.max_pieces
                        );
                        Some(Arc::new(tb))
                    }
                    Err(err) => {
                        println!("info string tablebase failed to load: {err}");
                        None
                    }
                });

        // `Skill Level` and `UCI_Elo` both target the same knob; the explicit
        // skill setting wins when it was lowered, otherwise the Elo mapping
        // applies whenever strength limiting is on.
        let skill_level = if self.skill_level < MAX_SKILL {
            Some(self.skill_level)
        } else if self.limit_strength {
            Some(elo_to_skill(self.uci_elo))
        } else {
            None
        };

        EngineConfig {
            tt_size_mb: self.hash_mb,
            threads: self.threads,
            multi_pv: self.multi_pv,
            move_overhead_ms: self.move_overhead_ms,
            book,
            use_book: self.own_book,
            book_variety: true,
            tablebase,
            skill_level,
        }
    }

    /// Applies one `setoption` pair. Returns `true` when the engine needs to
    /// be rebuilt (as opposed to a value the running engine can absorb).
    fn set(&mut self, name: &str, value: Option<&str>) -> bool {
        let text = value.unwrap_or("").trim();
        let as_bool = || matches!(text.to_ascii_lowercase().as_str(), "true" | "1" | "on");
        let as_usize = |fallback: usize| text.parse::<usize>().unwrap_or(fallback);

        match name.to_ascii_lowercase().as_str() {
            "hash" => {
                self.hash_mb = as_usize(self.hash_mb).clamp(MIN_HASH_MB, MAX_HASH_MB);
                true
            }
            "threads" => {
                self.threads = as_usize(self.threads).clamp(1, MAX_THREADS);
                true
            }
            "multipv" => {
                self.multi_pv = as_usize(self.multi_pv).clamp(1, MAX_MULTI_PV);
                true
            }
            "move overhead" => {
                self.move_overhead_ms = text.parse().unwrap_or(self.move_overhead_ms).min(5_000);
                true
            }
            "ponder" => {
                self.ponder = as_bool();
                false
            }
            "ownbook" => {
                self.own_book = as_bool();
                true
            }
            "bookfile" => {
                self.book_file = normalize_path(text);
                // A book path is only useful with OwnBook on; turn it on so
                // GUIs that only set the path still get a book.
                if self.book_file.is_some() {
                    self.own_book = true;
                }
                true
            }
            "syzygypath" => {
                self.syzygy_path = normalize_path(text);
                true
            }
            "uci_limitstrength" => {
                self.limit_strength = as_bool();
                true
            }
            "uci_elo" => {
                self.uci_elo = text
                    .parse()
                    .unwrap_or(self.uci_elo)
                    .clamp(MIN_UCI_ELO, MAX_UCI_ELO);
                true
            }
            "skill level" => {
                self.skill_level = text.parse().unwrap_or(self.skill_level).min(MAX_SKILL);
                true
            }
            _ => false,
        }
    }
}

/// Interprets a UCI string option value, treating the conventional
/// `<empty>` placeholder and blank input as "unset".
fn normalize_path(value: &str) -> Option<PathBuf> {
    if value.is_empty() || value == "<empty>" {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

// ---------------------------------------------------------------------------
// Protocol loop
// ---------------------------------------------------------------------------

/// Engine state owned by the UCI loop.
struct UciState {
    /// Engine instance; `None` while a search thread borrows it.
    engine: Option<SearchEngine>,
    /// Abort token of the current search (set by `stop`).
    ///
    /// Every `go` installs a *fresh* token instead of clearing this one, so a
    /// token handed to something outlasting its search — the `ponderhit`
    /// timer — can only ever stop the search it was taken from.
    abort: Arc<AtomicBool>,
    /// Running search thread, returning the engine when joined.
    search_thread: Option<JoinHandle<SearchEngine>>,
    /// Current position (game state including clocks and history).
    game: Game,
    /// Mutable option block driven by `setoption`.
    options: UciOptions,
    /// `true` while the running search is a ponder search.
    pondering: bool,
    /// Limits the ponder search should switch to on `ponderhit`.
    ponder_limits: Option<SearchLimits>,
}

impl UciState {
    fn new() -> Self {
        let abort = Arc::new(AtomicBool::new(false));
        let options = UciOptions::default();
        let mut engine = SearchEngine::with_config(options.to_config());
        engine.set_abort_token(Arc::clone(&abort));
        Self {
            engine: Some(engine),
            abort,
            search_thread: None,
            game: Game::new(),
            options,
            pondering: false,
            ponder_limits: None,
        }
    }

    /// Stops any running search and reclaims the engine instance.
    fn stop_search(&mut self) {
        self.abort.store(true, Ordering::Relaxed);
        if let Some(handle) = self.search_thread.take()
            && let Ok(engine) = handle.join()
        {
            self.engine = Some(engine);
        }
        self.pondering = false;
        self.ponder_limits = None;
    }

    /// Applies a `setoption` pair, rebuilding the engine when needed.
    fn set_option(&mut self, name: &str, value: Option<&str>) {
        if name.eq_ignore_ascii_case("clear hash") {
            self.stop_search();
            if let Some(engine) = self.engine.as_mut() {
                engine.clear_memory();
            }
            return;
        }
        if self.options.set(name, value) {
            self.stop_search();
            let config = self.options.to_config();
            match self.engine.as_mut() {
                Some(engine) => engine.set_config(config),
                None => {
                    let mut engine = SearchEngine::with_config(config);
                    engine.set_abort_token(Arc::clone(&self.abort));
                    self.engine = Some(engine);
                }
            }
        }
    }

    /// Prints the current board and FEN (`d` debug command).
    fn display(&self) {
        let renderer = BoardRenderer::new(true, false);
        print!(
            "{}",
            renderer.render(&self.game.board, &BoardHighlights::for_game(&self.game))
        );
        println!("Fen: {}", fen::game_to_fen(&self.game));
        println!(
            "Key: {:016X}",
            crate::zobrist::hash_position(
                &self.game.board,
                self.game.turn,
                &self.game.castling,
                self.game.en_passant
            )
        );
    }

    /// Prints the static evaluation of the current position (`eval`).
    fn print_eval(&self) {
        let score = crate::eval::evaluate(&self.game.board, self.game.turn);
        println!(
            "Static evaluation: {:+.2} (side to move), {:+.2} (white)",
            f64::from(score) / 100.0,
            f64::from(super::score::white_pov(score, self.game.turn)) / 100.0
        );
    }

    /// Rebuilds the current game from a `position` command.
    fn set_position(&mut self, fen_opt: Option<&str>, moves: &[String]) {
        let mut game = match fen_opt {
            Some(fen_str) => match fen::game_from_fen(fen_str) {
                Ok(g) => g,
                Err(_) => return, // ignore malformed positions, per UCI robustness
            },
            None => Game::new(),
        };
        for mv in moves {
            let Some(move_json) = uci_to_move_json(mv) else {
                break;
            };
            if game.make_move(&move_json).is_err() {
                break;
            }
        }
        self.game = game;
    }

    /// Starts a search thread for the current position.
    fn go(&mut self, params: GoParams) {
        self.stop_search();
        let Some(mut engine) = self.engine.take() else {
            return;
        };
        // A fresh token per search: the previous one may still be held by a
        // `ponderhit` timer that has not fired yet.
        self.abort = Arc::new(AtomicBool::new(false));
        engine.set_abort_token(Arc::clone(&self.abort));
        // Feed the moves already played so the search scores a line that
        // repeats an earlier game position as a draw (finds/avoids perpetuals).
        engine.set_game_history(&fen::history_hashes(&self.game));

        let mut limits = limits_from_go(&params, self.game.turn);
        // `searchmoves` restricts the root to the listed moves; unknown or
        // illegal entries are simply ignored, as the spec requires.
        if !params.search_moves.is_empty() {
            let legal = self.game.legal_moves();
            limits.search_moves = params
                .search_moves
                .iter()
                .filter_map(|token| uci_to_move_json(token))
                .filter_map(|mj| {
                    legal
                        .iter()
                        .find(|mv| {
                            mv.from.to_algebraic() == mj.from
                                && mv.to.to_algebraic() == mj.to
                                && mv.promotion.map(promotion_letter) == mj.promotion
                        })
                        .copied()
                })
                .collect();
        }

        if params.ponder {
            self.pondering = true;
            // Remember what the real budget would have been, so `ponderhit`
            // can convert the ponder search into a timed one.
            let mut real = limits.clone();
            let timed = limits_from_go(
                &GoParams {
                    ponder: false,
                    ..params.clone()
                },
                self.game.turn,
            );
            real.move_time_ms = timed.move_time_ms;
            self.ponder_limits = Some(real);
        }

        let pos = fen::search_position(&self.game);

        // Robustness for match play: if the search is aborted (`stop`/`quit`)
        // before it completes even one iteration, we must still answer with a
        // *legal* move — many match runners treat `bestmove 0000` as an illegal
        // move and forfeit the game. Pre-compute a safe fallback here.
        let fallback = self
            .game
            .legal_moves()
            .first()
            .map(move_to_uci)
            .unwrap_or_else(|| "0000".to_string());

        self.search_thread = Some(std::thread::spawn(move || {
            let mut on_iteration = |info: &IterationInfo| {
                println!("{}", format_info_line(info));
            };
            let result = engine.search_limited(&pos, &limits, Some(&mut on_iteration));
            let best = result
                .best_move
                .map(|mv| move_to_uci(&mv))
                .unwrap_or(fallback);
            // The second PV move is the expected reply — hand it to the GUI so
            // it can start a ponder search.
            match result.pv.get(1) {
                Some(ponder) => println!("bestmove {best} ponder {}", move_to_uci(ponder)),
                None => println!("bestmove {best}"),
            }
            engine
        }));
    }

    /// Handles `ponderhit`: the pondered move was played, so the search that
    /// is already running becomes the real one and needs a deadline.
    ///
    /// The running search has no deadline, so the simplest correct handling
    /// is to let it finish naturally when the GUI sends `stop`; when a budget
    /// is known we schedule the stop ourselves.
    ///
    /// The timer cannot be cancelled, so it may well outlive the search it was
    /// scheduled for — the pondered search often finishes on its own first.
    /// It therefore captures *this* search's abort token: by the time a stale
    /// timer fires, `go` has installed a new token for the next search and the
    /// store lands harmlessly on the finished one.
    fn ponder_hit(&mut self) {
        if !self.pondering {
            return;
        }
        self.pondering = false;
        let Some(budget) = self
            .ponder_limits
            .take()
            .and_then(|limits| limits.move_time_ms)
        else {
            return;
        };
        let abort = Arc::clone(&self.abort);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(budget));
            abort.store(true, Ordering::Relaxed);
        });
    }

    /// Clears search state between games.
    fn new_game(&mut self) {
        self.stop_search();
        if let Some(engine) = self.engine.as_mut() {
            engine.clear_memory();
        }
        self.game = Game::new();
    }
}

/// The single-letter promotion code used by [`MoveJson`].
fn promotion_letter(kind: crate::types::PieceKind) -> String {
    match kind {
        crate::types::PieceKind::Queen => "Q",
        crate::types::PieceKind::Rook => "R",
        crate::types::PieceKind::Bishop => "B",
        crate::types::PieceKind::Knight => "N",
        _ => "Q",
    }
    .to_string()
}

/// Runs the blocking UCI read-eval loop until `quit` or EOF.
fn run_uci_loop() {
    use std::io::BufRead;

    let mut state = UciState::new();
    let stdin = std::io::stdin();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        match parse_command(&line) {
            UciCommand::Uci => {
                println!("id name CheckAI {}", crate::update::version());
                println!("id author JosunLP and contributors");
                UciOptions::advertise();
                println!("uciok");
            }
            UciCommand::IsReady => println!("readyok"),
            UciCommand::SetOption { name, value } => {
                state.set_option(&name, value.as_deref());
            }
            UciCommand::UciNewGame => state.new_game(),
            UciCommand::Position { fen, moves } => {
                state.set_position(fen.as_deref(), &moves);
            }
            UciCommand::Go(params) => state.go(params),
            UciCommand::PonderHit => state.ponder_hit(),
            UciCommand::Stop => state.stop_search(),
            UciCommand::Display => state.display(),
            UciCommand::Eval => state.print_eval(),
            UciCommand::Quit => break,
            UciCommand::Unknown(_) => {} // silently ignore, per UCI convention
        }
    }
    state.stop_search();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_commands() {
        assert_eq!(parse_command("uci"), UciCommand::Uci);
        assert_eq!(parse_command("isready"), UciCommand::IsReady);
        assert_eq!(parse_command("ucinewgame"), UciCommand::UciNewGame);
        assert_eq!(parse_command("stop"), UciCommand::Stop);
        assert_eq!(parse_command("quit"), UciCommand::Quit);
        assert!(matches!(parse_command("banana"), UciCommand::Unknown(_)));
    }

    #[test]
    fn test_parse_position_startpos_with_moves() {
        let cmd = parse_command("position startpos moves e2e4 e7e5");
        assert_eq!(
            cmd,
            UciCommand::Position {
                fen: None,
                moves: vec!["e2e4".to_string(), "e7e5".to_string()],
            }
        );
    }

    #[test]
    fn test_parse_position_fen() {
        let cmd =
            parse_command("position fen 8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1 moves b4b1");
        assert_eq!(
            cmd,
            UciCommand::Position {
                fen: Some("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1".to_string()),
                moves: vec!["b4b1".to_string()],
            }
        );
    }

    #[test]
    fn test_parse_setoption_hash() {
        let cmd = parse_command("setoption name Hash value 128");
        assert_eq!(
            cmd,
            UciCommand::SetOption {
                name: "Hash".to_string(),
                value: Some("128".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_go_parameters() {
        let params = match parse_command("go depth 9 nodes 5000") {
            UciCommand::Go(p) => p,
            other => panic!("expected go, got {other:?}"),
        };
        assert_eq!(params.depth, Some(9));
        assert_eq!(params.nodes, Some(5000));
        assert!(!params.infinite);

        let params = match parse_command("go wtime 60000 btime 50000 winc 1000 binc 900") {
            UciCommand::Go(p) => p,
            other => panic!("expected go, got {other:?}"),
        };
        assert_eq!(params.wtime, Some(60_000));
        assert_eq!(params.binc, Some(900));
    }

    #[test]
    fn test_limits_from_clock() {
        let params = GoParams {
            wtime: Some(60_000),
            winc: Some(1_000),
            ..GoParams::default()
        };
        let limits = limits_from_go(&params, Color::White);
        // 60000/25 + 1000/2 = 2900 ms
        assert_eq!(limits.move_time_ms, Some(2_900));
        assert_eq!(limits.max_depth, MAX_DEPTH);

        // Black uses btime/binc.
        let params = GoParams {
            wtime: Some(60_000),
            btime: Some(10_000),
            binc: Some(500),
            ..GoParams::default()
        };
        let limits = limits_from_go(&params, Color::Black);
        assert_eq!(limits.move_time_ms, Some(10_000 / 25 + 250));
    }

    #[test]
    fn test_limits_movetime_takes_priority() {
        let params = GoParams {
            movetime: Some(750),
            wtime: Some(60_000),
            ..GoParams::default()
        };
        let limits = limits_from_go(&params, Color::White);
        assert_eq!(limits.move_time_ms, Some(750));
    }

    #[test]
    fn test_limits_infinite_has_no_time() {
        let params = GoParams {
            infinite: true,
            wtime: Some(60_000),
            ..GoParams::default()
        };
        let limits = limits_from_go(&params, Color::White);
        assert_eq!(limits.move_time_ms, None);
        assert_eq!(limits.max_depth, MAX_DEPTH);
    }

    #[test]
    fn test_move_to_uci_promotion() {
        use crate::types::{PieceKind, Square};
        let mv = ChessMove {
            from: Square::from_algebraic("e7").unwrap(),
            to: Square::from_algebraic("e8").unwrap(),
            promotion: Some(PieceKind::Queen),
            is_castling: false,
            is_en_passant: false,
        };
        assert_eq!(move_to_uci(&mv), "e7e8q");
        assert_eq!(display_to_uci("e7e8=Q"), "e7e8q");
    }

    #[test]
    fn test_uci_to_move_json() {
        let mj = uci_to_move_json("e7e8q").unwrap();
        assert_eq!(mj.from, "e7");
        assert_eq!(mj.to, "e8");
        assert_eq!(mj.promotion, Some("Q".to_string()));
        assert!(uci_to_move_json("nonsense").is_none());
    }

    #[test]
    fn test_format_info_line() {
        let info = IterationInfo {
            depth: 7,
            score_cp: -42,
            nodes: 123_456,
            elapsed_ms: 250,
            nps: 493_824,
            pv: vec!["e2e4".to_string(), "e7e8=Q".to_string()],
            ..IterationInfo::default()
        };
        assert_eq!(
            format_info_line(&info),
            "info depth 7 score cp -42 nodes 123456 nps 493824 hashfull 0 time 250 pv e2e4 e7e8q"
        );

        let mate = IterationInfo {
            mate_in: Some(-2),
            ..info
        };
        assert!(format_info_line(&mate).contains("score mate -2"));
    }

    #[test]
    fn test_format_info_line_reports_seldepth_and_multipv() {
        let info = IterationInfo {
            depth: 10,
            seldepth: 22,
            multipv: 3,
            score_cp: 15,
            nodes: 1_000,
            nps: 5_000,
            elapsed_ms: 200,
            hashfull: 314,
            tb_hits: 2,
            pv: vec!["e2e4".to_string()],
            ..IterationInfo::default()
        };
        let line = format_info_line(&info);
        assert!(line.contains("seldepth 22"));
        assert!(line.contains("multipv 3"));
        assert!(line.contains("hashfull 314"));
        assert!(line.contains("tbhits 2"));
    }

    #[test]
    fn test_parse_new_commands() {
        assert_eq!(parse_command("ponderhit"), UciCommand::PonderHit);
        assert_eq!(parse_command("d"), UciCommand::Display);
        assert_eq!(parse_command("eval"), UciCommand::Eval);
    }

    #[test]
    fn test_parse_go_searchmoves_and_mate() {
        let params = match parse_command("go searchmoves e2e4 d2d4 mate 3") {
            UciCommand::Go(p) => p,
            other => panic!("expected go, got {other:?}"),
        };
        assert_eq!(params.search_moves, vec!["e2e4", "d2d4"]);
        assert_eq!(params.mate, Some(3));
    }

    #[test]
    fn test_ponder_search_has_no_deadline() {
        let params = GoParams {
            ponder: true,
            wtime: Some(60_000),
            ..GoParams::default()
        };
        assert_eq!(limits_from_go(&params, Color::White).move_time_ms, None);
    }

    #[test]
    fn test_options_round_trip() {
        let mut options = UciOptions::default();
        assert!(options.set("Threads", Some("8")));
        assert_eq!(options.threads, 8);
        assert!(options.set("MultiPV", Some("4")));
        assert_eq!(options.multi_pv, 4);
        assert!(options.set("Hash", Some("999999")));
        assert_eq!(options.hash_mb, MAX_HASH_MB);
        assert!(
            !options.set("Ponder", Some("true")),
            "Ponder needs no rebuild"
        );
        assert!(options.ponder);
        // Unknown options are ignored without requesting a rebuild.
        assert!(!options.set("Nonexistent", Some("1")));

        let config = options.to_config();
        assert_eq!(config.threads, 8);
        assert_eq!(config.multi_pv, 4);
        assert!(config.skill_level.is_none(), "full strength by default");
    }

    #[test]
    fn test_book_file_enables_own_book() {
        let mut options = UciOptions::default();
        options.set("BookFile", Some("/tmp/book.bin"));
        assert!(options.own_book);
        assert_eq!(options.book_file, Some(PathBuf::from("/tmp/book.bin")));
        // The conventional empty placeholder clears the path again.
        options.set("BookFile", Some("<empty>"));
        assert_eq!(options.book_file, None);
    }

    #[test]
    fn test_strength_limiting_maps_elo_to_skill() {
        assert_eq!(elo_to_skill(MAX_UCI_ELO), MAX_SKILL);
        assert_eq!(elo_to_skill(MIN_UCI_ELO), 0);
        assert_eq!(elo_to_skill(100), 0, "below the range clamps to weakest");
        let mid = elo_to_skill((MIN_UCI_ELO + MAX_UCI_ELO) / 2);
        assert!((9..=11).contains(&mid), "got {mid}");

        let mut options = UciOptions::default();
        options.set("UCI_LimitStrength", Some("true"));
        options.set("UCI_Elo", Some("1500"));
        assert_eq!(
            options.to_config().skill_level,
            Some(elo_to_skill(1500)),
            "limiting strength must reach the engine config"
        );
    }

    #[test]
    fn test_skill_level_option_takes_precedence() {
        let mut options = UciOptions::default();
        options.set("Skill Level", Some("5"));
        assert_eq!(options.to_config().skill_level, Some(5));
    }

    /// The `ponderhit` timer holds a clone of the abort token and cannot be
    /// cancelled. If `go` reused one shared token, a timer left over from a
    /// pondered search that already finished would stop the *next* search
    /// after a handful of nodes.
    #[test]
    fn test_a_stale_ponder_timer_cannot_stop_the_next_search() {
        let mut state = UciState::new();
        // What a `ponderhit` timer would have captured for the ponder search.
        let pondered = Arc::clone(&state.abort);

        state.go(GoParams {
            depth: Some(1),
            ..GoParams::default()
        });
        let running = Arc::clone(&state.abort);

        assert!(
            !Arc::ptr_eq(&pondered, &running),
            "every search needs its own abort token"
        );

        // The stale timer fires late: it must not reach the running search.
        pondered.store(true, Ordering::Relaxed);
        assert!(
            !running.load(Ordering::Relaxed),
            "a superseded timer stopped the current search"
        );

        state.stop_search();
        assert!(
            running.load(Ordering::Relaxed),
            "`stop` must still reach the current search"
        );
    }
}
