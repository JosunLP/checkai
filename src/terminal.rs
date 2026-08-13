//! Terminal input toolkit for interactive CLI sessions.
//!
//! Provides the building blocks shared by the interactive game modes
//! (`checkai play`): line-based input with EOF handling, parsing of
//! coordinate moves (`e2e4`, `e7e8Q`) and of the in-game REPL commands
//! with their single-letter aliases.
//!
//! Rendering lives in [`crate::cli::board_renderer`]; this module is
//! strictly about reading and interpreting user input. No raw mode is
//! ever enabled, so Ctrl+C and terminal state need no special cleanup.

use std::io::{self, BufRead, Write};

use crate::types::{MoveJson, Square};

/// A parsed in-game REPL command.
#[derive(Debug, Clone)]
pub enum GameCommand {
    /// A coordinate move such as `e2e4` or `e7e8q`.
    Move(MoveJson),
    /// A move written in standard algebraic notation (`Nf3`, `exd5`, `O-O`).
    ///
    /// SAN is position-dependent, so it is resolved by the caller rather
    /// than here; the raw token is carried through.
    San(String),
    /// List all legal moves.
    Moves,
    /// Re-render the board.
    Board,
    /// Show the numbered move history.
    History,
    /// Print the current FEN string.
    Fen,
    /// Print the game state as JSON.
    Json,
    /// Print the game as PGN.
    Pgn,
    /// Ask the engine for a move suggestion.
    Hint,
    /// Run a deeper multi-line analysis of the current position.
    Analyze,
    /// Show the static evaluation breakdown.
    Eval,
    /// Show the opening-book entries for the current position.
    Book,
    /// Show the endgame tablebase verdict for the current position.
    Tablebase,
    /// Take back the last full move.
    Undo,
    /// Replay a move that was taken back.
    Redo,
    /// Flip the board orientation.
    Flip,
    /// Start a fresh game.
    New,
    /// Change the engine difficulty level (`level 8`).
    Level(u8),
    /// Save the game to a PGN file (`save game.pgn`).
    Save(Option<String>),
    /// Load a game from a PGN file (`load game.pgn`).
    Load(String),
    /// Resign the game.
    Resign,
    /// Claim a draw (if eligible).
    Draw,
    /// Show the in-game help table.
    Help,
    /// Quit the session.
    Quit,
}

impl PartialEq for GameCommand {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // MoveJson does not derive PartialEq, so compare fields.
            (Self::Move(a), Self::Move(b)) => {
                a.from == b.from && a.to == b.to && a.promotion == b.promotion
            }
            (Self::San(a), Self::San(b)) => a == b,
            (Self::Level(a), Self::Level(b)) => a == b,
            (Self::Save(a), Self::Save(b)) => a == b,
            (Self::Load(a), Self::Load(b)) => a == b,
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
        }
    }
}

impl Eq for GameCommand {}

impl GameCommand {
    /// Parses user input into a command.
    ///
    /// Keyword commands and their aliases are matched case-insensitively;
    /// anything else is interpreted as a move — first as coordinate notation
    /// (`e2e4`), then as SAN (`Nf3`), which the caller resolves against the
    /// current position. Returns `None` for input that cannot be either.
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        let mut parts = trimmed.split_whitespace();
        let head = parts.next()?.to_lowercase();
        let argument = parts.next().map(str::to_string);

        match head.as_str() {
            "moves" | "m" => return Some(Self::Moves),
            "board" | "b" => return Some(Self::Board),
            "history" | "hist" => return Some(Self::History),
            "fen" | "f" => return Some(Self::Fen),
            "json" | "j" => return Some(Self::Json),
            "pgn" => return Some(Self::Pgn),
            "hint" | "i" => return Some(Self::Hint),
            "analyze" | "analyse" | "a" => return Some(Self::Analyze),
            "eval" | "e" => return Some(Self::Eval),
            "book" => return Some(Self::Book),
            "tb" | "tablebase" => return Some(Self::Tablebase),
            "undo" | "u" => return Some(Self::Undo),
            "redo" => return Some(Self::Redo),
            "flip" => return Some(Self::Flip),
            "new" => return Some(Self::New),
            "level" | "l" => {
                return argument.and_then(|v| v.parse::<u8>().ok()).map(Self::Level);
            }
            "save" | "s" => return Some(Self::Save(argument)),
            "load" | "o" => return argument.map(Self::Load),
            "resign" | "r" => return Some(Self::Resign),
            "draw" | "d" => return Some(Self::Draw),
            "help" | "h" | "?" => return Some(Self::Help),
            "quit" | "exit" | "q" => return Some(Self::Quit),
            _ => {}
        }

