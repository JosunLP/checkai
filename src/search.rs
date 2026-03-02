//! Alpha-beta search engine for the CheckAI analysis module.
//!
//! Implements a full-featured chess search with:
//! - Iterative deepening with aspiration windows
//! - Principal Variation Search (PVS / Negascout)
//! - Transposition table
//! - Null-move pruning
//! - Late Move Reductions (LMR)
//! - Killer move heuristic
//! - History heuristic for move ordering
//! - MVV-LVA capture ordering
//! - Quiescence search to resolve tactical positions
//!
//! The search operates on a read-only snapshot of the game state and
//! is fully isolated from the core engine's game loop.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::eval::{self, DRAW_SCORE, MATE_SCORE, MATE_THRESHOLD};
use crate::movegen;
use crate::types::*;
use crate::zobrist;

// ---------------------------------------------------------------------------
// Search configuration
// ---------------------------------------------------------------------------

/// Default transposition table size in MB.
const DEFAULT_TT_SIZE_MB: usize = 64;

/// Null-move pruning depth reduction.
const NULL_MOVE_REDUCTION: i32 = 3;

/// Maximum search depth (hard ceiling).
const MAX_DEPTH: i32 = 128;

/// Infinity value for alpha-beta bounds.
const INFINITY: i32 = MATE_SCORE + 1;

/// Aspiration window initial width (centipawns).
const ASPIRATION_WINDOW: i32 = 50;

// ---------------------------------------------------------------------------
// Transposition table
// ---------------------------------------------------------------------------

/// Type of bound stored in a TT entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TTFlag {
    /// Exact score (PV node).
    Exact,
    /// Upper bound (all-node, score <= alpha).
    Alpha,
    /// Lower bound (cut-node, score >= beta).
    Beta,
}

/// A single transposition table entry.
#[derive(Debug, Clone, Copy)]
pub struct TTEntry {
    pub key: u64,
    pub depth: i32,
    pub score: i32,
    pub flag: TTFlag,
    pub best_move: Option<EncodedMove>,
}

/// Compact move encoding for TT storage (4 bytes).
#[derive(Debug, Clone, Copy)]
pub struct EncodedMove {
    pub from: u8,      // square index (0–63)
    pub to: u8,        // square index (0–63)
    pub promotion: u8, // 0=none, 1=Q, 2=R, 3=B, 4=N
    pub flags: u8,     // bit 0=castling, bit 1=en passant
}

impl EncodedMove {
    pub fn from_chess_move(mv: &ChessMove) -> Self {
        let promo = match mv.promotion {
            None => 0,
            Some(PieceKind::Queen) => 1,
            Some(PieceKind::Rook) => 2,
            Some(PieceKind::Bishop) => 3,
            Some(PieceKind::Knight) => 4,
            _ => 0,
        };
        let flags = (mv.is_castling as u8) | ((mv.is_en_passant as u8) << 1);
        Self {
            from: (mv.from.rank * 8 + mv.from.file),
            to: (mv.to.rank * 8 + mv.to.file),
            promotion: promo,
            flags,
        }
    }

    pub fn to_chess_move(&self) -> ChessMove {
        let from = Square::new(self.from % 8, self.from / 8);
        let to = Square::new(self.to % 8, self.to / 8);
        let promotion = match self.promotion {
            1 => Some(PieceKind::Queen),
            2 => Some(PieceKind::Rook),
            3 => Some(PieceKind::Bishop),
            4 => Some(PieceKind::Knight),
            _ => None,
        };
        ChessMove {
            from,
            to,
            promotion,
            is_castling: (self.flags & 1) != 0,
            is_en_passant: (self.flags & 2) != 0,
        }
    }
}

/// The transposition table.
pub struct TranspositionTable {
    entries: Vec<Option<TTEntry>>,
    mask: usize,
}

