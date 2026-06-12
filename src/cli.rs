//! Engine-facing command-line entry points (`analyze`, `bench`) and the
//! shared search-with-live-progress helper used across the interactive CLI.
//!
//! These commands showcase the search engine directly: they run a real
//! search on a position and stream an animated progress display via
//! [`crate::tui`], then print a formatted summary. All user-facing text is
//! routed through the i18n layer ([`t!`]).

use colored::Colorize;

use crate::game::Game;
use crate::search::{
    SearchEngine, SearchLimits, SearchPosition, SearchResult, score_to_mate_in,
};
use crate::terminal;
use crate::tui::{self, SearchProgressView};
use crate::types::{ChessMove, MoveJson, PieceKind};

/// Default transposition-table size (MB) for the standalone CLI commands.
pub const DEFAULT_TT_SIZE_MB: usize = 64;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Builds an immutable [`SearchPosition`] snapshot from a live [`Game`].
pub fn position_from_game(game: &Game) -> SearchPosition {
    SearchPosition::new(
        game.board.clone(),
        game.turn,
        game.castling,
        game.en_passant,
        game.halfmove_clock,
    )
}

/// Runs a search on `game`'s current position under `limits`, streaming a
/// live animated progress line, and returns the full [`SearchResult`].
///
/// Shared by `analyze`, `bench`, and engine play so the search-driving code
/// lives in exactly one place.
pub fn think(game: &Game, limits: &SearchLimits, tt_size_mb: usize) -> SearchResult {
    let pos = position_from_game(game);
    let mut engine = SearchEngine::new(tt_size_mb);
    let view = SearchProgressView::new();
    let result = engine.search_limited(&pos, limits, Some(&mut |info| view.update(info)));
    view.finish();
    result
}

/// Converts an engine [`ChessMove`] into the JSON move form accepted by
/// [`Game::make_move`].
pub fn move_to_json(mv: &ChessMove) -> MoveJson {
    MoveJson {
        from: mv.from.to_algebraic(),
        to: mv.to.to_algebraic(),
        promotion: mv.promotion.map(|p| promotion_letter(p).to_string()),
    }
}

/// Returns the uppercase promotion letter for a piece kind (`Q`/`R`/`B`/`N`).
fn promotion_letter(kind: PieceKind) -> char {
    match kind {
        PieceKind::Queen => 'Q',
        PieceKind::Rook => 'R',
        PieceKind::Bishop => 'B',
        PieceKind::Knight => 'N',
        _ => 'Q',
    }
}

/// Renders a friendly, language-neutral move description such as
/// `"N g1→f3"`, `"R a1×a7"`, or `"P e7→e8=Q"` (piece letter, separator that
/// reflects captures, and a promotion suffix).
pub fn pretty_move(game: &Game, mv: &ChessMove) -> String {
    let glyph = game
        .board
        .get(mv.from)
        .map(|p| piece_letter(p.kind))
        .unwrap_or(' ');
    let is_capture = game.board.get(mv.to).is_some() || mv.is_en_passant;
    let separator = if is_capture { "×" } else { "→" };
    let promotion = mv
        .promotion
        .map(|p| format!("={}", promotion_letter(p)))
        .unwrap_or_default();

    let castling = if mv.is_castling {
        if mv.to.file > mv.from.file {
            "  (O-O)"
        } else {
            "  (O-O-O)"
        }
    } else {
        ""
    };

    format!(
        "{} {}{}{}{}{}",
        glyph,
        mv.from.to_algebraic(),
        separator,
        mv.to.to_algebraic(),
        promotion,
        castling
    )
}

/// Returns the piece letter used in move descriptions (pawns render as `P`).
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

// ---------------------------------------------------------------------------
// `analyze` command
// ---------------------------------------------------------------------------

