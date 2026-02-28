//! Terminal interface for the CheckAI chess engine.
//!
//! This module provides a command-line interface for playing chess
//! directly in the terminal. It supports:
//!
//! - Colored board display with Unicode pieces
//! - Interactive move input (algebraic notation)
//! - Game state display (check, castling rights, move history)
//! - Draw claims and resignation
//! - Two-player mode (human vs human)

use colored::Colorize;
use std::io::{self, Write};

use crate::game::Game;
use crate::movegen;
use crate::types::*;

/// Renders the board to the terminal with colors and piece symbols.
///
/// The board is displayed from White's perspective (rank 8 at top).
/// Dark squares are shown with a dark background, light squares with light.
/// Pieces are colored based on their side (White/Black).
pub fn print_board(game: &Game) {
    println!();
    println!("  +---+---+---+---+---+---+---+---+");

    for rank in (0..8u8).rev() {
        print!("{} ", rank + 1);
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            let is_dark_square = (file + rank) % 2 == 0;

            let piece_str = match game.board.get(sq) {
                Some(piece) => {
                    let symbol = piece_to_unicode(piece);
                    if piece.color == Color::White {
                        symbol.white().bold().to_string()
                    } else {
                        symbol.blue().bold().to_string()
                    }
                }
                None => {
                    if is_dark_square {
                        "·".dimmed().to_string()
                    } else {
                        " ".to_string()
                    }
                }
            };

            print!("| {} ", piece_str);
        }
        println!("|");
        println!("  +---+---+---+---+---+---+---+---+");
    }
    println!("    a   b   c   d   e   f   g   h");
    println!();
}

/// Converts a piece to its Unicode chess symbol.
fn piece_to_unicode(piece: Piece) -> &'static str {
    match (piece.color, piece.kind) {
        (Color::White, PieceKind::King) => "K",
        (Color::White, PieceKind::Queen) => "Q",
        (Color::White, PieceKind::Rook) => "R",
        (Color::White, PieceKind::Bishop) => "B",
        (Color::White, PieceKind::Knight) => "N",
        (Color::White, PieceKind::Pawn) => "P",
        (Color::Black, PieceKind::King) => "k",
        (Color::Black, PieceKind::Queen) => "q",
        (Color::Black, PieceKind::Rook) => "r",
        (Color::Black, PieceKind::Bishop) => "b",
        (Color::Black, PieceKind::Knight) => "n",
        (Color::Black, PieceKind::Pawn) => "p",
    }
}

/// Prints the game status bar (turn, check, move number, etc.).
pub fn print_status(game: &Game) {
    let turn_str = match game.turn {
        Color::White => "White".white().bold(),
        Color::Black => "Black".blue().bold(),
    };

    let is_check = movegen::is_in_check(&game.board, game.turn);
    let legal_moves = game.legal_moves();

    print!(
        "{}",
        t!("terminal.move_status", num = game.fullmove_number, color = turn_str),
    );

    if is_check {
        print!("  {}", t!("terminal.check").to_string().red().bold());
    }

    println!(
        "  {}",
        t!("terminal.legal_moves_count", count = legal_moves.len())
    );

    // Castling rights
    let wk = if game.castling.white.kingside { "K" } else { "-" };
    let wq = if game.castling.white.queenside { "Q" } else { "-" };
    let bk = if game.castling.black.kingside { "k" } else { "-" };
    let bq = if game.castling.black.queenside { "q" } else { "-" };
    let rights = format!("{}{}{}{}", wk, wq, bk, bq);
    println!(
        "{}",
        t!("terminal.castling_info", rights = &rights, clock = game.halfmove_clock)
    );

    if let Some(ep) = game.en_passant {
        println!(
            "{}",
            t!("terminal.en_passant_info", square = ep.to_algebraic())
        );
    }

    println!();
}

/// Prints the game result when the game ends.
pub fn print_game_result(game: &Game) {
    if let (Some(result), Some(reason)) = (&game.result, &game.end_reason) {
        println!();
        println!("{}", "═══════════════════════════════════".yellow());
        println!("  {} — {}", t!("terminal.game_over_label").to_string().yellow().bold(), reason);
        println!(
            "{}",
            t!("terminal.result_label", result = result.to_string().green().bold())
        );
        println!("{}", "═══════════════════════════════════".yellow());
        println!();
    }
}

/// Prints available commands in the terminal.
pub fn print_help() {
    println!("{}", t!("terminal.cmd_header").to_string().yellow().bold());
    println!("  {}      - {}", "e2e4".green(), t!("terminal.cmd_move"));
    println!("  {}     - {}", "moves".green(), t!("terminal.cmd_moves"));
    println!("  {}      - {}", "board".green(), t!("terminal.cmd_board"));
    println!("  {}    - {}", "resign".green(), t!("terminal.cmd_resign"));
    println!("  {}      - {}", "draw".green(), t!("terminal.cmd_draw"));
    println!("  {}   - {}", "history".green(), t!("terminal.cmd_history"));
    println!("  {}       - {}", "json".green(), t!("terminal.cmd_json"));
    println!("  {}      - {}", "help".green(), t!("terminal.cmd_help"));
    println!("  {}      - {}", "quit".green(), t!("terminal.cmd_quit"));
    println!();
}