impl TranspositionTable {
    /// Creates a new transposition table with the given size in MB.
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<Option<TTEntry>>();
        let num_entries = (size_mb * 1024 * 1024) / entry_size;
        // Round down to the largest power of 2 that is <= num_entries
        let num_entries = if num_entries.is_power_of_two() {
            num_entries
        } else {
            num_entries.next_power_of_two() / 2
        };
        let num_entries = num_entries.max(1024);

        Self {
            entries: vec![None; num_entries],
            mask: num_entries - 1,
        }
    }

    /// Probes the TT for an entry matching the given hash.
    pub fn probe(&self, key: u64) -> Option<&TTEntry> {
        let index = (key as usize) & self.mask;
        self.entries[index]
            .as_ref()
            .filter(|entry| entry.key == key)
    }

    /// Stores an entry in the TT (always-replace strategy).
    pub fn store(
        &mut self,
        key: u64,
        depth: i32,
        score: i32,
        flag: TTFlag,
        best_move: Option<&ChessMove>,
    ) {
        let index = (key as usize) & self.mask;
        self.entries[index] = Some(TTEntry {
            key,
            depth,
            score,
            flag,
            best_move: best_move.map(EncodedMove::from_chess_move),
        });
    }

    /// Clears the entire table.
    pub fn clear(&mut self) {
        self.entries.fill(None);
    }
}

// ---------------------------------------------------------------------------
// Search position (immutable snapshot for the search)
// ---------------------------------------------------------------------------

/// An immutable snapshot of a chess position for the search engine.
/// Cloned for each child node in the search tree.
#[derive(Clone)]
pub struct SearchPosition {
    pub board: Board,
    pub turn: Color,
    pub castling: CastlingRights,
    pub en_passant: Option<Square>,
    pub hash: u64,
    pub halfmove_clock: u32,
}

impl SearchPosition {
    /// Creates a search position from an existing board state.
    pub fn new(
        board: Board,
        turn: Color,
        castling: CastlingRights,
        en_passant: Option<Square>,
        halfmove_clock: u32,
    ) -> Self {
        let hash = zobrist::hash_position(&board, turn, &castling, en_passant);
        Self {
            board,
            turn,
            castling,
            en_passant,
            hash,
            halfmove_clock,
        }
    }

    /// Generates all legal moves for the current position.
    pub fn legal_moves(&self) -> Vec<ChessMove> {
        movegen::generate_legal_moves(&self.board, self.turn, &self.castling, self.en_passant)
    }

    /// Returns `true` if the side to move is in check.
    pub fn is_in_check(&self) -> bool {
        movegen::is_in_check(&self.board, self.turn)
    }

    /// Makes a move and returns the resulting position.
    pub fn make_move(&self, mv: &ChessMove) -> Self {
        let mut new_board = self.board.clone();
        let moving_piece = new_board.get(mv.from).unwrap();
        let is_pawn_move = moving_piece.kind == PieceKind::Pawn;
        let is_capture = new_board.get(mv.to).is_some() || mv.is_en_passant;

        movegen::apply_move_to_board(&mut new_board, mv, self.turn);

        // Update castling rights
        let mut new_castling = self.castling;
        // Check king moves
        if moving_piece.kind == PieceKind::King {
            let rights = new_castling.for_color_mut(self.turn);
            rights.kingside = false;
            rights.queenside = false;
        }
        // Check rook squares
        Self::update_rook_castling(mv.from, &mut new_castling);
        Self::update_rook_castling(mv.to, &mut new_castling);

        // Update en passant
        let new_ep = if is_pawn_move {
            let rank_diff = (mv.to.rank as i8 - mv.from.rank as i8).abs();
            if rank_diff == 2 {
                let ep_rank = (mv.from.rank as i8 + self.turn.pawn_direction()) as u8;
                Some(Square::new(mv.from.file, ep_rank))
            } else {
                None
            }
        } else {
            None
        };

        // Update halfmove clock
        let new_halfmove = if is_pawn_move || is_capture {
            0
        } else {
            self.halfmove_clock + 1
        };

        let new_turn = self.turn.opponent();
        let new_hash = zobrist::hash_position(&new_board, new_turn, &new_castling, new_ep);

        Self {
            board: new_board,
            turn: new_turn,
            castling: new_castling,
            en_passant: new_ep,
            hash: new_hash,
            halfmove_clock: new_halfmove,
        }
    }

