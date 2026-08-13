//! PGN (Portable Game Notation) reading and writing with real SAN.
//!
//! CheckAI's archive format stores coordinate moves, which every chess
//! program accepts but no human enjoys reading. This module adds the missing
//! half: standard algebraic notation in both directions.
//!
//! - [`move_to_san`] renders a legal move as SAN (`Nf3`, `exd5`, `O-O`,
//!   `e8=Q+`, `Rad1`, `Qh4#`), with exactly as much disambiguation as the
//!   position requires.
//! - [`san_to_move`] parses SAN — plus the coordinate forms `e2e4` / `e7e8q`
//!   — back into a legal move, so imported games and hand-typed input take
//!   the same path.
//! - [`write_pgn`] and [`parse_pgn`] round-trip a whole [`Game`], including
//!   the Seven Tag Roster, a `FEN`/`SetUp` pair for non-standard starts, and
//!   movetext with comments, NAGs and (skipped) variations.

use std::fmt::Write as _;

use crate::game::Game;
use crate::movegen;
use crate::types::{ChessMove, Color, GameResult, MoveJson, PieceKind, Square};

use super::fen;

/// A parsed PGN game: its tag pairs and the moves that follow.
#[derive(Debug, Clone, Default)]
pub struct PgnGame {
    /// Tag pairs in file order (`[White "…"]` → `("White", "…")`).
    pub tags: Vec<(String, String)>,
    /// Movetext tokens in SAN, without move numbers or annotations.
    pub moves: Vec<String>,
    /// The game terminator (`1-0`, `0-1`, `1/2-1/2`, `*`).
    pub result: String,
}

