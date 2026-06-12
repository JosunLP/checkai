//! `checkai uci` — the Universal Chess Interface protocol.
//!
//! Speaks plain UCI on stdin/stdout so CheckAI can be plugged into any
//! chess GUI or match runner (cutechess-cli, fastchess, Arena, …).
//!
//! Supported commands: `uci`, `isready`, `setoption name Hash value N`,
//! `ucinewgame`, `position [startpos | fen <FEN>] [moves ...]`,
//! `go [depth N | movetime MS | nodes N | wtime/btime/winc/binc/movestogo
//! | infinite]`, `stop`, `quit`.
//!
//! Searches run on a dedicated `std::thread` with the engine's abort
//! token wired to `stop`, emitting `info` lines from the iterative-
//! deepening callback and a final `bestmove`.
//!
//! This module is machine-facing: it deliberately bypasses i18n and
//! colors — output is pure protocol text.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use clap::Args;

use super::fen;
use super::{CliCommand, CliContext, CliResult};
use crate::game::Game;
use crate::search::{IterationInfo, MAX_DEPTH, SearchEngine, SearchLimits};
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

/// Arguments for `checkai uci` (none — pure stdio protocol).
#[derive(Args, Debug)]
#[command(after_help = "\
Example session:\n\
  $ checkai uci\n\
  uci\n\
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
    /// `stop` — abort the current search.
    Stop,
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
        Some("stop") => UciCommand::Stop,
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
            "infinite" => {
                params.infinite = true;
                i += 1;
            }
            _ => i += 1,
        }
    }
    params
}

/// Converts `go` parameters into [`SearchLimits`] for the given side.
///
/// Clock allocation: `remaining / movestogo.clamp(2, 25) + increment / 2`,
/// capped so at least 50 ms stays on the clock, with a 10 ms floor.
pub fn limits_from_go(params: &GoParams, side: Color) -> SearchLimits {
    let mut limits = SearchLimits {
        max_depth: params.depth.unwrap_or(MAX_DEPTH).clamp(1, MAX_DEPTH),
        move_time_ms: params.movetime,
        max_nodes: params.nodes,
    };

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
    let mut line = format!(
        "info depth {} {} nodes {} nps {} time {}",
        info.depth, score, info.nodes, info.nps, info.elapsed_ms
    );
    if !pv.is_empty() {
        line.push_str(" pv ");
        line.push_str(&pv.join(" "));
    }
    line
}

// ---------------------------------------------------------------------------
// Protocol loop
// ---------------------------------------------------------------------------

/// Engine state owned by the UCI loop.
struct UciState {
    /// Engine instance; `None` while a search thread borrows it.
    engine: Option<SearchEngine>,
    /// Shared abort token wired into the engine (set by `stop`).
    abort: Arc<AtomicBool>,
    /// Running search thread, returning the engine when joined.
    search_thread: Option<JoinHandle<SearchEngine>>,
    /// Current position (game state including clocks and history).
    game: Game,
    /// Configured hash size in MB.
    hash_mb: usize,
}

impl UciState {
    fn new() -> Self {
        let abort = Arc::new(AtomicBool::new(false));
        let mut engine = SearchEngine::new(DEFAULT_HASH_MB);
        engine.set_abort_token(Arc::clone(&abort));
        Self {
            engine: Some(engine),
            abort,
            search_thread: None,
            game: Game::new(),
            hash_mb: DEFAULT_HASH_MB,
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
    }

    /// Applies `setoption` (only `Hash` is supported).
    fn set_option(&mut self, name: &str, value: Option<&str>) {
        if name.eq_ignore_ascii_case("hash")
            && let Some(mb) = value.and_then(|v| v.parse::<usize>().ok())
        {
            self.stop_search();
            self.hash_mb = mb.clamp(MIN_HASH_MB, MAX_HASH_MB);
            let mut engine = SearchEngine::new(self.hash_mb);
            engine.set_abort_token(Arc::clone(&self.abort));
            self.engine = Some(engine);
        }
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
        engine.reset_abort();
        // Feed the moves already played so the search scores a line that
        // repeats an earlier game position as a draw (finds/avoids perpetuals).
        engine.set_game_history(&fen::history_hashes(&self.game));

        let limits = limits_from_go(&params, self.game.turn);
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
            println!("bestmove {best}");
            engine
        }));
    }

    /// Clears search state between games.
    fn new_game(&mut self) {
        self.stop_search();
        if let Some(engine) = self.engine.as_mut() {
            engine.tt.clear();
        }
        self.game = Game::new();
    }
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
                println!(
                    "option name Hash type spin default {DEFAULT_HASH_MB} min {MIN_HASH_MB} max {MAX_HASH_MB}"
                );
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
            UciCommand::Stop => state.stop_search(),
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
            mate_in: None,
            nodes: 123_456,
            elapsed_ms: 250,
            nps: 493_824,
            pv: vec!["e2e4".to_string(), "e7e8=Q".to_string()],
        };
        assert_eq!(
            format_info_line(&info),
            "info depth 7 score cp -42 nodes 123456 nps 493824 time 250 pv e2e4 e7e8q"
        );

        let mate = IterationInfo {
            mate_in: Some(-2),
            ..info
        };
        assert!(format_info_line(&mate).contains("score mate -2"));
    }
}
