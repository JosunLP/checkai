//! Human-readable game export from archive storage.
//!
//! Provides formatting functions that convert archived games into
//! readable text, PGN (Portable Game Notation), or JSON — suitable
//! for post-game analysis, sharing, and import into other tools.
//!
//! # Supported Formats
//!
//! - **text**: Rich human-readable output with move list, board diagrams,
//!   timestamps, and game metadata.
//! - **pgn**: Standard PGN format compatible with any chess software.
//! - **json**: Full game data as pretty-printed JSON.

use crate::api::board_to_ascii;
use crate::movegen;
use crate::storage::{GameArchive, GameStorage};
use crate::types::*;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Export format enum
// ---------------------------------------------------------------------------

/// Output format for game exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Rich human-readable text with board diagrams.
    Text,
    /// Portable Game Notation (PGN), the chess standard.
    Pgn,
    /// Full game data as pretty-printed JSON.
    Json,
}

impl ExportFormat {
    /// Parses a format string (case-insensitive).
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "text" | "txt" => Ok(Self::Text),
            "pgn" => Ok(Self::Pgn),
            "json" => Ok(Self::Json),
            _ => Err(format!(
                "Unknown export format '{}'. Valid: text, pgn, json",
                s
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Timestamp formatting
// ---------------------------------------------------------------------------

/// Formats a Unix timestamp into a human-readable UTC datetime string.
///
/// Returns `"—"` for timestamp 0 (game not yet ended).
fn format_timestamp(ts: u64) -> String {
    if ts == 0 {
        return "—".to_string();
    }

    // Manual UTC formatting without chrono dependency:
    // Calculate date/time from Unix epoch seconds.
    let secs = ts;
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year/month/day from days since epoch (1970-01-01)
    let (year, month, day) = days_to_date(days);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year, month, day, hours, minutes, seconds
    )
}

/// Converts days since Unix epoch to (year, month, day).
fn days_to_date(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Formats a duration in seconds into a human-readable string.
fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        let h = seconds / 3600;
        let m = (seconds % 3600) / 60;
        let s = seconds % 60;
        format!("{}h {}m {}s", h, m, s)
    }
}

// ---------------------------------------------------------------------------
// Text format — rich human-readable output
// ---------------------------------------------------------------------------

/// Formats a game archive as rich human-readable text.
///
/// Includes:
/// - Header with game ID, timestamps, result
/// - Numbered move list with White/Black columns
/// - Board diagram of the final position
/// - Storage size info
pub fn format_text(archive: &GameArchive, compressed_bytes: Option<u64>) -> Result<String, String> {
    let mut out = String::new();

    // ── Header ──────────────────────────────────────────────
    out.push_str("╔══════════════════════════════════════════════════════════╗\n");
    out.push_str("║                    CHECKAI GAME EXPORT                  ║\n");
    out.push_str("╚══════════════════════════════════════════════════════════╝\n\n");

    out.push_str(&format!("  Game ID:    {}\n", archive.game_id));
    out.push_str(&format!("  Started:    {}\n", format_timestamp(archive.start_timestamp)));
    out.push_str(&format!("  Ended:      {}\n", format_timestamp(archive.end_timestamp)));

    if archive.end_timestamp > archive.start_timestamp && archive.start_timestamp > 0 {
        let duration = archive.end_timestamp - archive.start_timestamp;
        out.push_str(&format!("  Duration:   {}\n", format_duration(duration)));
    }

    out.push_str(&format!("  Moves:      {} half-moves", archive.move_count()));
    let fullmoves = (archive.move_count() + 1) / 2;
    out.push_str(&format!(" ({} full moves)\n", fullmoves));

    // Result
    match &archive.result {
        Some(result) => {
            out.push_str(&format!("  Result:     {}\n", result));
        }
        None => {
            out.push_str("  Result:     In progress\n");
        }
    }
    if let Some(reason) = &archive.end_reason {
        out.push_str(&format!("  Reason:     {}\n", reason));
    }

    // Storage info
    let raw = archive.raw_size();
    out.push_str(&format!("  Raw size:   {} bytes\n", raw));
    if let Some(comp) = compressed_bytes {
        let ratio = if raw > 0 {
            (comp as f64 / raw as f64) * 100.0
        } else {
            0.0
        };
        out.push_str(&format!(
            "  Compressed: {} bytes ({:.1}%)\n",
            comp, ratio
        ));
    }

    // ── Move list ───────────────────────────────────────────
    out.push_str("\n┌──────────────────────────────────┐\n");
    out.push_str("│           MOVE LIST              │\n");
    out.push_str("├─────┬─────────────┬──────────────┤\n");
    out.push_str("│  #  │    White    │    Black     │\n");
    out.push_str("├─────┼─────────────┼──────────────┤\n");

    let mut i = 0;
    let mut move_num = 1;
    while i < archive.moves.len() {
        let white_move = format_move_notation(&archive.moves[i]);
        let black_move = if i + 1 < archive.moves.len() {
            format_move_notation(&archive.moves[i + 1])
        } else {
            "".to_string()
        };

        out.push_str(&format!(
            "│ {:>3} │ {:>11} │ {:>12} │\n",
            move_num, white_move, black_move
        ));

        i += 2;
        move_num += 1;
    }

    out.push_str("└─────┴─────────────┴──────────────┘\n");

    // ── Final position board ────────────────────────────────
    out.push_str("\n  Final Position:\n\n");
    let game = archive.replay_full()?;
    let board_str = board_to_ascii(&game.board, game.turn);
    // Indent the board
    for line in board_str.lines() {
        out.push_str(&format!("  {}\n", line));
    }

    // ── Check / checkmate status at end ─────────────────────
    if game.is_over() {
        if let Some(reason) = &game.end_reason {
            out.push_str(&format!("\n  Game ended by: {}\n", reason));
        }
    } else {
        let is_check = movegen::is_in_check(&game.board, game.turn);
        if is_check {
            out.push_str(&format!("\n  {} is in check.\n", game.turn));
        }
    }

    Ok(out)
}

/// Formats a single move in human-readable notation (e.g. "e2→e4", "e7→e8=Q").
fn format_move_notation(mv: &MoveJson) -> String {
    let mut s = format!("{}→{}", mv.from, mv.to);
    if let Some(promo) = &mv.promotion {
        s.push('=');
        s.push_str(promo);
    }
    s
}

// ---------------------------------------------------------------------------
// PGN format — Portable Game Notation
// ---------------------------------------------------------------------------

/// Formats a game archive as PGN (Portable Game Notation).
///
/// Produces a standard PGN file that can be imported into any chess
/// software (Lichess, chess.com, SCID, ChessBase, etc.).
///
/// Note: Uses coordinate notation (e2e4) since the archive doesn't
/// store standard algebraic notation (SAN). Most software accepts this.
pub fn format_pgn(archive: &GameArchive) -> Result<String, String> {
    let mut out = String::new();

    // PGN headers (Seven Tag Roster)
    out.push_str(&format!(
        "[Event \"CheckAI Game\"]\n"
    ));
    out.push_str(&format!(
        "[Site \"CheckAI Server\"]\n"
    ));

    // Date
    if archive.start_timestamp > 0 {
        let (y, m, d) = days_to_date(archive.start_timestamp / 86400);
        out.push_str(&format!("[Date \"{:04}.{:02}.{:02}\"]\n", y, m, d));
    } else {
        out.push_str("[Date \"????.??.??\"]\n");
    }

    out.push_str("[Round \"1\"]\n");
    out.push_str("[White \"Agent White\"]\n");
    out.push_str("[Black \"Agent Black\"]\n");

    // Result tag
    let result_str = match &archive.result {
        Some(GameResult::WhiteWins) => "1-0",
        Some(GameResult::BlackWins) => "0-1",
        Some(GameResult::Draw) => "1/2-1/2",
        None => "*",
    };
    out.push_str(&format!("[Result \"{}\"]\n", result_str));

    // Extra tags
    out.push_str(&format!(
        "[GameId \"{}\"]\n",
        archive.game_id
    ));
    if let Some(reason) = &archive.end_reason {
        out.push_str(&format!("[Termination \"{}\"]\n", reason));
    }
    out.push('\n');

    // Move text — coordinate notation with move numbers
    let mut move_text = String::new();
    for (i, mv) in archive.moves.iter().enumerate() {
        if i % 2 == 0 {
            // White's move — prepend the move number
            let move_num = i / 2 + 1;
            if !move_text.is_empty() {
                move_text.push(' ');
            }
            move_text.push_str(&format!("{}.", move_num));
        }
        move_text.push(' ');

        // Format: from+to (e.g. "e2e4") with optional promotion
        move_text.push_str(&mv.from);
        move_text.push_str(&mv.to);
        if let Some(promo) = &mv.promotion {
            move_text.push_str(promo);
        }
    }

    // Append result
    if !move_text.is_empty() {
        move_text.push(' ');
    }
    move_text.push_str(result_str);

    // Wrap at 80 columns per PGN spec
    let wrapped = wrap_pgn_text(&move_text, 80);
    out.push_str(&wrapped);
    out.push('\n');

    Ok(out)
}

/// Wraps PGN movetext at word boundaries to fit within `max_width` columns.
fn wrap_pgn_text(text: &str, max_width: usize) -> String {
    let mut result = String::new();
    let mut line_len = 0;

    for word in text.split_whitespace() {
        let word_len = word.len();
        if line_len > 0 && line_len + 1 + word_len > max_width {
            result.push('\n');
            line_len = 0;
        }
        if line_len > 0 {
            result.push(' ');
            line_len += 1;
        }
        result.push_str(word);
        line_len += word_len;
    }

    result
}

// ---------------------------------------------------------------------------
// JSON format — structured data
// ---------------------------------------------------------------------------

/// Formats a game archive as pretty-printed JSON.
///
/// Includes metadata, the full move list, and the final board position.
pub fn format_json(archive: &GameArchive) -> Result<String, String> {
    let game = archive.replay_full()?;

    let board_map = game.board.to_map();

    let export = serde_json::json!({
        "game_id": archive.game_id.to_string(),
        "start_timestamp": archive.start_timestamp,
        "end_timestamp": archive.end_timestamp,
        "start_time": format_timestamp(archive.start_timestamp),
        "end_time": format_timestamp(archive.end_timestamp),
        "result": archive.result.as_ref().map(|r| r.to_string()),
        "end_reason": archive.end_reason.as_ref().map(|r| r.to_string()),
        "move_count": archive.move_count(),
        "fullmove_count": (archive.move_count() + 1) / 2,
        "moves": archive.moves.iter().enumerate().map(|(i, mv)| {
            serde_json::json!({
                "half_move": i + 1,
                "move_number": i / 2 + 1,
                "side": if i % 2 == 0 { "White" } else { "Black" },
                "from": mv.from,
                "to": mv.to,
                "promotion": mv.promotion,
                "notation": format_move_notation(mv),
            })
        }).collect::<Vec<_>>(),
        "final_position": board_map,
        "final_turn": game.turn.to_string(),
    });

    serde_json::to_string_pretty(&export)
        .map_err(|e| format!("JSON serialization failed: {}", e))
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

/// Runs the export CLI command.
///
/// Handles listing archived games, exporting single games or all games,
/// and writing output to stdout or a file.
pub fn run_export(
    data_dir: &str,
    format: ExportFormat,
    game_id: Option<&str>,
    list_only: bool,
    all: bool,
    output: Option<&str>,
) -> Result<(), String> {
    let storage = GameStorage::new(data_dir)
        .map_err(|e| format!("Failed to open storage at '{}': {}", data_dir, e))?;

    // ── List mode ───────────────────────────────────────────
    if list_only {
        return run_list(&storage);
    }

    // ── Export all games ────────────────────────────────────
    if all {
        return run_export_all(&storage, format, output);
    }

    // ── Export single game ──────────────────────────────────
    let id_str = game_id.ok_or(
        "Please specify --game-id <UUID> or use --list / --all"
    )?;
    let id = Uuid::parse_str(id_str)
        .map_err(|_| format!("Invalid game ID: '{}'", id_str))?;

    let (archive, _compressed) = storage.load_any(&id)?;
    let compressed_bytes = storage.archive_file_size(&id);
    let text = format_game(&archive, format, compressed_bytes)?;

    write_output(&text, output)?;
    Ok(())
}

/// Lists all archived games in a human-readable table.
fn run_list(storage: &GameStorage) -> Result<(), String> {
    let archived = storage.list_archived()?;
    let active = storage.list_active_on_disk()?;

    if archived.is_empty() && active.is_empty() {
        println!("No games found in storage.");
        return Ok(());
    }

    let stats = storage.stats()?;

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                       ARCHIVED GAMES                           ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");

    if !archived.is_empty() {
        println!("║                                                                ║");
        println!("║  Completed ({} games, {} bytes compressed):                  ",
            stats.archived_count, stats.archive_bytes);
        println!("║                                                                ║");

        for id in &archived {
            if let Ok(archive) = storage.load_archive(id) {
                let result_str = match &archive.result {
                    Some(r) => r.to_string(),
                    None => "—".to_string(),
                };
                let size = storage.archive_file_size(id).unwrap_or(0);
                let fullmoves = (archive.move_count() + 1) / 2;
                println!(
                    "║  {} │ {:>3} moves │ {:>5} B │ {}",
                    id, fullmoves, size, result_str
                );
            }
        }
    }

    if !active.is_empty() {
        println!("║                                                                ║");
        println!("║  Active ({} games, {} bytes):",
            stats.active_count, stats.active_bytes);
        println!("║                                                                ║");

        for id in &active {
            if let Ok(archive) = storage.load_active(id) {
                let fullmoves = (archive.move_count() + 1) / 2;
                println!(
                    "║  {} │ {:>3} moves │ In progress",
                    id, fullmoves
                );
            }
        }
    }

    println!("║                                                                ║");
    println!("║  Total storage: {} bytes                                     ", stats.total_bytes);
    println!("╚══════════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Exports all archived games into a single output.
fn run_export_all(
    storage: &GameStorage,
    format: ExportFormat,
    output: Option<&str>,
) -> Result<(), String> {
    let archived = storage.list_archived()?;
    if archived.is_empty() {
        println!("No archived games found.");
        return Ok(());
    }

    let mut combined = String::new();
    let separator = match format {
        ExportFormat::Text => "\n\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n",
        ExportFormat::Pgn => "\n\n",
        ExportFormat::Json => "\n,\n", // separate JSON objects with comma
    };

    if format == ExportFormat::Json {
        combined.push_str("[\n");
    }

    for (idx, id) in archived.iter().enumerate() {
        let archive = storage.load_archive(id)?;
        let compressed_bytes = storage.archive_file_size(id);
        let text = format_game(&archive, format, compressed_bytes)?;

        if idx > 0 {
            combined.push_str(separator);
        }
        combined.push_str(&text);
    }

    if format == ExportFormat::Json {
        combined.push_str("\n]\n");
    }

    write_output(&combined, output)?;

    eprintln!(
        "Exported {} game(s) in {:?} format.",
        archived.len(),
        format
    );

    Ok(())
}

/// Formats a single game in the given format.
fn format_game(
    archive: &GameArchive,
    format: ExportFormat,
    compressed_bytes: Option<u64>,
) -> Result<String, String> {
    match format {
        ExportFormat::Text => format_text(archive, compressed_bytes),
        ExportFormat::Pgn => format_pgn(archive),
        ExportFormat::Json => format_json(archive),
    }
}

/// Writes output to stdout or a file.
fn write_output(content: &str, output_path: Option<&str>) -> Result<(), String> {
    match output_path {
        Some(path) => {
            std::fs::write(path, content)
                .map_err(|e| format!("Failed to write to '{}': {}", path, e))?;
            eprintln!("Written to: {}", path);
            Ok(())
        }
        None => {
            print!("{}", content);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    fn make_sample_game() -> GameArchive {
        let mut game = Game::new();
        let moves = vec![
            ("e2", "e4"), ("e7", "e5"),
            ("g1", "f3"), ("b8", "c6"),
            ("f1", "b5"), ("a7", "a6"),
        ];
        for (from, to) in moves {
            game.make_move(&MoveJson {
                from: from.into(), to: to.into(), promotion: None,
            }).unwrap();
        }

        GameArchive {
            game_id: game.id,
            start_timestamp: 1740000000, // 2025-02-19 ~16:00 UTC
            end_timestamp: 1740000300,   // 5 minutes later
            result: Some(GameResult::WhiteWins),
            end_reason: Some(GameEndReason::Resignation),
            moves: game.move_history.iter().map(|r| r.move_json.clone()).collect(),
        }
    }

    #[test]
    fn test_format_text_produces_output() {
        let archive = make_sample_game();
        let text = format_text(&archive, Some(150)).unwrap();

        assert!(text.contains("CHECKAI GAME EXPORT"));
        assert!(text.contains(&archive.game_id.to_string()));
        assert!(text.contains("e2→e4"));
        assert!(text.contains("e7→e5"));
        assert!(text.contains("MOVE LIST"));
        assert!(text.contains("Final Position"));
        assert!(text.contains("Resignation"));
    }

    #[test]
    fn test_format_pgn_valid() {
        let archive = make_sample_game();
        let pgn = format_pgn(&archive).unwrap();

        assert!(pgn.contains("[Event \"CheckAI Game\"]"));
        assert!(pgn.contains("[Result \"1-0\"]"));
        assert!(pgn.contains("1. e2e4 e7e5"));
        assert!(pgn.contains("2. g1f3 b8c6"));
        assert!(pgn.contains("1-0"));
    }

    #[test]
    fn test_format_json_parseable() {
        let archive = make_sample_game();
        let json = format_json(&archive).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["move_count"], 6);
        assert_eq!(parsed["moves"].as_array().unwrap().len(), 6);
        assert!(parsed["game_id"].is_string());
        assert!(parsed["final_position"].is_object());
    }

    #[test]
    fn test_format_timestamp() {
        let ts = format_timestamp(0);
        assert_eq!(ts, "—");

        let ts = format_timestamp(1740000000);
        assert!(ts.contains("2025"));
        assert!(ts.contains("UTC"));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m 1s");
    }

    #[test]
    fn test_wrap_pgn_text() {
        let long = "1. e2e4 e7e5 2. g1f3 b8c6 3. f1b5 a7a6 4. b5a4 g8f6 5. e1g1 f8e7";
        let wrapped = wrap_pgn_text(long, 40);
        for line in wrapped.lines() {
            assert!(line.len() <= 40, "Line too long: {}", line);
        }
    }
}