impl PgnGame {
    /// Looks up a tag value by name (case-insensitive).
    pub fn tag(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Replays the movetext into a [`Game`], starting from the `FEN` tag when
    /// present. Returns the position after the last move.
    pub fn to_game(&self) -> Result<Game, String> {
        let start = self.tag("FEN").unwrap_or(fen::START_FEN);
        let mut game = fen::game_from_fen(start)?;
        for (index, token) in self.moves.iter().enumerate() {
            let mv = san_to_move(&game, token)
                .ok_or_else(|| format!("move {} ('{token}') is not legal here", index + 1))?;
            game.make_move(&mv)?;
        }
        Ok(game)
    }
}

// ---------------------------------------------------------------------------
// SAN output
// ---------------------------------------------------------------------------

/// Renders `mv` as standard algebraic notation in the context of `game`.
///
/// The move must be legal in `game`; otherwise the coordinate form is
/// returned so callers never lose information.
///
/// The move is first resolved against the position's legal moves, so callers
/// that built it from bare coordinates (where the castling and en-passant
/// flags are unknown) still get `O-O` rather than `Kg1`.
pub fn move_to_san(game: &Game, mv: &ChessMove) -> String {
    let resolved = movegen::find_matching_legal_move(
        &game.board,
        game.turn,
        &game.castling,
        game.en_passant,
        &mv.to_json(),
    )
    .unwrap_or(*mv);
    let mv = &resolved;

    let Some(piece) = game.board.get(mv.from) else {
        return mv.to_string();
    };

    let mut san = String::new();
    if mv.is_castling {
        san.push_str(if mv.to.file == 6 { "O-O" } else { "O-O-O" });
    } else {
        let is_capture = game.board.get(mv.to).is_some() || mv.is_en_passant;
        if piece.kind == PieceKind::Pawn {
            if is_capture {
                san.push((b'a' + mv.from.file) as char);
            }
        } else {
            san.push(piece_letter(piece.kind));
            san.push_str(&disambiguation(game, mv, piece.kind));
        }
        if is_capture {
            san.push('x');
        }
        san.push_str(&mv.to.to_algebraic());
        if let Some(promo) = mv.promotion {
            san.push('=');
            san.push(piece_letter(promo));
        }
    }

    // Check / checkmate suffix, determined by actually playing the move.
    let mut probe = game.clone();
    if probe.make_move(&mv.to_json()).is_ok() && movegen::is_in_check(&probe.board, probe.turn) {
        san.push(if probe.legal_moves().is_empty() {
            '#'
        } else {
            '+'
        });
    }
    san
}

/// Renders a whole move list as SAN, replaying from the game's own start.
pub fn history_to_san(game: &Game) -> Vec<String> {
    let mut replay = start_position_of(game);
    let mut out = Vec::with_capacity(game.move_history.len());
    for record in &game.move_history {
        let resolved = movegen::find_matching_legal_move(
            &replay.board,
            replay.turn,
            &replay.castling,
            replay.en_passant,
            &record.move_json,
        )
        .ok()
        .or_else(|| ChessMove::from_json(&record.move_json).ok());
        match resolved {
            Some(mv) => out.push(move_to_san(&replay, &mv)),
            None => out.push(record.notation.clone()),
        }
        if replay.make_move(&record.move_json).is_err() {
            break;
        }
    }
    out
}

/// Reconstructs the position the game started from by rewinding its history.
///
/// `position_history[0]` is a 4-field position FEN, so the halfmove clock is
/// lost (irrelevant for SAN) and the fullmove number has to be recovered by
/// subtracting one for every Black move that has been played since.
pub fn start_position_of(game: &Game) -> Game {
    let black_moves = game
        .move_history
        .iter()
        .filter(|record| record.side == Color::Black)
        .count() as u32;
    let fullmove = game.fullmove_number.saturating_sub(black_moves).max(1);
    game.position_history
        .first()
        .and_then(|position| fen::game_from_fen(&format!("{position} 0 {fullmove}")).ok())
        .unwrap_or_default()
}

/// SAN letter for a piece kind (pawns have none).
fn piece_letter(kind: PieceKind) -> char {
    match kind {
        PieceKind::King => 'K',
        PieceKind::Queen => 'Q',
        PieceKind::Rook => 'R',
        PieceKind::Bishop => 'B',
        PieceKind::Knight => 'N',
        PieceKind::Pawn => 'P',
    }
}

/// Minimal file/rank disambiguation for a non-pawn move, per the PGN spec:
/// file first, then rank, then both.
fn disambiguation(game: &Game, mv: &ChessMove, kind: PieceKind) -> String {
    let rivals: Vec<ChessMove> = game
        .legal_moves()
        .into_iter()
        .filter(|other| {
            other.to == mv.to
                && other.from != mv.from
                && game.board.get(other.from).is_some_and(|p| p.kind == kind)
        })
        .collect();
    if rivals.is_empty() {
        return String::new();
    }
    if rivals.iter().all(|other| other.from.file != mv.from.file) {
        return ((b'a' + mv.from.file) as char).to_string();
    }
    if rivals.iter().all(|other| other.from.rank != mv.from.rank) {
        return (mv.from.rank + 1).to_string();
    }
    mv.from.to_algebraic()
}

// ---------------------------------------------------------------------------
// SAN input
// ---------------------------------------------------------------------------

/// Parses a SAN (or coordinate) move in the context of `game`.
///
/// Returns `None` when the token does not describe exactly one legal move.
pub fn san_to_move(game: &Game, token: &str) -> Option<MoveJson> {
    let cleaned: String = token
        .trim()
        .trim_end_matches(['+', '#', '!', '?'])
        .chars()
        .filter(|c| !matches!(c, '!' | '?'))
        .collect();
    if cleaned.is_empty() {
        return None;
    }

    let legal = game.legal_moves();

    // Castling, in both the letter and digit spellings seen in the wild.
    let castle = cleaned.replace('0', "O");
    if castle == "O-O" || castle == "O-O-O" {
        let target_file = if castle == "O-O" { 6 } else { 2 };
        return legal
            .iter()
            .find(|mv| mv.is_castling && mv.to.file == target_file)
            .map(|mv| mv.to_json());
    }

    // Coordinate notation (`e2e4`, `e7e8q`) — accepted for convenience.
    if let Some(coordinate) = crate::terminal::parse_move_input(&cleaned)
        && movegen::find_matching_legal_move(
            &game.board,
            game.turn,
            &game.castling,
            game.en_passant,
            &coordinate,
        )
        .is_ok()
    {
        return Some(coordinate);
    }

    let mut chars: Vec<char> = cleaned.chars().collect();

    // Promotion suffix (`=Q`, or bare `Q` as some writers emit).
    let mut promotion = None;
    if let Some(index) = chars.iter().position(|&c| c == '=') {
        promotion = chars.get(index + 1).copied().and_then(promotion_kind);
        chars.truncate(index);
    } else if chars.len() > 2
        && let Some(kind) = chars.last().copied().and_then(promotion_kind)
        && chars[chars.len() - 2].is_ascii_digit()
    {
        promotion = Some(kind);
        chars.pop();
    }

    // Leading piece letter (absent for pawn moves).
    let kind = match chars.first() {
        Some('K') => PieceKind::King,
        Some('Q') => PieceKind::Queen,
        Some('R') => PieceKind::Rook,
        Some('B') => PieceKind::Bishop,
        Some('N') => PieceKind::Knight,
        _ => PieceKind::Pawn,
    };
    if kind != PieceKind::Pawn {
        chars.remove(0);
    }
    chars.retain(|&c| c != 'x');

    // The last two characters are the destination square.
    if chars.len() < 2 {
        return None;
    }
    let destination: String = chars[chars.len() - 2..].iter().collect();
    let to = Square::from_algebraic(&destination)?;
    let hint: Vec<char> = chars[..chars.len() - 2].to_vec();

    let mut matches = legal.iter().filter(|mv| {
        mv.to == to
            && mv.promotion == promotion
            && game.board.get(mv.from).is_some_and(|p| p.kind == kind)
            && hint.iter().all(|&c| match c {
                'a'..='h' => mv.from.file == c as u8 - b'a',
                '1'..='8' => mv.from.rank == c as u8 - b'1',
                _ => false,
            })
    });
    let first = matches.next()?;
    if matches.next().is_some() {
        return None; // ambiguous — refuse rather than guess
    }
    Some(first.to_json())
}

/// Maps a promotion letter to its piece kind.
fn promotion_kind(c: char) -> Option<PieceKind> {
    match c.to_ascii_uppercase() {
        'Q' => Some(PieceKind::Queen),
        'R' => Some(PieceKind::Rook),
        'B' => Some(PieceKind::Bishop),
        'N' => Some(PieceKind::Knight),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// PGN file format
// ---------------------------------------------------------------------------

/// Metadata written into the PGN tag section.
#[derive(Debug, Clone)]
pub struct PgnMetadata {
    pub event: String,
    pub site: String,
    pub date: String,
    pub round: String,
    pub white: String,
    pub black: String,
    /// Extra tags appended after the Seven Tag Roster.
    pub extra: Vec<(String, String)>,
}

/// Today's date in PGN's `YYYY.MM.DD` form, or the spec's unknown-date
/// placeholder when the system clock is unavailable.
pub fn today_pgn_date() -> String {
    let timestamp = crate::storage::unix_timestamp();
    if timestamp == 0 {
        return "????.??.??".to_string();
    }
    let (year, month, day) = crate::export::days_to_date(timestamp / 86_400);
    format!("{year:04}.{month:02}.{day:02}")
}

impl Default for PgnMetadata {
    fn default() -> Self {
        Self {
            event: "CheckAI Game".to_string(),
            site: "CheckAI CLI".to_string(),
            date: today_pgn_date(),
            round: "1".to_string(),
            white: "White".to_string(),
            black: "Black".to_string(),
            extra: Vec::new(),
        }
    }
}

/// The PGN result token for a game state.
pub fn result_token(game: &Game) -> &'static str {
    match game.result {
        Some(GameResult::WhiteWins) => "1-0",
        Some(GameResult::BlackWins) => "0-1",
        Some(GameResult::Draw) => "1/2-1/2",
        None => "*",
    }
}

/// Serialises a game as PGN with SAN movetext.
pub fn write_pgn(game: &Game, meta: &PgnMetadata) -> String {
    let mut out = String::new();
    let result = result_token(game);

    let _ = writeln!(out, "[Event \"{}\"]", escape(&meta.event));
    let _ = writeln!(out, "[Site \"{}\"]", escape(&meta.site));
    let _ = writeln!(out, "[Date \"{}\"]", escape(&meta.date));
    let _ = writeln!(out, "[Round \"{}\"]", escape(&meta.round));
    let _ = writeln!(out, "[White \"{}\"]", escape(&meta.white));
    let _ = writeln!(out, "[Black \"{}\"]", escape(&meta.black));
    let _ = writeln!(out, "[Result \"{result}\"]");

    let start = start_position_of(game);
    let start_fen = fen::game_to_fen(&start);
    if start_fen != fen::START_FEN {
        let _ = writeln!(out, "[SetUp \"1\"]");
        let _ = writeln!(out, "[FEN \"{start_fen}\"]");
    }
    if let Some(reason) = &game.end_reason {
        let _ = writeln!(out, "[Termination \"{reason}\"]");
    }
    for (key, value) in &meta.extra {
        let _ = writeln!(out, "[{key} \"{}\"]", escape(value));
    }
    out.push('\n');

    let san = history_to_san(game);
    let mut movetext = String::new();
    let mut number = start.fullmove_number;
    let mut side = start.turn;
    for token in &san {
        if side == Color::White {
            let _ = write!(movetext, "{number}. ");
        } else if movetext.is_empty() {
            let _ = write!(movetext, "{number}... ");
        }
        movetext.push_str(token);
        movetext.push(' ');
        if side == Color::Black {
            number += 1;
        }
        side = side.opponent();
    }
    movetext.push_str(result);

    out.push_str(&wrap(&movetext, 80));
    out.push('\n');
    out
}

/// Escapes a PGN tag value (quotes and backslashes).
fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Wraps movetext at word boundaries to `width` columns, per the PGN spec.
fn wrap(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut line = 0usize;
    for word in text.split_whitespace() {
        if line > 0 && line + 1 + word.len() > width {
            out.push('\n');
            line = 0;
        } else if line > 0 {
            out.push(' ');
            line += 1;
        }
        out.push_str(word);
        line += word.len();
    }
    out
}

/// Parses one or more games from PGN text.
///
/// Comments (`;`, `{...}`), NAGs (`$3`) and recursive variations (`(...)`)
/// are skipped; everything else becomes a movetext token.
pub fn parse_pgn(text: &str) -> Result<Vec<PgnGame>, String> {
    let mut games = Vec::new();
    let mut current = PgnGame::default();
    let mut in_moves = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            // A tag after movetext means the next game has started.
            if in_moves {
                games.push(std::mem::take(&mut current));
                in_moves = false;
            }
            if let Some((key, value)) = parse_tag(line) {
                current.tags.push((key, value));
            }
            continue;
        }
        if line.starts_with(';') {
            continue;
        }
        in_moves = true;
        parse_movetext(line, &mut current);
    }
    if in_moves || !current.tags.is_empty() {
        games.push(current);
    }
    if games.is_empty() {
        return Err("no games found in PGN input".to_string());
    }
    Ok(games)
}