/// Analyzes a single position (a FEN string, or the start position when
/// `fen` is `None`) and prints the best move, evaluation, and principal
/// variation with a live, animated search display.
pub fn run_analyze(
    fen: Option<&str>,
    depth: u32,
    move_time: Option<u64>,
    tt_size_mb: usize,
) -> Result<(), String> {
    let game = match fen {
        Some(f) => Game::from_fen(f)?,
        None => Game::new(),
    };

    print_section_header(&t!("analyze.header"));
    println!(
        "  {} {}",
        t!("analyze.source").bold(),
        match fen {
            Some(f) => f.dimmed().to_string(),
            None => t!("analyze.source_start").dimmed().to_string(),
        }
    );
    terminal::print_board(&game);

    if game.legal_moves().is_empty() {
        println!("  {}", t!("analyze.no_moves").yellow().bold());
        return Ok(());
    }

    let limits = SearchLimits {
        max_depth: depth.clamp(1, 128) as i32,
        move_time_ms: move_time,
        max_nodes: None,
    };

    println!("  {}", t!("analyze.thinking").cyan());
    let result = think(&game, &limits, tt_size_mb);
    print_analysis_result(&game, &result);
    Ok(())
}

/// Prints the formatted result of an `analyze` run.
fn print_analysis_result(game: &Game, result: &SearchResult) {
    println!();
    println!("{}", tui::divider(60));

    let mate_in = score_to_mate_in(result.score);
    match &result.best_move {
        Some(mv) => {
            println!(
                "  {}  {}",
                t!("analyze.best_move").bold(),
                pretty_move(game, mv).green().bold()
            );
        }
        None => {
            println!("  {}", t!("analyze.no_best_move").yellow());
        }
    }

    println!(
        "  {}  {}  ({} {})",
        t!("analyze.evaluation").bold(),
        tui::colorize_score(result.score, mate_in),
        game.turn,
        t!("analyze.to_move")
    );

    if let Some(n) = mate_in {
        let key = if n > 0 {
            "analyze.mate_for_side"
        } else {
            "analyze.mate_against_side"
        };
        println!("  {}", t!(key, n = n.abs()).magenta().bold());
    }

    println!(
        "  {}  {}   {}  {}   {}  {}",
        t!("engine.label_depth").bold(),
        result.depth,
        t!("engine.label_nodes").bold(),
        tui::humanize_count(result.stats.nodes),
        t!("engine.label_time").bold(),
        format!("{:.2}s", result.time_ms as f64 / 1000.0),
    );

    if !result.pv.is_empty() {
        let pv = result
            .pv
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        println!("  {}  {}", t!("engine.label_pv").bold(), pv.cyan());
    }

    println!("{}", tui::divider(60));
}

// ---------------------------------------------------------------------------
// `bench` command
// ---------------------------------------------------------------------------

/// Runs a fixed-depth benchmark search from the starting position and prints
/// the node count, elapsed time, and nodes-per-second throughput.
pub fn run_bench(depth: u32, tt_size_mb: usize) -> Result<(), String> {
    let game = Game::new();
    print_section_header(&t!("bench.header"));
    println!(
        "  {} {}",
        t!("bench.config").bold(),
        t!("bench.config_value", depth = depth, tt = tt_size_mb).dimmed()
    );

    let limits = SearchLimits::depth(depth.clamp(1, 128) as i32);
    let result = think(&game, &limits, tt_size_mb);

    let nodes = result.stats.nodes;
    let scaled = nodes.saturating_mul(1000);
    let nps = scaled.checked_div(result.time_ms).unwrap_or(scaled);

    println!();
    println!("{}", tui::divider(60));
    println!(
        "  {}  {}",
        t!("bench.nodes").bold(),
        tui::humanize_count(nodes)
    );
    println!(
        "  {}  {}",
        t!("bench.qnodes").bold(),
        tui::humanize_count(result.stats.quiescence_nodes)
    );
    println!(
        "  {}  {:.2}s",
        t!("bench.time").bold(),
        result.time_ms as f64 / 1000.0
    );
    println!(
        "  {}  {} {}",
        t!("bench.nps").bold(),
        tui::humanize_count(nps).green().bold(),
        t!("bench.nps_unit").dimmed()
    );
    if let Some(mv) = &result.best_move {
        println!(
            "  {}  {}",
            t!("analyze.best_move").bold(),
            pretty_move(&game, mv).green()
        );
    }
    println!("{}", tui::divider(60));
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared presentation
// ---------------------------------------------------------------------------

/// Prints a boxed, colored section header used by the engine CLI commands.
fn print_section_header(title: &str) {
    let width = 60;
    println!();
    println!("{}", tui::divider(width));
    println!("  {}", title.cyan().bold());
    println!("{}", tui::divider(width));
}