/// Prints the move history.
pub fn print_history(game: &Game) {
    if game.move_history.is_empty() {
        println!("{}", t!("terminal.no_moves_yet"));
        return;
    }

    println!("{}", t!("terminal.move_history_label").to_string().yellow().bold());
    for (i, record) in game.move_history.iter().enumerate() {
        let side = match record.side {
            Color::White => "White",
            Color::Black => "Black",
        };
        println!(
            "  {}. {} {}",
            i + 1,
            side,
            record.notation
        );
    }
    println!();
}

/// Runs the interactive terminal chess game.
///
/// Two players alternate entering moves via the terminal.
/// The game continues until checkmate, stalemate, draw, or resignation.
pub fn run_terminal_game() {
    println!();
    println!("{}", "╔═══════════════════════════════════════╗".cyan());
    println!("{}", format!("\u{2551}     {}     \u{2551}", t!("terminal.banner_title")).cyan());
    println!("{}", format!("\u{2551}     {}                   \u{2551}", t!("terminal.banner_subtitle")).cyan());
    println!("{}", "╚═══════════════════════════════════════╝".cyan());
    println!();

    let mut game = Game::new();

    print_help();
    print_board(&game);
    print_status(&game);

    loop {
        if game.is_over() {
            print_game_result(&game);
            break;
        }

        let turn_prompt = match game.turn {
            Color::White => "White".white().bold(),
            Color::Black => "Black".blue().bold(),
        };

        print!("{} > ", turn_prompt);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("{}", t!("terminal.input_error"));
            continue;
        }
        let input = input.trim().to_lowercase();

        if input.is_empty() {
            continue;
        }

        match input.as_str() {
            "quit" | "exit" | "q" => {
                println!("{}", t!("terminal.goodbye"));
                break;
            }
            "help" | "h" | "?" => {
                print_help();
            }
            "board" | "b" => {
                print_board(&game);
                print_status(&game);
            }
            "moves" | "m" => {
                let moves = game.legal_moves();
                println!(
                    "{} {}",
                    t!("terminal.legal_moves_header").to_string().yellow().bold(),
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
            "resign" | "r" => {
                let action = ActionJson {
                    action: "resign".to_string(),
                    reason: None,
                };
                match game.process_action(&action) {
                    Ok(()) => {
                        print_board(&game);
                        print_game_result(&game);
                        break;
                    }
                    Err(e) => println!("{}: {}", t!("terminal.error_label").to_string().red().bold(), e),
                }
            }
            "draw" | "d" => {
                // Try to claim a draw
                let can_claim_repetition = game.position_history.iter()
                    .filter(|p| {
                        *p == game.position_history.last().unwrap()
                    })
                    .count() >= 3;

                let can_claim_fifty = game.halfmove_clock >= 100;

                if can_claim_repetition {
                    let action = ActionJson {
                        action: "claim_draw".to_string(),
                        reason: Some("threefold_repetition".to_string()),
                    };
                    match game.process_action(&action) {
                        Ok(()) => {
                            print_game_result(&game);
                            break;
                        }
                        Err(e) => println!("{}: {}", t!("terminal.error_label").to_string().red().bold(), e),
                    }
                } else if can_claim_fifty {
                    let action = ActionJson {
                        action: "claim_draw".to_string(),
                        reason: Some("fifty_move_rule".to_string()),
                    };
                    match game.process_action(&action) {
                        Ok(()) => {
                            print_game_result(&game);
                            break;
                        }
                        Err(e) => println!("{}: {}", t!("terminal.error_label").to_string().red().bold(), e),
                    }
                } else {
                    println!(
                        "{}",
                        t!(
                            "terminal.no_draw_available",
                            clock = game.halfmove_clock,
                            reps = game.position_history.iter()
                                .filter(|p| *p == game.position_history.last().unwrap())
                                .count()
                        )
                    );
                }
            }
            "history" => {
                print_history(&game);
            }
            "json" | "j" => {
                let state = game.to_game_state_json();
                println!("{}", serde_json::to_string_pretty(&state).unwrap());
                println!();
            }
            _ => {
                // Try to parse as a move (e.g. "e2e4" or "e7e8Q")
                if let Some(move_json) = parse_move_input(&input) {
                    match game.make_move(&move_json) {
                        Ok(()) => {
                            print_board(&game);
                            print_status(&game);

                            if game.is_over() {
                                print_game_result(&game);
                                break;
                            }
                        }
                        Err(e) => {
                            println!("{}: {}", t!("terminal.illegal_move").to_string().red().bold(), e);
                        }
                    }
                } else {
                    println!(
                        "{}",
                        t!("terminal.unknown_cmd_hint", cmd = &input, help = "help".green())
                    );
                }
            }
        }
    }
}

/// Parses a move input string like "e2e4" or "e7e8Q" into a MoveJson.
///
/// Accepts formats:
/// - `e2e4` — normal move
/// - `e7e8Q` — promotion (Q, R, B, N)
/// - `e2 e4` — with space separator
fn parse_move_input(input: &str) -> Option<MoveJson> {
    let input = input.replace(' ', "");
    let input = input.trim();

    if input.len() < 4 || input.len() > 5 {
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
}