    /// Makes a null move (pass — switches turn without moving).
    pub fn make_null_move(&self) -> Self {
        let new_turn = self.turn.opponent();
        let new_hash = zobrist::hash_position(&self.board, new_turn, &self.castling, None);
        Self {
            board: self.board.clone(),
            turn: new_turn,
            castling: self.castling,
            en_passant: None,
            hash: new_hash,
            halfmove_clock: self.halfmove_clock + 1,
        }
    }

    fn update_rook_castling(sq: Square, castling: &mut CastlingRights) {
        if sq == Square::new(7, 0) {
            castling.white.kingside = false;
        }
        if sq == Square::new(0, 0) {
            castling.white.queenside = false;
        }
        if sq == Square::new(7, 7) {
            castling.black.kingside = false;
        }
        if sq == Square::new(0, 7) {
            castling.black.queenside = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Search statistics
// ---------------------------------------------------------------------------

/// Statistics collected during a search.
#[derive(Debug, Clone, Default)]
pub struct SearchStats {
    pub nodes: u64,
    pub tt_hits: u64,
    pub tt_cutoffs: u64,
    pub null_cutoffs: u64,
    pub lmr_searches: u64,
    pub beta_cutoffs: u64,
    pub quiescence_nodes: u64,
}

// ---------------------------------------------------------------------------
// Search result
// ---------------------------------------------------------------------------

/// The result of a completed search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The best move found.
    pub best_move: Option<ChessMove>,
    /// Evaluation score (centipawns, from the searching side's perspective).
    pub score: i32,
    /// The search depth achieved.
    pub depth: i32,
    /// The principal variation (best line of play).
    pub pv: Vec<ChessMove>,
    /// Search statistics.
    pub stats: SearchStats,
    /// Total time spent searching (milliseconds).
    pub time_ms: u64,
}

// ---------------------------------------------------------------------------
// Move ordering
// ---------------------------------------------------------------------------

/// MVV-LVA (Most Valuable Victim – Least Valuable Attacker) score.
fn mvv_lva_score(board: &Board, mv: &ChessMove) -> i32 {
    let victim_value = board.get(mv.to).map(|p| piece_value(p.kind)).unwrap_or(0);
    let attacker_value = board.get(mv.from).map(|p| piece_value(p.kind)).unwrap_or(0);
    victim_value * 10 - attacker_value
}

/// Simple piece value for move ordering.
fn piece_value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => 1,
        PieceKind::Knight => 3,
        PieceKind::Bishop => 3,
        PieceKind::Rook => 5,
        PieceKind::Queen => 9,
        PieceKind::King => 100,
    }
}

/// Orders moves for optimal alpha-beta pruning.
///
/// Priority:
/// 1. TT best move (score = 10_000_000)
/// 2. Captures ordered by MVV-LVA (score = 1_000_000 + mvv_lva)
/// 3. Killer moves (score = 900_000)
/// 4. Quiet moves by history heuristic
fn score_moves(
    moves: &[ChessMove],
    board: &Board,
    tt_move: Option<&ChessMove>,
    killers: &[Option<ChessMove>; 2],
    history: &[[i32; 64]; 64],
) -> Vec<(ChessMove, i32)> {
    moves
        .iter()
        .map(|mv| {
            let score = if tt_move.is_some_and(|tm| tm == mv) {
                10_000_000
            } else if board.get(mv.to).is_some() || mv.is_en_passant {
                // Capture
                1_000_000 + mvv_lva_score(board, mv)
            } else if killers[0].as_ref().is_some_and(|k| k == mv) {
                900_000
            } else if killers[1].as_ref().is_some_and(|k| k == mv) {
                899_000
            } else {
                // History heuristic
                history[mv.from.index()][mv.to.index()]
            };
            (*mv, score)
        })
        .collect()
}