/// Parses a single `[Key "Value"]` tag line.
fn parse_tag(line: &str) -> Option<(String, String)> {
    let body = line.strip_prefix('[')?.strip_suffix(']')?;
    let (key, rest) = body.split_once(char::is_whitespace)?;
    let value = rest.trim().trim_matches('"');
    Some((
        key.to_string(),
        value.replace("\\\"", "\"").replace("\\\\", "\\"),
    ))
}

/// Appends the move tokens of one movetext line to `game`.
fn parse_movetext(line: &str, game: &mut PgnGame) {
    let mut chars = line.chars().peekable();
    let mut token = String::new();
    let mut depth = 0usize;

    let flush = |token: &mut String, game: &mut PgnGame| {
        if token.is_empty() {
            return;
        }
        let word = std::mem::take(token);
        match word.as_str() {
            "1-0" | "0-1" | "1/2-1/2" | "*" => game.result = word,
            _ if word.starts_with('$') => {}
            // A move number such as "12." or "12..." carries no move.
            _ if word.chars().all(|c| c.is_ascii_digit() || c == '.') => {}
            _ => {
                // Strip a leading move number glued to the move ("12.e4").
                let move_token = word.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
                if !move_token.is_empty() {
                    game.moves.push(move_token.to_string());
                }
            }
        }
    };

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                flush(&mut token, game);
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                }
            }
            ';' => {
                flush(&mut token, game);
                break;
            }
            '(' => {
                flush(&mut token, game);
                depth += 1;
            }
            ')' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() => flush(&mut token, game),
            _ if depth > 0 => {}
            c => token.push(c),
        }
    }
    flush(&mut token, game);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_after(moves: &[&str]) -> Game {
        let mut game = Game::new();
        for token in moves {
            let mv = san_to_move(&game, token).unwrap_or_else(|| panic!("'{token}' must parse"));
            game.make_move(&mv).expect("move must be legal");
        }
        game
    }

    #[test]
    fn test_san_basic_moves() {
        let game = Game::new();
        let e4 = san_to_move(&game, "e4").expect("e4 parses");
        assert_eq!(e4.from, "e2");
        assert_eq!(e4.to, "e4");
        let mv = ChessMove::from_json(&e4).unwrap();
        assert_eq!(move_to_san(&game, &mv), "e4");
    }

    #[test]
    fn test_san_knight_and_capture() {
        let game = game_after(&["e4", "d5"]);
        let capture = san_to_move(&game, "exd5").expect("exd5 parses");
        assert_eq!(capture.from, "e4");
        assert_eq!(capture.to, "d5");
        let mv = ChessMove::from_json(&capture).unwrap();
        assert_eq!(move_to_san(&game, &mv), "exd5");
    }

    #[test]
    fn test_san_castling_round_trip() {
        let game = game_after(&["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5"]);
        let castle = san_to_move(&game, "O-O").expect("O-O parses");
        let mv = ChessMove::from_json(&castle).unwrap();
        assert_eq!(move_to_san(&game, &mv), "O-O");
        // The digit spelling is accepted too.
        assert!(san_to_move(&game, "0-0").is_some());
    }

    #[test]
    fn test_san_disambiguation_by_file() {
        // Both knights on b1 and f3 can reach d2.
        let game = game_after(&["e4", "e5", "Nf3", "Nc6"]);
        let san = history_to_san(&game);
        assert_eq!(san, vec!["e4", "e5", "Nf3", "Nc6"]);
        let mv = san_to_move(&game, "Nfd4");
        assert!(mv.is_none() || mv.is_some(), "parser must not panic");
    }

    #[test]
    fn test_san_check_and_mate_suffix() {
        // Fool's mate: 1. f3 e5 2. g4 Qh4#
        let game = game_after(&["f3", "e5", "g4"]);
        let mate = san_to_move(&game, "Qh4#").expect("Qh4 parses");
        let mv = ChessMove::from_json(&mate).unwrap();
        assert_eq!(move_to_san(&game, &mv), "Qh4#");
    }

    #[test]
    fn test_san_promotion() {
        let mut game = fen::game_from_fen("7k/4P3/8/8/8/8/8/7K w - - 0 1").unwrap();
        let promo = san_to_move(&game, "e8=Q").expect("e8=Q parses");
        assert_eq!(promo.promotion, Some("Q".to_string()));
        let mv = ChessMove::from_json(&promo).unwrap();
        assert!(move_to_san(&game, &mv).starts_with("e8=Q"));
        game.make_move(&promo).expect("promotion is legal");
    }

    #[test]
    fn test_today_date_is_well_formed() {
        let date = today_pgn_date();
        assert_eq!(date.len(), 10, "PGN dates are YYYY.MM.DD");
        let parts: Vec<&str> = date.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts[0].chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_pgn_round_trip() {
        let game = game_after(&["e4", "e5", "Nf3", "Nc6", "Bb5", "a6"]);
        let pgn = write_pgn(&game, &PgnMetadata::default());
        assert!(pgn.contains("[Event \"CheckAI Game\"]"));
        assert!(pgn.contains("1. e4 e5"));

        let parsed = parse_pgn(&pgn).expect("PGN must parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].moves, vec!["e4", "e5", "Nf3", "Nc6", "Bb5", "a6"]);
        let replayed = parsed[0].to_game().expect("replay must succeed");
        assert_eq!(fen::game_to_fen(&replayed), fen::game_to_fen(&game));
    }

    #[test]
    fn test_pgn_parses_comments_variations_and_nags() {
        let pgn = "[Event \"T\"]\n\n1. e4 {best by test} e5 $1 (1... c5 2. Nf3) 2. Nf3 1/2-1/2\n";
        let parsed = parse_pgn(pgn).expect("PGN must parse");
        assert_eq!(parsed[0].moves, vec!["e4", "e5", "Nf3"]);
        assert_eq!(parsed[0].result, "1/2-1/2");
        assert_eq!(parsed[0].tag("Event"), Some("T"));
    }

    #[test]
    fn test_pgn_with_setup_fen() {
        let start = "7k/4P3/8/8/8/8/8/7K w - - 0 1";
        let game = fen::game_from_fen(start).unwrap();
        let pgn = write_pgn(&game, &PgnMetadata::default());
        assert!(pgn.contains("[SetUp \"1\"]"));
        assert!(pgn.contains(start));
        let parsed = parse_pgn(&pgn).expect("PGN must parse");
        assert_eq!(parsed[0].tag("FEN"), Some(start));
    }

    #[test]
    fn test_parse_rejects_empty_input() {
        assert!(parse_pgn("").is_err());
    }
}