        // Not a keyword: a move, in one notation or the other.
        if let Some(mv) = parse_move_input(&trimmed.to_lowercase()) {
            return Some(Self::Move(mv));
        }
        if looks_like_san(trimmed) {
            return Some(Self::San(trimmed.to_string()));
        }
        None
    }
}

/// Cheap syntactic filter for SAN-looking input.
///
/// The real check happens when the token is resolved against the position;
/// this only keeps obvious nonsense out of the SAN path so unknown commands
/// still produce a "unknown command" message.
fn looks_like_san(token: &str) -> bool {
    let cleaned = token.replace('0', "O");
    if cleaned == "O-O" || cleaned == "O-O-O" {
        return true;
    }
    let body: String = token
        .chars()
        .filter(|c| !matches!(c, 'x' | '+' | '#' | '=' | '!' | '?' | '-'))
        .collect();
    if body.len() < 2 || body.len() > 6 || !body.is_ascii() {
        return false;
    }
    // Must contain a destination square: a file letter followed by a rank.
    body.as_bytes()
        .windows(2)
        .any(|pair| pair[0].is_ascii_lowercase() && pair[1].is_ascii_digit())
        && body.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Prints `prompt`, flushes stdout, and reads one line from stdin.
///
/// Returns `None` on EOF or a read error so callers can terminate
/// cleanly instead of spinning on an exhausted pipe.
pub fn read_input_line(prompt: &str) -> Option<String> {
    print!("{prompt}");
    io::stdout().flush().ok();

    let mut input = String::new();
    match io::stdin().lock().read_line(&mut input) {
        Ok(0) => None, // EOF
        Ok(_) => Some(input.trim().to_string()),
        Err(_) => None,
    }
}

/// Parses a coordinate move like `e2e4` or `e7e8Q` into a [`MoveJson`].
///
/// Accepted formats:
/// - `e2e4` — normal move
/// - `e7e8Q` — promotion (Q, R, B, N — case-insensitive)
/// - `e2 e4` — with space separator
///
/// Non-ASCII input is rejected up front so slicing can never panic.
pub fn parse_move_input(input: &str) -> Option<MoveJson> {
    let input = input.replace(' ', "");
    let input = input.trim();

    if !input.is_ascii() || input.len() < 4 || input.len() > 5 {
        return None;
    }

    let from = &input[0..2];
    let to = &input[2..4];

    // Validate squares
    if Square::from_algebraic(from).is_none() || Square::from_algebraic(to).is_none() {
        return None;
    }

    let promotion = if input.len() == 5 {
        let promo_char = input.chars().nth(4)?.to_ascii_uppercase();
        match promo_char {
            'Q' | 'R' | 'B' | 'N' => Some(promo_char.to_string()),
            _ => return None,
        }
    } else {
        None
    };

    Some(MoveJson {
        from: from.to_string(),
        to: to.to_string(),
        promotion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_move_normal() {
        let m = parse_move_input("e2e4").unwrap();
        assert_eq!(m.from, "e2");
        assert_eq!(m.to, "e4");
        assert_eq!(m.promotion, None);
    }

    #[test]
    fn test_parse_move_promotion() {
        let m = parse_move_input("e7e8q").unwrap();
        assert_eq!(m.from, "e7");
        assert_eq!(m.to, "e8");
        assert_eq!(m.promotion, Some("Q".to_string()));
    }

    #[test]
    fn test_parse_move_with_space() {
        let m = parse_move_input("e2 e4").unwrap();
        assert_eq!(m.from, "e2");
        assert_eq!(m.to, "e4");
    }

    #[test]
    fn test_parse_invalid() {
        assert!(parse_move_input("abc").is_none());
        assert!(parse_move_input("z9z9").is_none());
        assert!(parse_move_input("e2e4x").is_none());
    }

    #[test]
    fn test_parse_multibyte_input_does_not_panic() {
        // 4–5 *bytes* of multi-byte UTF-8 used to panic on byte slicing.
        assert!(parse_move_input("♔♕").is_none());
        assert!(parse_move_input("éé").is_none());
    }

    #[test]
    fn test_command_keywords_and_aliases() {
        assert_eq!(GameCommand::parse("quit"), Some(GameCommand::Quit));
        assert_eq!(GameCommand::parse("Q"), Some(GameCommand::Quit));
        assert_eq!(GameCommand::parse("HELP"), Some(GameCommand::Help));
        assert_eq!(GameCommand::parse("?"), Some(GameCommand::Help));
        assert_eq!(GameCommand::parse("hint"), Some(GameCommand::Hint));
        assert_eq!(GameCommand::parse("i"), Some(GameCommand::Hint));
        assert_eq!(GameCommand::parse("u"), Some(GameCommand::Undo));
        assert_eq!(GameCommand::parse("hist"), Some(GameCommand::History));
        assert_eq!(GameCommand::parse("d"), Some(GameCommand::Draw));
    }

    #[test]
    fn test_command_move_fallback() {
        match GameCommand::parse("E2E4") {
            Some(GameCommand::Move(m)) => {
                assert_eq!(m.from, "e2");
                assert_eq!(m.to, "e4");
            }
            other => panic!("expected move, got {other:?}"),
        }
        assert_eq!(GameCommand::parse("xyzzy"), None);
    }

    #[test]
    fn test_command_san_fallback() {
        assert_eq!(
            GameCommand::parse("Nf3"),
            Some(GameCommand::San("Nf3".into()))
        );
        assert_eq!(
            GameCommand::parse("exd5"),
            Some(GameCommand::San("exd5".into()))
        );
        assert_eq!(
            GameCommand::parse("O-O"),
            Some(GameCommand::San("O-O".into()))
        );
        assert_eq!(
            GameCommand::parse("Qh4#"),
            Some(GameCommand::San("Qh4#".into()))
        );
        // Keywords still win over the SAN reading.
        assert_eq!(GameCommand::parse("board"), Some(GameCommand::Board));
        // And clear nonsense is still rejected.
        assert_eq!(GameCommand::parse("zzzzzzzz"), None);
        assert_eq!(GameCommand::parse("42"), None);
    }

    #[test]
    fn test_command_arguments() {
        assert_eq!(GameCommand::parse("level 8"), Some(GameCommand::Level(8)));
        assert_eq!(GameCommand::parse("level"), None);
        assert_eq!(
            GameCommand::parse("save my.pgn"),
            Some(GameCommand::Save(Some("my.pgn".into())))
        );
        assert_eq!(GameCommand::parse("save"), Some(GameCommand::Save(None)));
        assert_eq!(
            GameCommand::parse("load my.pgn"),
            Some(GameCommand::Load("my.pgn".into()))
        );
        assert_eq!(GameCommand::parse("load"), None);
    }

    #[test]
    fn test_new_display_commands() {
        assert_eq!(GameCommand::parse("pgn"), Some(GameCommand::Pgn));
        assert_eq!(GameCommand::parse("eval"), Some(GameCommand::Eval));
        assert_eq!(GameCommand::parse("a"), Some(GameCommand::Analyze));
        assert_eq!(GameCommand::parse("flip"), Some(GameCommand::Flip));
        assert_eq!(GameCommand::parse("redo"), Some(GameCommand::Redo));
        assert_eq!(GameCommand::parse("tb"), Some(GameCommand::Tablebase));
    }
}
