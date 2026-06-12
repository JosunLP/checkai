//! FEN import/export helpers for CLI commands.
//!
//! This module provides a strict 4–6 field FEN parser (it rejects unknown
//! castling characters and rank overflow with explicit errors, which is
//! desirable for interactive CLI input), the reverse direction (full
//! 6-field FEN output), the canonical `Game` → `SearchPosition` conversion
//! used by every engine-facing command, and the position-history hashes
//! that feed the engine's repetition detection.

use crate::game::Game;
use crate::search::SearchPosition;
use crate::types::{Board, CastlingRights, Color, Piece, SideCastlingRights, Square};
use crate::zobrist;

/// FEN of the standard starting position.
pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Parses a 4–6 field FEN string into a fresh [`Game`].
///
/// The halfmove clock and fullmove number are optional (default `0` / `1`).
/// Position history starts at the imported position, so repetition
/// counting only sees moves played after the import.
pub fn game_from_fen(fen: &str) -> Result<Game, String> {
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.len() < 4 {
        return Err("FEN must have at least 4 fields".to_string());
    }

    // Piece placement.
    let mut board = Board::default();
    let rows: Vec<&str> = parts[0].split('/').collect();
    if rows.len() != 8 {
        return Err("FEN piece placement must have exactly 8 ranks".to_string());
    }
    for (row_idx, row) in rows.iter().enumerate() {
        let rank = 7 - row_idx as u8;
        let mut file: u8 = 0;
        for ch in row.chars() {
            if let Some(skip) = ch.to_digit(10) {
                if skip == 0 || skip > 8 {
                    return Err(format!(
                        "Invalid empty-square count '{ch}' in FEN — must be 1–8"
                    ));
                }
                file += skip as u8;
            } else {
                if file >= 8 {
                    return Err(format!("Too many pieces on rank {}", rank + 1));
                }
                let piece =
                    Piece::from_fen_char(ch).ok_or_else(|| format!("Invalid piece '{ch}'"))?;
                board.set(Square::new(file, rank), Some(piece));
                file += 1;
            }
        }
        if file != 8 {
            return Err(format!("Rank {} has {} files, expected 8", rank + 1, file));
        }
    }

    // Side to move.
    let turn = match parts[1] {
        "w" => Color::White,
        "b" => Color::Black,
        other => return Err(format!("Invalid turn field: '{other}'")),
    };

    // Castling rights.
    let mut castling = CastlingRights {
        white: SideCastlingRights {
            kingside: false,
            queenside: false,
        },
        black: SideCastlingRights {
            kingside: false,
            queenside: false,
        },
    };
    if parts[2] != "-" {
        for ch in parts[2].chars() {
            match ch {
                'K' => castling.white.kingside = true,
                'Q' => castling.white.queenside = true,
                'k' => castling.black.kingside = true,
                'q' => castling.black.queenside = true,
                other => return Err(format!("Invalid castling character: '{other}'")),
            }
        }
    }

    // En passant target square.
    let en_passant = if parts[3] == "-" {
        None
    } else {
        Some(
            Square::from_algebraic(parts[3])
                .ok_or_else(|| format!("Invalid en passant square: '{}'", parts[3]))?,
        )
    };

    // Optional clocks.
    let halfmove_clock = match parts.get(4) {
        Some(v) => v
            .parse::<u32>()
            .map_err(|_| format!("Invalid halfmove clock: '{v}'"))?,
        None => 0,
    };
    let fullmove_number = match parts.get(5) {
        Some(v) => v
            .parse::<u32>()
            .map_err(|_| format!("Invalid fullmove number: '{v}'"))?,
        None => 1,
    };

    // Sanity check: both kings must be present for a playable game.
    if board.find_king(Color::White).is_none() || board.find_king(Color::Black).is_none() {
        return Err("Both kings must be present".to_string());
    }

    let initial_fen_str = board.to_position_fen(turn, &castling, en_passant);

    Ok(Game {
        id: uuid::Uuid::new_v4(),
        board,
        turn,
        castling,
        en_passant,
        halfmove_clock,
        fullmove_number,
        position_history: vec![initial_fen_str],
        move_history: Vec::new(),
        result: None,
        end_reason: None,
        draw_offered_by: None,
        start_timestamp: crate::storage::unix_timestamp(),
        end_timestamp: 0,
    })
}

/// Builds the full 6-field FEN string for a live game.
pub fn game_to_fen(game: &Game) -> String {
    format!(
        "{} {} {}",
        game.board
            .to_position_fen(game.turn, &game.castling, game.en_passant),
        game.halfmove_clock,
        game.fullmove_number
    )
}

/// Converts a [`Game`] into the engine's [`SearchPosition`] snapshot
/// (the canonical recipe used by the analysis subsystem).
pub fn search_position(game: &Game) -> SearchPosition {
    SearchPosition::new(
        game.board.clone(),
        game.turn,
        game.castling,
        game.en_passant,
        game.halfmove_clock,
    )
}

/// Zobrist hashes of every position that preceded the current one, in game
/// order — the input to [`crate::search::SearchEngine::set_game_history`].
///
/// `Game` stores its history as position-only FEN strings; we re-hash each so
/// the engine can score a search line that returns to an earlier game position
/// as a draw by repetition. The current position is excluded (the search root
/// is tracked separately, inside the tree).
pub fn history_hashes(game: &Game) -> Vec<u64> {
    let history = &game.position_history;
    let preceding = history.len().saturating_sub(1);
    history[..preceding]
        .iter()
        .filter_map(|fen| match game_from_fen(fen) {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("warning: skipping invalid history FEN ({e}): {fen}");
                None
            }
        })
        .map(|g| zobrist::hash_position(&g.board, g.turn, &g.castling, g.en_passant))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startpos_round_trip() {
        let game = game_from_fen(START_FEN).expect("startpos must parse");
        assert_eq!(game_to_fen(&game), START_FEN);
        assert_eq!(game.legal_moves().len(), 20);
    }

    #[test]
    fn test_kiwipete_parses() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let game = game_from_fen(fen).expect("kiwipete must parse");
        assert_eq!(game.legal_moves().len(), 48);
        assert_eq!(game_to_fen(&game), fen);
    }

    #[test]
    fn test_invalid_fens_rejected() {
        assert!(game_from_fen("").is_err());
        assert!(game_from_fen("8/8/8/8 w - -").is_err()); // 4 ranks only
        assert!(game_from_fen("9/8/8/8/8/8/8/8 w - -").is_err()); // bad digit
        assert!(game_from_fen("8/8/8/8/8/8/8/8 w - -").is_err()); // no kings
        let no_black_king = "4K3/8/8/8/8/8/8/8 w - - 0 1";
        assert!(game_from_fen(no_black_king).is_err());
    }

    #[test]
    fn test_search_position_matches_game() {
        let game = game_from_fen(START_FEN).unwrap();
        let pos = search_position(&game);
        assert_eq!(pos.turn, game.turn);
        assert_eq!(pos.halfmove_clock, game.halfmove_clock);
        assert_eq!(pos.legal_moves().len(), 20);
    }
}