/// Sort scored moves in descending order.
fn sort_moves(scored: &mut [(ChessMove, i32)]) {
    scored.sort_unstable_by(|a, b| b.1.cmp(&a.1));
}

// ---------------------------------------------------------------------------
// Search engine
// ---------------------------------------------------------------------------

/// The main search engine.
pub struct SearchEngine {
    pub tt: TranspositionTable,
    /// Killer moves per ply (2 slots per ply).
    killers: Vec<[Option<ChessMove>; 2]>,
    /// History heuristic table: `[from_sq][to_sq] -> score`.
    history: [[i32; 64]; 64],
    /// Search statistics for the current search.
    pub stats: SearchStats,
    /// Cancellation flag — set to `true` to abort the search.
    pub abort: Arc<AtomicBool>,
}

impl SearchEngine {
    /// Creates a new search engine with the given TT size.
    pub fn new(tt_size_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::new(tt_size_mb),
            killers: vec![[None; 2]; MAX_DEPTH as usize],
            history: [[0i32; 64]; 64],
            stats: SearchStats::default(),
            abort: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Creates a new search engine with default TT size.
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_TT_SIZE_MB)
    }

    /// Runs iterative deepening search to the specified depth.
    ///
    /// Returns the best move and evaluation at the target depth.
    pub fn search(&mut self, pos: &SearchPosition, max_depth: i32) -> SearchResult {
        let max_depth = max_depth.clamp(1, MAX_DEPTH);
        let start = Instant::now();
        self.stats = SearchStats::default();
        self.abort.store(false, Ordering::Relaxed);

        // Clear killer and history tables
        for k in &mut self.killers {
            *k = [None; 2];
        }
        self.history = [[0; 64]; 64];

        let mut best_move: Option<ChessMove> = None;
        let mut best_score = -INFINITY;
        let mut best_pv: Vec<ChessMove> = Vec::new();
        let mut last_good_depth = 0;

        // Iterative deepening
        for depth in 1..=max_depth {
            if self.abort.load(Ordering::Relaxed) {
                break;
            }

            let score;
            if depth <= 4 || best_score.abs() > MATE_THRESHOLD {
                // Simple window for shallow depths or near-mate scores
                score = self.alpha_beta(pos, depth, -INFINITY, INFINITY, 0, true);
            } else {
                // Aspiration windows for deeper searches
                let mut delta = ASPIRATION_WINDOW;
                let mut alpha = best_score - delta;
                let mut beta = best_score + delta;
                let mut found_score = None;

                loop {
                    let s = self.alpha_beta(pos, depth, alpha, beta, 0, true);
                    if self.abort.load(Ordering::Relaxed) {
                        break;
                    }
                    if s <= alpha {
                        alpha = (s - delta).max(-INFINITY);
                        delta *= 2;
                    } else if s >= beta {
                        beta = (s + delta).min(INFINITY);
                        delta *= 2;
                    } else {
                        found_score = Some(s);
                        break;
                    }
                    if delta > 2000 {
                        // Fallback to full window
                        found_score =
                            Some(self.alpha_beta(pos, depth, -INFINITY, INFINITY, 0, true));
                        break;
                    }
                }
                // Use found score, or fall back to TT / previous best
                score = found_score.unwrap_or_else(|| {
                    self.tt
                        .probe(pos.hash)
                        .map(|e| e.score)
                        .unwrap_or(best_score)
                });
            }

            if self.abort.load(Ordering::Relaxed) {
                break;
            }

            best_score = score;
            last_good_depth = depth;

            // Extract PV from TT
            let pv = self.extract_pv(pos, depth);
            if let Some(first) = pv.first() {
                best_move = Some(*first);
                best_pv = pv;
            }

            log::trace!(
                "depth {} score {} pv {} nodes {} time {}ms",
                depth,
                score,
                best_pv
                    .iter()
                    .map(|m| m.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
                self.stats.nodes,
                start.elapsed().as_millis()
            );
        }

        let elapsed = start.elapsed();

        SearchResult {
            best_move,
            score: best_score,
            depth: last_good_depth,
            pv: best_pv,
            stats: self.stats.clone(),
            time_ms: elapsed.as_millis() as u64,
        }
    }

    /// Principal Variation Search (alpha-beta with PVS enhancements).
    fn alpha_beta(
        &mut self,
        pos: &SearchPosition,
        depth: i32,
        mut alpha: i32,
        beta: i32,
        ply: i32,
        is_pv: bool,
    ) -> i32 {
        // Check for cancellation
        if self.abort.load(Ordering::Relaxed) {
            return 0;
        }

        self.stats.nodes += 1;

        // Hard ply ceiling to prevent out-of-bounds access on killers table
        if ply >= MAX_DEPTH {
            return eval::evaluate(&pos.board, pos.turn);
        }

        // Depth exhausted → quiescence search
        if depth <= 0 {
            return self.quiescence(pos, alpha, beta, ply);
        }

        // Draw detection: 50-move rule check
        if pos.halfmove_clock >= 100 {
            return DRAW_SCORE;
        }

        // Probe transposition table
        let tt_move: Option<ChessMove>;
        if let Some(entry) = self.tt.probe(pos.hash) {
            self.stats.tt_hits += 1;
            tt_move = entry.best_move.map(|em| em.to_chess_move());

            if !is_pv && entry.depth >= depth {
                // Denormalize mate scores: stored as distance from root,
                // adjust back to distance from the current ply.
                let tt_score = if entry.score > MATE_THRESHOLD {
                    entry.score - ply
                } else if entry.score < -MATE_THRESHOLD {
                    entry.score + ply
                } else {
                    entry.score
                };
                match entry.flag {
                    TTFlag::Exact => {
                        self.stats.tt_cutoffs += 1;
                        return tt_score;
                    }
                    TTFlag::Beta => {
                        if tt_score >= beta {
                            self.stats.tt_cutoffs += 1;
                            return tt_score;
                        }
                    }
                    TTFlag::Alpha => {
                        if tt_score <= alpha {
                            self.stats.tt_cutoffs += 1;
                            return tt_score;
                        }
                    }
                }
            }
        } else {
            tt_move = None;
        }

        let in_check = pos.is_in_check();

        // Null-move pruning
        // Conditions: not in check, not PV, depth >= 3, has non-pawn material
        if !in_check && !is_pv && depth >= 3 && has_non_pawn_material(pos) {
            let null_pos = pos.make_null_move();
            let null_score = -self.alpha_beta(
                &null_pos,
                depth - 1 - NULL_MOVE_REDUCTION,
                -beta,
                -beta + 1,
                ply + 1,
                false,
            );
            if null_score >= beta {
                self.stats.null_cutoffs += 1;
                return beta;
            }
        }

        // Generate and order moves
        let moves = pos.legal_moves();

        // Checkmate / stalemate
        if moves.is_empty() {
            if in_check {
                // Checkmate — return negative mate score, adjusted for ply
                return -MATE_SCORE + ply;
            } else {
                // Stalemate
                return DRAW_SCORE;
            }
        }

        let killers = &self.killers[ply as usize];
        let mut scored = score_moves(&moves, &pos.board, tt_move.as_ref(), killers, &self.history);
        sort_moves(&mut scored);

        let mut best_score = -INFINITY;
        let mut best_move: Option<ChessMove> = None;
        let mut flag = TTFlag::Alpha;

        for (i, &(mv, _)) in scored.iter().enumerate() {
            let child = pos.make_move(&mv);
            let is_capture = pos.board.get(mv.to).is_some() || mv.is_en_passant;
            let gives_check = child.is_in_check();

            let mut score;

            // Check extension: extend search by 1 ply if the move gives check
            let extension = if gives_check { 1 } else { 0 };

            if i == 0 {
                // First move: search with full window
                score =
                    -self.alpha_beta(&child, depth - 1 + extension, -beta, -alpha, ply + 1, is_pv);
            } else {
                // Late Move Reductions
                let mut reduction = 0;
                if depth >= 3
                    && !in_check
                    && !is_capture
                    && mv.promotion.is_none()
                    && !gives_check
                    && i >= 4
                {
                    // LMR: reduce depth for late, non-tactical moves
                    reduction = 1 + (i as i32 / 8);
                    reduction = reduction.min(depth - 1);
                    self.stats.lmr_searches += 1;
                }

                // Zero-window search (PVS)
                score = -self.alpha_beta(
                    &child,
                    depth - 1 - reduction + extension,
                    -alpha - 1,
                    -alpha,
                    ply + 1,
                    false,
                );

                // Re-search with full window if ZWS failed high
                if score > alpha && (reduction > 0 || !is_pv) {
                    score = -self.alpha_beta(
                        &child,
                        depth - 1 + extension,
                        -beta,
                        -alpha,
                        ply + 1,
                        is_pv,
                    );
                }
            }

            if self.abort.load(Ordering::Relaxed) {
                return 0;
            }

            if score > best_score {
                best_score = score;
                best_move = Some(mv);

                if score > alpha {
                    alpha = score;
                    flag = TTFlag::Exact;

                    if score >= beta {
                        // Beta cutoff
                        self.stats.beta_cutoffs += 1;
                        flag = TTFlag::Beta;

                        // Update killer moves (non-captures only)
                        if !is_capture {
                            let ply_idx = ply as usize;
                            if self.killers[ply_idx][0] != Some(mv) {
                                self.killers[ply_idx][1] = self.killers[ply_idx][0];
                                self.killers[ply_idx][0] = Some(mv);
                            }

                            // Update history heuristic
                            self.history[mv.from.index()][mv.to.index()] += depth * depth;
                        }

                        break;
                    }
                }
            }
        }

        // Store in TT — normalize mate scores to be ply-independent
        // (relative to the node being stored, not the root).
        let tt_score = if best_score > MATE_THRESHOLD {
            best_score + ply
        } else if best_score < -MATE_THRESHOLD {
            best_score - ply
        } else {
            best_score
        };
        self.tt
            .store(pos.hash, depth, tt_score, flag, best_move.as_ref());

        best_score
    }

    /// Quiescence search: only searches captures to resolve tactical positions.
    #[allow(clippy::only_used_in_recursion)]
    fn quiescence(&mut self, pos: &SearchPosition, mut alpha: i32, beta: i32, ply: i32) -> i32 {
        if self.abort.load(Ordering::Relaxed) {
            return 0;
        }

        self.stats.quiescence_nodes += 1;
        self.stats.nodes += 1;

        // Stand pat: static evaluation
        let stand_pat = eval::evaluate(&pos.board, pos.turn);

        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }

        // Delta pruning: if even a queen capture can't beat alpha, skip
        if stand_pat + 1025 < alpha {
            return alpha;
        }

        // Generate all legal moves, then filter to captures
        let all_moves = pos.legal_moves();
        let captures: Vec<ChessMove> = all_moves
            .into_iter()
            .filter(|mv| {
                pos.board.get(mv.to).is_some() || mv.is_en_passant || mv.promotion.is_some()
            })
            .collect();

        // Order captures by MVV-LVA
        let mut scored: Vec<(ChessMove, i32)> = captures
            .iter()
            .map(|mv| (*mv, mvv_lva_score(&pos.board, mv)))
            .collect();
        scored.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        for (mv, _) in scored {
            let child = pos.make_move(&mv);
            let score = -self.quiescence(&child, -beta, -alpha, ply + 1);

            if self.abort.load(Ordering::Relaxed) {
                return 0;
            }

            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }

        alpha
    }

    /// Extracts the principal variation from the transposition table.
    fn extract_pv(&self, pos: &SearchPosition, max_depth: i32) -> Vec<ChessMove> {
        let mut pv = Vec::new();
        let mut current = pos.clone();
        let mut depth = 0;

        while depth < max_depth {
            if let Some(entry) = self.tt.probe(current.hash)
                && let Some(encoded_move) = entry.best_move
            {
                let mv = encoded_move.to_chess_move();
                // Verify the move is legal in the current position
                let legal = current.legal_moves();
                if legal
                    .iter()
                    .any(|lm| lm.from == mv.from && lm.to == mv.to && lm.promotion == mv.promotion)
                {
                    // Find the full move (with flags) from legal moves
                    let full_mv = legal
                        .iter()
                        .find(|lm| {
                            lm.from == mv.from && lm.to == mv.to && lm.promotion == mv.promotion
                        })
                        .unwrap();
                    pv.push(*full_mv);
                    current = current.make_move(full_mv);
                    depth += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        pv
    }
}

/// Checks if the side to move has any non-pawn, non-king material.
/// Used for null-move pruning safety.
fn has_non_pawn_material(pos: &SearchPosition) -> bool {
    for rank in 0..8u8 {
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            if let Some(piece) = pos.board.get(sq)
                && piece.color == pos.turn
                && piece.kind != PieceKind::Pawn
                && piece.kind != PieceKind::King
            {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn starting_pos() -> SearchPosition {
        SearchPosition::new(
            Board::starting_position(),
            Color::White,
            CastlingRights::default(),
            None,
            0,
        )
    }

    #[test]
    fn test_depth_1_search_finds_move() {
        let pos = starting_pos();
        let mut engine = SearchEngine::with_defaults();
        let result = engine.search(&pos, 1);
        assert!(result.best_move.is_some(), "Should find a move at depth 1");
    }

    #[test]
    fn test_depth_5_search() {
        let pos = starting_pos();
        let mut engine = SearchEngine::with_defaults();
        let result = engine.search(&pos, 5);
        assert!(result.best_move.is_some());
        assert_eq!(result.depth, 5);
    }

    #[test]
    fn test_checkmate_detection() {
        // Fool's mate position: after 1. f3 e5 2. g4
        let mut board = Board::starting_position();
        // Simulate: f2-f3, e7-e5, g2-g4
        movegen::apply_move_to_board(
            &mut board,
            &ChessMove::simple(Square::new(5, 1), Square::new(5, 2)),
            Color::White,
        );
        movegen::apply_move_to_board(
            &mut board,
            &ChessMove::simple(Square::new(4, 6), Square::new(4, 4)),
            Color::Black,
        );
        movegen::apply_move_to_board(
            &mut board,
            &ChessMove::simple(Square::new(6, 1), Square::new(6, 3)),
            Color::White,
        );

        let pos = SearchPosition::new(board, Color::Black, CastlingRights::default(), None, 0);
        let mut engine = SearchEngine::with_defaults();
        let result = engine.search(&pos, 3);

        // Black should find Qh4# (mate in 1)
        assert!(result.best_move.is_some());
        let mv = result.best_move.unwrap();
        assert_eq!(mv.to, Square::new(7, 3), "Best move should be Qh4#");
        assert!(
            result.score > MATE_THRESHOLD,
            "Score should indicate mate, got {}",
            result.score
        );
    }

    #[test]
    fn test_tt_basic() {
        let mut tt = TranspositionTable::new(1);
        let key = 0x12345678;
        tt.store(key, 5, 100, TTFlag::Exact, None);
        let entry = tt.probe(key);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().score, 100);
        assert_eq!(entry.unwrap().depth, 5);
    }
}
