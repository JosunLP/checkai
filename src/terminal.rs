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
    /// Ask the engine for a move suggestion.
    Hint,
    /// Take back the last full move.
    Undo,
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
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
        }
    }
}

impl Eq for GameCommand {}

impl GameCommand {
    /// Parses user input (case-insensitive) into a command.
    ///
    /// Keyword commands and their single-letter aliases are tried first;
    /// anything else is interpreted as a coordinate move. Returns `None`
    /// for unrecognized input.
    pub fn parse(input: &str) -> Option<Self> {
        let normalized = input.trim().to_lowercase();
        match normalized.as_str() {
            "moves" | "m" => Some(Self::Moves),
            "board" | "b" => Some(Self::Board),
            "history" | "hist" => Some(Self::History),
            "fen" | "f" => Some(Self::Fen),
            "json" | "j" => Some(Self::Json),
            "hint" | "i" => Some(Self::Hint),
            "undo" | "u" => Some(Self::Undo),
            "resign" | "r" => Some(Self::Resign),
            "draw" | "d" => Some(Self::Draw),
            "help" | "h" | "?" => Some(Self::Help),
            "quit" | "exit" | "q" => Some(Self::Quit),
            _ => parse_move_input(&normalized).map(Self::Move),
        }
    }
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
}
