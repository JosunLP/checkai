//! Alpha-beta search engine for the CheckAI analysis module.
//!
//! Implements a full-featured chess search with:
//! - Iterative deepening with aspiration windows
//! - Principal Variation Search (PVS / Negascout) with proper re-searches
//! - Transposition table with generation-based aging and depth-preferred
//!   replacement, probed and updated in both the main search and quiescence
//! - Hard time/node limits enforced *inside* the tree (checked every
//!   [`NODE_CHECK_INTERVAL`] nodes), with partial iterations discarded
//! - In-tree repetition detection (a single repetition along the search
//!   path or against the supplied game history scores as a draw) and
//!   50-move-rule awareness with mate precedence
//! - Mate-distance pruning
//! - Reverse futility pruning (static null move)
//! - Adaptive null-move pruning with verification search at high depth
//! - Razoring and classic futility pruning at frontier nodes
//! - Late Move Reductions driven by a precomputed log-log table, adjusted
//!   for PV nodes, killers and history
//! - Late Move Pruning of late quiets at shallow depth
//! - Internal Iterative Reduction when no TT move is available
//! - Check extensions (capped by ply)
//! - Killer move, counter-move, and capped history heuristics (with
//!   gravity-style aging and maluses for failed quiets)
//! - MVV-LVA + Static Exchange Evaluation (SEE) capture ordering and pruning
//! - Quiescence search with stand-pat, per-capture delta pruning, SEE
//!   pruning, TT integration, and check-evasion handling
//!
//! The search operates on a read-only snapshot of the game state and
//! is fully isolated from the core engine's game loop.

use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use crate::engine_time::{Duration, Instant, THREADS_SUPPORTED, epoch_nanos};
use crate::eval::{self, DRAW_SCORE, MATE_SCORE, MATE_THRESHOLD};
use crate::movegen;
use crate::opening_book::OpeningBook;
use crate::tablebase::{SyzygyTablebase, TablebaseInfo};
use crate::types::*;
use crate::zobrist;

// ---------------------------------------------------------------------------
// Search configuration
// ---------------------------------------------------------------------------

/// Default transposition table size in MB.
const DEFAULT_TT_SIZE_MB: usize = 64;

/// Maximum search depth (hard ceiling).
pub const MAX_DEPTH: i32 = 128;

/// Highest interactive skill level accepted by [`SearchLimits::for_level`].
pub const MAX_SKILL_LEVEL: u8 = 10;

/// Maximum number of Lazy SMP search threads.
pub const MAX_THREADS: usize = 64;

/// Maximum number of principal variations the root search can report.
pub const MAX_MULTI_PV: usize = 16;

/// Full-strength value of [`EngineConfig::skill_level`].
pub const MAX_SKILL: u8 = 20;

/// Percentage of the hard time budget after which no new iterative-deepening
/// iteration is started (the "soft" limit).
const SOFT_TIME_NUMERATOR: u64 = 55;

/// The soft limit is stretched by this percentage while the best root move
/// keeps changing, so unstable positions get more thinking time.
const UNSTABLE_TIME_BONUS: u64 = 65;

/// Consecutive iterations with an unchanged best move after which the soft
/// limit is *shortened* — the answer is not going to change any more.
const STABLE_ITERATIONS: u32 = 5;

/// Percentage the soft limit shrinks to once the best move is stable.
const STABLE_TIME_FACTOR: u64 = 70;

/// Infinity value for alpha-beta bounds.
const INFINITY: i32 = MATE_SCORE + 1;

/// Aspiration window initial width (centipawns).
const ASPIRATION_WINDOW: i32 = 50;

/// Once the aspiration window has widened past this many centipawns the
/// search falls back to a full window instead of widening further.
const ASPIRATION_GIVE_UP: i32 = 700;

/// How often (in nodes) the in-tree hard time/node limit is re-checked.
/// Must be a power of two; the check triggers when
/// `nodes & (NODE_CHECK_INTERVAL - 1) == 0`.
const NODE_CHECK_INTERVAL: u64 = 2048;

/// Tighter re-check interval used while a node budget is active.
///
/// What makes the check expensive is [`Instant::now`], not the node count, so
/// a search under `--nodes` can afford to poll far more often. Every thread
/// may overshoot by up to one interval, so the coarse value alone let a
/// 32-thread search run tens of thousands of nodes past its budget — useless
/// for the reproducible, node-limited runs the flag exists for.
/// Must be a power of two and divide [`NODE_CHECK_INTERVAL`].
const NODE_LIMIT_CHECK_INTERVAL: u64 = 128;

/// Futility pruning margins (indexed by depth remaining).
/// At depth `d` (1..=3) quiet moves are skipped when
/// `static_eval + FUTILITY_MARGINS[d] <= alpha`.
const FUTILITY_MARGINS: [i32; 4] = [0, 200, 400, 600];

/// Reverse futility pruning (static null move): at shallow non-PV nodes,
/// if `static_eval - RFP_MARGIN_PER_DEPTH * depth >= beta` the node is
/// assumed to fail high and the static eval is returned immediately.
const RFP_MARGIN_PER_DEPTH: i32 = 90;

/// Maximum depth at which reverse futility pruning applies.
const RFP_MAX_DEPTH: i32 = 7;

/// Razoring margin: if static eval + RAZORING_MARGIN < alpha at depth 1-2,
/// drop into quiescence search directly.
const RAZORING_MARGIN: i32 = 300;

/// Late-move pruning thresholds indexed by depth (max quiet moves to search).
const LMP_THRESHOLDS: [usize; 5] = [0, 5, 8, 13, 20];

/// Null-move pruning: base depth reduction. The effective reduction is
/// `NULL_MOVE_BASE_REDUCTION + depth / 4 + min(2, (static_eval - beta) / 200)`.
const NULL_MOVE_BASE_REDUCTION: i32 = 3;

/// Minimum remaining depth at which a null-move fail-high is verified with
/// a reduced-depth search (zugzwang guard at high depths).
const NULL_MOVE_VERIFICATION_DEPTH: i32 = 10;

/// Minimum depth for Internal Iterative Reduction: when no TT move is
/// available at `depth >= IIR_MIN_DEPTH`, the search depth is reduced by
/// one ply (move ordering is poor, so the subtree is cheapened; the TT
/// fills up and a later, deeper visit re-searches with a good move first).
const IIR_MIN_DEPTH: i32 = 4;

/// Maximum depth at which losing captures (SEE < 0) are pruned outright
/// at non-PV nodes.
const SEE_PRUNE_MAX_DEPTH: i32 = 3;

/// Quiescence delta pruning margin: a capture is skipped when even
/// `stand_pat + victim_value + QS_DELTA_MARGIN` cannot reach alpha.
const QS_DELTA_MARGIN: i32 = 200;

/// History scores are kept within `[-HISTORY_MAX, HISTORY_MAX]` by the
/// gravity-style update formula (see [`update_history`]).
const HISTORY_MAX: i32 = 16_384;

/// Minimum remaining depth at which singular extensions are attempted.
const SINGULAR_MIN_DEPTH: i32 = 8;

/// The TT entry backing a singular verification must be at least this close
/// to the current depth to be trusted.
const SINGULAR_TT_DEPTH_MARGIN: i32 = 3;

/// Singular margin per ply of remaining depth: the verification search runs
/// below `tt_score - SINGULAR_MARGIN_PER_DEPTH * depth`.
const SINGULAR_MARGIN_PER_DEPTH: i32 = 2;

/// Quiet moves whose combined history score falls below
/// `-HISTORY_PRUNE_PER_DEPTH * depth` are pruned at shallow non-PV nodes.
const HISTORY_PRUNE_PER_DEPTH: i32 = 2_000;

/// Maximum depth at which history-based pruning of quiets applies.
const HISTORY_PRUNE_MAX_DEPTH: i32 = 5;

/// Number of distinct `(color, piece kind)` combinations, used to index the
/// continuation-history table.
const PIECE_SLOTS: usize = 12;

/// Base term of the LMR reduction formula.
const LMR_BASE: f64 = 0.75;

/// Divisor of the `ln(depth) * ln(move_number)` term of the LMR formula.
const LMR_DIVISOR: f64 = 2.25;

/// Precomputed Late Move Reduction table:
/// `LMR_TABLE[depth.min(63)][move_number.min(63)]`
/// = `LMR_BASE + ln(depth) * ln(move_number) / LMR_DIVISOR` (floored, >= 0).
static LMR_TABLE: LazyLock<[[u8; 64]; 64]> = LazyLock::new(|| {
    let mut table = [[0u8; 64]; 64];
    for (d, row) in table.iter_mut().enumerate().skip(1) {
        for (m, cell) in row.iter_mut().enumerate().skip(1) {
            let r = LMR_BASE + (d as f64).ln() * (m as f64).ln() / LMR_DIVISOR;
            *cell = r.max(0.0) as u8;
        }
    }
    table
});

// Move-ordering score tiers (descending priority).

/// Ordering score for the transposition-table move.
const ORDER_TT_MOVE: i32 = 10_000_000;

/// Base ordering score for queen promotions (tried right after good captures).
const ORDER_PROMOTION: i32 = 1_100_000;

/// Base ordering score for good captures (SEE >= 0), plus MVV-LVA.
const ORDER_GOOD_CAPTURE: i32 = 1_000_000;

/// Ordering score for the first killer move.
const ORDER_KILLER_0: i32 = 900_000;

/// Ordering score for the second killer move.
const ORDER_KILLER_1: i32 = 899_000;

/// Ordering score for the counter-move of the opponent's previous move.
const ORDER_COUNTER_MOVE: i32 = 898_000;

/// Base ordering score for losing captures (SEE < 0); tried after quiets.
const ORDER_BAD_CAPTURE: i32 = -1_000_000;

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

impl TTFlag {
    /// Two-bit representation used inside a packed TT word.
    #[inline]
    const fn to_bits(self) -> u64 {
        match self {
            TTFlag::Exact => 0,
            TTFlag::Alpha => 1,
            TTFlag::Beta => 2,
        }
    }

    /// Inverse of [`TTFlag::to_bits`].
    #[inline]
    const fn from_bits(bits: u64) -> Self {
        match bits {
            0 => TTFlag::Exact,
            1 => TTFlag::Alpha,
            _ => TTFlag::Beta,
        }
    }
}

/// Sentinel marking a TT entry that carries no cached static evaluation.
pub const TT_EVAL_NONE: i32 = i32::MIN + 1;

/// Packed representation of [`TT_EVAL_NONE`] inside a TT word.
const TT_EVAL_NONE_PACKED: i16 = i16::MIN;

/// A single transposition table entry (decoded form).
#[derive(Debug, Clone, Copy)]
pub struct TTEntry {
    pub key: u64,
    pub depth: i32,
    pub score: i32,
    pub flag: TTFlag,
    pub best_move: Option<EncodedMove>,
    /// Cached static evaluation of the position ([`TT_EVAL_NONE`] if absent).
    pub static_eval: i32,
    /// Search generation that wrote this entry (used for aging).
    pub generation: u8,
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

    /// Packs the move into the 17 low bits of a `u64`.
    ///
    /// `0` is reserved for "no move": it would decode to the impossible
    /// move a1→a1 without promotion or flags.
    #[inline]
    const fn to_bits(self) -> u64 {
        (self.from as u64)
            | ((self.to as u64) << 6)
            | ((self.promotion as u64) << 12)
            | ((self.flags as u64) << 15)
    }

    /// Inverse of [`EncodedMove::to_bits`]; `None` for the reserved `0`.
    #[inline]
    const fn from_bits(bits: u64) -> Option<Self> {
        if bits == 0 {
            return None;
        }
        Some(Self {
            from: (bits & 0x3F) as u8,
            to: ((bits >> 6) & 0x3F) as u8,
            promotion: ((bits >> 12) & 0x07) as u8,
            flags: ((bits >> 15) & 0x03) as u8,
        })
    }
}

/// One 16-byte transposition table slot, stored as two atomic words so the
/// table can be shared between Lazy SMP search threads without locking.
#[derive(Default)]
struct TtSlot {
    /// `zobrist_key ^ data` — the classic lockless XOR checksum. A slot torn
    /// by a concurrent write fails the check and is simply treated as a miss.
    key: AtomicU64,
    /// Packed payload (see [`TranspositionTable::pack`]).
    data: AtomicU64,
}

/// Number of slots sampled by [`TranspositionTable::hashfull`].
const HASHFULL_SAMPLE: usize = 1000;

/// The transposition table.
///
/// Single-slot, power-of-two sized, with a generation counter for aging:
/// entries written by older searches are always evictable, while within
/// the current generation deeper entries are preferred (an entry is only
/// kept if it is from this generation and more than one ply deeper than
/// the incoming one). Same-key stores always update, but retain the old
/// best move when the new store has none.
///
/// Every operation takes `&self` and uses relaxed atomics, so a single table
/// can be wrapped in an [`Arc`] and shared by all Lazy SMP threads. Races are
/// benign: a torn slot fails the XOR checksum and reads as a miss, which only
/// ever costs a re-search, never correctness.
///
/// # Entry layout
///
/// | Bits    | Field                                        |
/// |---------|----------------------------------------------|
/// | 0–15    | score (`i16`)                                |
/// | 16–31   | static eval (`i16`, `i16::MIN` = none)       |
/// | 32–48   | best move (17 bits, `0` = none)              |
/// | 49–56   | depth (`u8`)                                 |
/// | 57–58   | bound flag                                   |
/// | 59–63   | generation (5 bits, wrapping)                |
pub struct TranspositionTable {
    entries: Vec<TtSlot>,
    mask: usize,
    /// Current search generation; bumped via [`TranspositionTable::new_generation`].
    generation: AtomicU8,
}

impl TranspositionTable {
    /// Creates a new transposition table with the given size in MB.
    pub fn new(size_mb: usize) -> Self {
        const SLOT_BYTES: usize = std::mem::size_of::<u64>() * 2;
        let num_entries = size_mb.saturating_mul(1024 * 1024) / SLOT_BYTES;
        // Round down to the largest power of 2 that is <= num_entries
        let num_entries = if num_entries.is_power_of_two() {
            num_entries
        } else {
            num_entries.next_power_of_two() / 2
        };
        let num_entries = num_entries.max(1024);

        Self {
            entries: (0..num_entries).map(|_| TtSlot::default()).collect(),
            mask: num_entries - 1,
            generation: AtomicU8::new(0),
        }
    }

    /// Number of slots in the table.
    pub fn capacity(&self) -> usize {
        self.entries.len()
    }

    /// Advances the table to a new search generation.
    ///
    /// Called once per search; entries from previous generations become
    /// freely replaceable, implementing a cheap aging scheme.
    pub fn new_generation(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Estimated fill level in per mille, sampled over the first
    /// [`HASHFULL_SAMPLE`] slots (the UCI `hashfull` statistic).
    pub fn hashfull(&self) -> u32 {
        let generation = self.generation.load(Ordering::Relaxed) & 0x1F;
        let sample = HASHFULL_SAMPLE.min(self.entries.len());
        if sample == 0 {
            return 0;
        }
        let used = self.entries[..sample]
            .iter()
            .filter(|slot| {
                let data = slot.data.load(Ordering::Relaxed);
                data != 0 && ((data >> 59) & 0x1F) as u8 == generation
            })
            .count();
        (used * 1000 / sample) as u32
    }

    /// Packs an entry payload into a single 64-bit word.
    #[inline]
    fn pack(
        depth: i32,
        score: i32,
        static_eval: i32,
        flag: TTFlag,
        move_bits: u64,
        generation: u8,
    ) -> u64 {
        let score = score.clamp(i16::MIN as i32 + 1, i16::MAX as i32) as i16;
        let eval = if static_eval == TT_EVAL_NONE {
            TT_EVAL_NONE_PACKED
        } else {
            static_eval.clamp(i16::MIN as i32 + 1, i16::MAX as i32) as i16
        };
        (score as u16 as u64)
            | ((eval as u16 as u64) << 16)
            | ((move_bits & 0x1_FFFF) << 32)
            | ((depth.clamp(0, u8::MAX as i32) as u64) << 49)
            | (flag.to_bits() << 57)
            | (((generation & 0x1F) as u64) << 59)
    }

    /// Decodes a packed payload back into a [`TTEntry`].
    #[inline]
    fn unpack(key: u64, data: u64) -> TTEntry {
        let eval = ((data >> 16) & 0xFFFF) as u16 as i16;
        TTEntry {
            key,
            depth: ((data >> 49) & 0xFF) as i32,
            score: (data & 0xFFFF) as u16 as i16 as i32,
            flag: TTFlag::from_bits((data >> 57) & 0x3),
            best_move: EncodedMove::from_bits((data >> 32) & 0x1_FFFF),
            static_eval: if eval == TT_EVAL_NONE_PACKED {
                TT_EVAL_NONE
            } else {
                i32::from(eval)
            },
            generation: ((data >> 59) & 0x1F) as u8,
        }
    }

    /// Probes the TT for an entry matching the given hash.
    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        let slot = &self.entries[(key as usize) & self.mask];
        let stored = slot.key.load(Ordering::Relaxed);
        let data = slot.data.load(Ordering::Relaxed);
        if stored ^ data != key {
            return None;
        }
        Some(Self::unpack(key, data))
    }

    /// Stores an entry without a cached static evaluation.
    ///
    /// Convenience wrapper around [`TranspositionTable::store_with_eval`].
    pub fn store(
        &self,
        key: u64,
        depth: i32,
        score: i32,
        flag: TTFlag,
        best_move: Option<&ChessMove>,
    ) {
        self.store_with_eval(key, depth, score, flag, best_move, TT_EVAL_NONE);
    }

    /// Stores an entry using the depth-preferred + aging replacement scheme.
    pub fn store_with_eval(
        &self,
        key: u64,
        depth: i32,
        score: i32,
        flag: TTFlag,
        best_move: Option<&ChessMove>,
        static_eval: i32,
    ) {
        let slot = &self.entries[(key as usize) & self.mask];
        let generation = self.generation.load(Ordering::Relaxed);
        let mut move_bits = best_move
            .map(|mv| EncodedMove::from_chess_move(mv).to_bits())
            .unwrap_or(0);

        let stored = slot.key.load(Ordering::Relaxed);
        let existing = slot.data.load(Ordering::Relaxed);
        if stored ^ existing == key {
            // Same position: always refresh, but never lose a known move.
            if move_bits == 0 {
                move_bits = (existing >> 32) & 0x1_FFFF;
            }
        } else if existing != 0 {
            let existing_gen = ((existing >> 59) & 0x1F) as u8;
            let existing_depth = ((existing >> 49) & 0xFF) as i32;
            if existing_gen == generation & 0x1F && existing_depth > depth + 1 {
                // Different position, current generation, clearly deeper:
                // keep the more valuable existing entry.
                return;
            }
        }

        let data = Self::pack(depth, score, static_eval, flag, move_bits, generation);
        slot.data.store(data, Ordering::Relaxed);
        slot.key.store(key ^ data, Ordering::Relaxed);
    }

    /// Clears the entire table.
    pub fn clear(&self) {
        for slot in &self.entries {
            slot.key.store(0, Ordering::Relaxed);
            slot.data.store(0, Ordering::Relaxed);
        }
        self.generation.store(0, Ordering::Relaxed);
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

        // Capture piece before applying move (needed for incremental hash)
        let captured_piece = new_board.get(mv.to);

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

        // Incremental Zobrist hash update (avoids full board scan)
        let mut new_hash = self.hash;
        // Toggle side-to-move
        new_hash ^= zobrist::side_key();
        // Remove old castling contribution
        new_hash ^= zobrist::castling_hash(&self.castling);
        // Remove old en passant contribution (if any, only when capture was possible)
        if let Some(ep_sq) = self.en_passant
            && zobrist::has_ep_capture_candidate(&self.board, self.turn, ep_sq)
        {
            new_hash ^= zobrist::en_passant_key(ep_sq.file);
        }
        // Remove moving piece from source square
        new_hash ^= zobrist::piece_square_key(&moving_piece, mv.from);
        // Remove captured piece (normal capture)
        if let Some(cap) = captured_piece {
            new_hash ^= zobrist::piece_square_key(&cap, mv.to);
        }
        // Remove en-passant captured pawn
        if mv.is_en_passant {
            let ep_captured_rank = match self.turn {
                Color::White => mv.to.rank - 1,
                Color::Black => mv.to.rank + 1,
            };
            let ep_pawn = Piece::new(PieceKind::Pawn, new_turn);
            new_hash ^=
                zobrist::piece_square_key(&ep_pawn, Square::new(mv.to.file, ep_captured_rank));
        }
        // Add piece at destination (possibly promoted)
        let dest_piece = if let Some(promo_kind) = mv.promotion {
            Piece::new(promo_kind, self.turn)
        } else {
            moving_piece
        };
        new_hash ^= zobrist::piece_square_key(&dest_piece, mv.to);
        // Castling: update rook positions
        if mv.is_castling {
            let rank = mv.from.rank;
            let rook = Piece::new(PieceKind::Rook, self.turn);
            let (rook_from, rook_to) = if mv.to.file == 6 {
                (7u8, 5u8)
            } else {
                (0u8, 3u8)
            };
            new_hash ^= zobrist::piece_square_key(&rook, Square::new(rook_from, rank));
            new_hash ^= zobrist::piece_square_key(&rook, Square::new(rook_to, rank));
        }
        // Add new castling contribution
        new_hash ^= zobrist::castling_hash(&new_castling);
        // Add new en passant contribution (only when capture is possible)
        if let Some(ep_sq) = new_ep
            && zobrist::has_ep_capture_candidate(&new_board, new_turn, ep_sq)
        {
            new_hash ^= zobrist::en_passant_key(ep_sq.file);
        }

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
        // Incremental hash: toggle side-to-move and remove old EP contribution
        let mut new_hash = self.hash;
        new_hash ^= zobrist::side_key();
        if let Some(ep_sq) = self.en_passant
            && zobrist::has_ep_capture_candidate(&self.board, self.turn, ep_sq)
        {
            new_hash ^= zobrist::en_passant_key(ep_sq.file);
        }
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
    /// Nodes saved by a singular-extension verification search.
    pub singular_extensions: u64,
    /// Endgame tablebase probes that returned a definitive result.
    pub tb_hits: u64,
}

impl SearchStats {
    /// Adds another worker's counters into this one (Lazy SMP aggregation).
    fn merge(&mut self, other: &SearchStats) {
        self.nodes += other.nodes;
        self.tt_hits += other.tt_hits;
        self.tt_cutoffs += other.tt_cutoffs;
        self.null_cutoffs += other.null_cutoffs;
        self.lmr_searches += other.lmr_searches;
        self.beta_cutoffs += other.beta_cutoffs;
        self.quiescence_nodes += other.quiescence_nodes;
        self.singular_extensions += other.singular_extensions;
        self.tb_hits += other.tb_hits;
    }
}

// ---------------------------------------------------------------------------
// Search result
// ---------------------------------------------------------------------------

/// One principal variation produced by the search.
///
/// With `multi_pv == 1` a result carries a single line; higher settings add
/// the second-, third-, … best lines in descending order of score.
#[derive(Debug, Clone)]
pub struct PvLine {
    /// 1-based rank of this line (1 = best).
    pub rank: usize,
    /// Score in centipawns from the searching side's perspective.
    pub score: i32,
    /// Full moves until mate, if the line is a forced mate.
    pub mate_in: Option<i32>,
    /// The line itself, starting with the root move.
    pub moves: Vec<ChessMove>,
}

impl PvLine {
    /// The root move of this line, if any.
    pub fn best_move(&self) -> Option<ChessMove> {
        self.moves.first().copied()
    }
}

/// How the engine arrived at its answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveSource {
    /// Produced by the alpha-beta search.
    Search,
    /// Taken straight from the Polyglot opening book.
    Book,
    /// Taken from a definitive endgame tablebase result.
    Tablebase,
}

/// The result of a completed search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The best move found.
    pub best_move: Option<ChessMove>,
    /// Evaluation score (centipawns, from the searching side's perspective).
    pub score: i32,
    /// The search depth achieved.
    pub depth: i32,
    /// Greatest ply reached anywhere in the tree (including quiescence).
    pub seldepth: i32,
    /// The principal variation (best line of play).
    pub pv: Vec<ChessMove>,
    /// All principal variations, best first (length == effective MultiPV).
    pub pv_lines: Vec<PvLine>,
    /// Search statistics.
    pub stats: SearchStats,
    /// Total time spent searching (milliseconds).
    pub time_ms: u64,
    /// Where the returned move came from.
    pub source: MoveSource,
    /// Tablebase verdict for the root position, when one was available.
    pub tablebase: Option<TablebaseInfo>,
    /// Transposition table fill level in per mille.
    pub hashfull: u32,
}

impl SearchResult {
    /// An empty result for a position with no legal moves.
    fn empty(time_ms: u64) -> Self {
        Self {
            best_move: None,
            score: 0,
            depth: 0,
            seldepth: 0,
            pv: Vec::new(),
            pv_lines: Vec::new(),
            stats: SearchStats::default(),
            time_ms,
            source: MoveSource::Search,
            tablebase: None,
            hashfull: 0,
        }
    }

    /// Nodes per second over the whole search (0 for an instant answer).
    pub fn nps(&self) -> u64 {
        let scaled = self.stats.nodes.saturating_mul(1000);
        scaled.checked_div(self.time_ms.max(1)).unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Search limits & progress reporting
// ---------------------------------------------------------------------------

/// Resource limits for a single search invocation.
///
/// All limits are optional except `max_depth`. The search stops as soon
/// as any hard limit is exceeded and returns the best result found so far.
///
/// Time control uses two budgets: `soft_time_ms` gates the *start* of a new
/// iterative-deepening iteration (and stretches when the best move is still
/// unstable), while `move_time_ms` is the hard wall-clock cap enforced inside
/// the tree. Passing only `move_time_ms` derives a sensible soft budget.
#[derive(Debug, Clone)]
pub struct SearchLimits {
    /// Maximum search depth in plies (clamped to `[1, MAX_DEPTH]`).
    pub max_depth: i32,
    /// Hard time budget for this search, in milliseconds.
    pub move_time_ms: Option<u64>,
    /// Soft time budget: no new iteration is started past this point.
    pub soft_time_ms: Option<u64>,
    /// Maximum number of nodes to search.
    pub max_nodes: Option<u64>,
    /// Restrict the search to these root moves (UCI `go searchmoves`).
    pub search_moves: Vec<ChessMove>,
    /// Stop as soon as a mate in at most this many moves is found
    /// (UCI `go mate`).
    pub mate_in: Option<i32>,
}

impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            max_depth: MAX_DEPTH,
            move_time_ms: None,
            soft_time_ms: None,
            max_nodes: None,
            search_moves: Vec::new(),
            mate_in: None,
        }
    }
}

impl SearchLimits {
    /// Limits for a pure fixed-depth search (no time or node budget).
    pub fn depth(max_depth: i32) -> Self {
        Self {
            max_depth,
            ..Self::default()
        }
    }

    /// Limits for a time-budgeted search (depth capped at `MAX_DEPTH`).
    pub fn move_time(move_time_ms: u64) -> Self {
        Self {
            move_time_ms: Some(move_time_ms),
            ..Self::default()
        }
    }

    /// Limits for a node-budgeted search.
    pub fn nodes(max_nodes: u64) -> Self {
        Self {
            max_nodes: Some(max_nodes),
            ..Self::default()
        }
    }

    /// Limits for an interactive skill level from `1` (weakest) to
    /// [`MAX_SKILL_LEVEL`] (strongest).
    ///
    /// Higher levels grant a longer per-move time budget and a higher depth
    /// ceiling; low levels are intentionally depth-capped so a casual player
    /// can win. Out-of-range values are clamped.
    pub fn for_level(level: u8) -> Self {
        let level = i32::from(level.clamp(1, MAX_SKILL_LEVEL));
        let move_time_ms = (level * level * 30).clamp(100, 4000) as u64;
        let max_depth = (level * 2 + 2).min(MAX_DEPTH);
        Self {
            max_depth,
            move_time_ms: Some(move_time_ms),
            ..Self::default()
        }
    }

    /// The soft budget actually used, derived from the hard budget when the
    /// caller did not set one explicitly.
    fn effective_soft_ms(&self) -> Option<u64> {
        self.soft_time_ms
            .or_else(|| self.move_time_ms.map(|ms| ms * SOFT_TIME_NUMERATOR / 100))
    }
}

/// Progress snapshot emitted after each completed iterative-deepening
/// iteration. Consumed by live CLI displays and the UCI `info` output.
#[derive(Debug, Clone)]
pub struct IterationInfo {
    /// Completed iteration depth in plies.
    pub depth: i32,
    /// Greatest ply reached anywhere in the tree so far.
    pub seldepth: i32,
    /// 1-based MultiPV line index this snapshot describes.
    pub multipv: usize,
    /// Score in centipawns from the side to move's perspective.
    pub score_cp: i32,
    /// Full moves until mate (positive: side to move mates,
    /// negative: side to move gets mated). `None` if no forced mate.
    pub mate_in: Option<i32>,
    /// Total nodes searched so far (including quiescence nodes).
    pub nodes: u64,
    /// Elapsed wall-clock time since the search started, in milliseconds.
    pub elapsed_ms: u64,
    /// Nodes per second over the whole search so far.
    pub nps: u64,
    /// Transposition table fill level in per mille.
    pub hashfull: u32,
    /// Endgame tablebase hits so far.
    pub tb_hits: u64,
    /// Principal variation in long algebraic notation (e.g. `["e2e4", "e7e5"]`).
    pub pv: Vec<String>,
}

impl Default for IterationInfo {
    fn default() -> Self {
        Self {
            depth: 0,
            seldepth: 0,
            multipv: 1,
            score_cp: 0,
            mate_in: None,
            nodes: 0,
            elapsed_ms: 0,
            nps: 0,
            hashfull: 0,
            tb_hits: 0,
            pv: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Engine configuration
// ---------------------------------------------------------------------------

/// Static configuration of a [`SearchEngine`] instance.
///
/// Unlike [`SearchLimits`], which changes from move to move, these settings
/// describe the engine itself: how much memory and how many cores it may use,
/// which knowledge sources are attached, and how strong it is allowed to play.
#[derive(Clone)]
pub struct EngineConfig {
    /// Transposition table size in MB.
    pub tt_size_mb: usize,
    /// Number of search threads (Lazy SMP). `1` = fully deterministic.
    pub threads: usize,
    /// Number of principal variations to report (`1` = classic search).
    pub multi_pv: usize,
    /// Time subtracted from every budget to absorb I/O and GUI latency.
    pub move_overhead_ms: u64,
    /// Polyglot opening book consulted at the root.
    pub book: Option<Arc<OpeningBook>>,
    /// Whether the attached book may actually be used.
    pub use_book: bool,
    /// Pick book moves by weight-proportional sampling instead of always
    /// taking the most popular one.
    pub book_variety: bool,
    /// Syzygy tablebase consulted at the root.
    pub tablebase: Option<Arc<SyzygyTablebase>>,
    /// Artificial strength limit from `0` (weakest) to [`MAX_SKILL`]
    /// (full strength). `None` plays at full strength.
    pub skill_level: Option<u8>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            tt_size_mb: DEFAULT_TT_SIZE_MB,
            threads: 1,
            multi_pv: 1,
            move_overhead_ms: 0,
            book: None,
            use_book: true,
            book_variety: true,
            tablebase: None,
            skill_level: None,
        }
    }
}

impl EngineConfig {
    /// Clamps every field into its supported range.
    fn sanitize(mut self) -> Self {
        self.tt_size_mb = self.tt_size_mb.clamp(1, 65_536);
        // Targets without real threads (WebAssembly) always search serially.
        self.threads = if THREADS_SUPPORTED {
            self.threads.clamp(1, MAX_THREADS)
        } else {
            1
        };
        self.multi_pv = self.multi_pv.clamp(1, MAX_MULTI_PV);
        self.move_overhead_ms = self.move_overhead_ms.min(5_000);
        self.skill_level = self.skill_level.map(|s| s.min(MAX_SKILL));
        self
    }
}

/// Converts a search score to a "mate in N full moves" value, if forced.
pub fn score_to_mate_in(score: i32) -> Option<i32> {
    if score.abs() > MATE_THRESHOLD {
        let plies = MATE_SCORE - score.abs();
        let moves = (plies + 1) / 2;
        Some(if score > 0 { moves } else { -moves })
    } else {
        None
    }
}

/// Converts a transposition-table-stored, ply-independent mate score back
/// into a node-local score (shifted by the current `ply`).
#[inline]
fn denormalize_mate(score: i32, ply: i32) -> i32 {
    if score > MATE_THRESHOLD {
        score - ply
    } else if score < -MATE_THRESHOLD {
        score + ply
    } else {
        score
    }
}

/// Converts a node-local mate score into a ply-independent score suitable
/// for transposition-table storage (the inverse of [`denormalize_mate`]).
#[inline]
fn normalize_mate(score: i32, ply: i32) -> i32 {
    if score > MATE_THRESHOLD {
        score + ply
    } else if score < -MATE_THRESHOLD {
        score - ply
    } else {
        score
    }
}

// ---------------------------------------------------------------------------
// Move ordering
// ---------------------------------------------------------------------------

/// MVV-LVA (Most Valuable Victim – Least Valuable Attacker) score.
fn mvv_lva_score(board: &Board, mv: &ChessMove) -> i32 {
    let victim_value = if mv.is_en_passant {
        // En passant captures a pawn on a different square than mv.to
        piece_value(PieceKind::Pawn)
    } else {
        board.get(mv.to).map(|p| piece_value(p.kind)).unwrap_or(0)
    };
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

/// Index of a `(color, kind)` pair in the continuation-history table.
#[inline]
fn piece_slot(piece: Piece) -> usize {
    let kind = match piece.kind {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 1,
        PieceKind::Bishop => 2,
        PieceKind::Rook => 3,
        PieceKind::Queen => 4,
        PieceKind::King => 5,
    };
    kind + if piece.color == Color::White {
        0
    } else {
        PIECE_SLOTS / 2
    }
}

/// One-ply continuation history: how good move *B* turned out to be when it
/// followed move *A*, keyed by `(moved piece, destination)` for both.
///
/// Stored flat (≈ 1.1 MB of `i16`) so it can live behind a single allocation
/// per search thread instead of a deeply nested array on the stack.
struct ContinuationHistory {
    table: Vec<i16>,
}

/// Context identifying the opponent's previous move for continuation history.
#[derive(Debug, Clone, Copy)]
struct ContinuationContext {
    /// Piece slot of the piece that made the previous move.
    slot: usize,
    /// Destination square index of the previous move.
    to: usize,
}

impl ContinuationHistory {
    fn new() -> Self {
        Self {
            table: vec![0; PIECE_SLOTS * 64 * PIECE_SLOTS * 64],
        }
    }

    #[inline]
    fn offset(ctx: ContinuationContext, slot: usize, to: usize) -> usize {
        ((ctx.slot * 64 + ctx.to) * PIECE_SLOTS + slot) * 64 + to
    }

    #[inline]
    fn get(&self, ctx: Option<ContinuationContext>, slot: usize, to: usize) -> i32 {
        match ctx {
            Some(ctx) => i32::from(self.table[Self::offset(ctx, slot, to)]),
            None => 0,
        }
    }

    #[inline]
    fn update(&mut self, ctx: Option<ContinuationContext>, slot: usize, to: usize, bonus: i32) {
        if let Some(ctx) = ctx {
            let entry = &mut self.table[Self::offset(ctx, slot, to)];
            let current = i32::from(*entry);
            let updated = current + bonus - bonus.abs() * current / CONT_HISTORY_MAX;
            *entry = updated.clamp(-CONT_HISTORY_MAX, CONT_HISTORY_MAX) as i16;
        }
    }

    /// Halves every score, keeping ordering hints across searches without
    /// letting stale results dominate.
    fn decay(&mut self) {
        for entry in &mut self.table {
            *entry /= 2;
        }
    }
}

/// Continuation-history scores stay within `+-CONT_HISTORY_MAX` (fits `i16`).
const CONT_HISTORY_MAX: i32 = 8_192;

/// Applies a gravity-style history update.
///
/// `h += bonus - |bonus| * h / HISTORY_MAX` keeps the score within
/// `[-HISTORY_MAX, HISTORY_MAX]` without explicit clamping and makes
/// saturated scores decay naturally (recent results outweigh stale ones).
fn update_history(entry: &mut i32, bonus: i32) {
    *entry += bonus - bonus.abs() * *entry / HISTORY_MAX;
}

/// Sort scored moves in descending order.
fn sort_moves(scored: &mut [(ChessMove, i32)]) {
    scored.sort_unstable_by_key(|m| std::cmp::Reverse(m.1));
}

// ---------------------------------------------------------------------------
// Search engine
// ---------------------------------------------------------------------------

/// A root move together with the state the root loop tracks for it.
#[derive(Debug, Clone)]
struct RootMove {
    /// The move itself.
    mv: ChessMove,
    /// Score from the most recent completed root search.
    score: i32,
    /// Score from the previous iteration (seeds the aspiration window).
    previous_score: i32,
    /// Best line found for this move, starting with the move itself.
    pv: Vec<ChessMove>,
}

impl RootMove {
    fn new(mv: ChessMove) -> Self {
        Self {
            mv,
            score: -INFINITY,
            previous_score: -INFINITY,
            pv: vec![mv],
        }
    }
}

/// The main search engine.
///
/// One instance owns the shared transposition table, the engine configuration
/// and the per-thread heuristic tables of the *main* search thread. When
/// [`EngineConfig::threads`] exceeds one, the engine clones itself into
/// short-lived helper workers that share the table (Lazy SMP).
pub struct SearchEngine {
    /// Shared transposition table (also used by all Lazy SMP helpers).
    pub tt: Arc<TranspositionTable>,
    /// Static engine configuration.
    config: EngineConfig,
    /// Killer moves per ply (2 slots per ply).
    killers: Vec<[Option<ChessMove>; 2]>,
    /// History heuristic table: `[from_sq][to_sq] -> score`.
    history: Box<[[i32; 64]; 64]>,
    /// Counter-move heuristic: `[prev_from][prev_to] -> counter_move`.
    counter_moves: Box<[[Option<ChessMove>; 64]; 64]>,
    /// One-ply continuation history.
    continuation: ContinuationHistory,
    /// Search statistics for the current search.
    pub stats: SearchStats,
    /// Cancellation flag — set to `true` to abort the search.
    pub abort: Arc<AtomicBool>,
    /// Secondary stop flag used to retire Lazy SMP helpers.
    secondary_abort: Option<Arc<AtomicBool>>,
    /// Hard wall-clock deadline for the current search (in-tree time limit).
    deadline: Option<Instant>,
    /// Hard node budget for the current search (in-tree node limit).
    node_limit: Option<u64>,
    /// Set once a hard limit is hit; makes every node bail out immediately.
    stopped: bool,
    /// Zobrist hashes of the ancestors along the current search line, indexed
    /// by ply (`path[k]` is the position at ply `k`). Used to detect
    /// draw-by-repetition inside the tree.
    path: Vec<u64>,
    /// Static evaluation per ply, backing the "improving" heuristic.
    eval_stack: Vec<i32>,
    /// Nodes searched by *every* thread of the current search.
    ///
    /// [`SearchLimits::max_nodes`] is one budget for the search as a whole, so
    /// helpers share this counter with the main thread rather than each
    /// policing a private slice of it.
    shared_nodes: Arc<AtomicU64>,
    /// This thread's node count as last published into [`Self::shared_nodes`].
    published_nodes: u64,
    /// Move excluded at a given ply during singular-extension verification.
    excluded: Vec<Option<ChessMove>>,
    /// Continuation context (opponent's previous move) per ply.
    cont_stack: Vec<Option<ContinuationContext>>,
    /// Zobrist hashes of the positions that preceded the search root in the
    /// actual game (set via [`SearchEngine::set_game_history`]). Lets the
    /// engine see repetitions that span the move already played on the board.
    game_history: Vec<u64>,
    /// Root moves of the current search, ordered best first.
    root_moves: Vec<RootMove>,
    /// Deepest iterative-deepening iteration that ran to completion.
    completed_depth: i32,
    /// Greatest ply reached in the current search.
    seldepth: i32,
    /// Worker index: `0` is the main thread, `>0` a Lazy SMP helper.
    thread_id: usize,
    /// xorshift state for skill-limited move selection.
    rng: u64,
}

impl SearchEngine {
    /// Creates a new search engine with the given TT size and otherwise
    /// default configuration (single-threaded, MultiPV 1, no book).
    pub fn new(tt_size_mb: usize) -> Self {
        Self::with_config(EngineConfig {
            tt_size_mb,
            ..EngineConfig::default()
        })
    }

    /// Creates a new search engine from a full [`EngineConfig`].
    pub fn with_config(config: EngineConfig) -> Self {
        let config = config.sanitize();
        let tt = Arc::new(TranspositionTable::new(config.tt_size_mb));
        Self::from_parts(tt, config, 0)
    }

    /// Creates a new search engine with default TT size.
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_TT_SIZE_MB)
    }

    /// Builds an engine around an existing shared table.
    fn from_parts(tt: Arc<TranspositionTable>, config: EngineConfig, thread_id: usize) -> Self {
        let plies = MAX_DEPTH as usize + 1;
        Self {
            tt,
            config,
            killers: vec![[None; 2]; plies],
            history: Box::new([[0i32; 64]; 64]),
            counter_moves: Box::new([[None; 64]; 64]),
            continuation: ContinuationHistory::new(),
            stats: SearchStats::default(),
            abort: Arc::new(AtomicBool::new(false)),
            secondary_abort: None,
            deadline: None,
            node_limit: None,
            stopped: false,
            path: vec![0u64; plies],
            eval_stack: vec![0i32; plies],
            shared_nodes: Arc::new(AtomicU64::new(0)),
            published_nodes: 0,
            excluded: vec![None; plies],
            cont_stack: vec![None; plies],
            game_history: Vec::new(),
            root_moves: Vec::new(),
            completed_depth: 0,
            seldepth: 0,
            thread_id,
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }

    /// Returns the engine's current configuration.
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Applies a new configuration, reallocating the transposition table when
    /// its size changed. Heuristic tables are preserved.
    pub fn set_config(&mut self, config: EngineConfig) {
        let config = config.sanitize();
        if config.tt_size_mb != self.config.tt_size_mb {
            self.tt = Arc::new(TranspositionTable::new(config.tt_size_mb));
        }
        self.config = config;
    }

    /// Convenience setter for the number of Lazy SMP threads.
    pub fn set_threads(&mut self, threads: usize) {
        self.config.threads = threads.clamp(1, MAX_THREADS);
    }

    /// Convenience setter for the number of reported principal variations.
    pub fn set_multi_pv(&mut self, multi_pv: usize) {
        self.config.multi_pv = multi_pv.clamp(1, MAX_MULTI_PV);
    }

    /// Supplies the position hashes that precede the search root in the real
    /// game, so the search can score a line that repeats one of them as a
    /// draw. Pass the Zobrist hashes (see [`zobrist::hash_position`]) of every
    /// earlier position, in game order. Replaces any previously set history;
    /// pass an empty slice to clear it.
    pub fn set_game_history(&mut self, history: &[u64]) {
        self.game_history.clear();
        self.game_history.extend_from_slice(history);
    }

    /// Replaces the internal abort flag with a shared external token.
    ///
    /// This allows external orchestration (e.g. analysis job cancellation)
    /// to stop the search promptly while it is running.
    pub fn set_abort_token(&mut self, token: Arc<AtomicBool>) {
        self.abort = token;
    }

    /// Resets the current abort flag to `false`.
    pub fn reset_abort(&self) {
        self.abort.store(false, Ordering::Relaxed);
    }

    /// Clears every learned table and the transposition table — the engine
    /// starts the next search with no memory of previous ones.
    pub fn clear_memory(&mut self) {
        self.tt.clear();
        for k in &mut self.killers {
            *k = [None; 2];
        }
        *self.history = [[0i32; 64]; 64];
        *self.counter_moves = [[None; 64]; 64];
        self.continuation = ContinuationHistory::new();
        self.game_history.clear();
    }

    /// Creates a Lazy SMP helper that shares this engine's table and knowledge
    /// sources but keeps its own heuristic tables.
    fn spawn_helper(&self, thread_id: usize, stop: &Arc<AtomicBool>) -> Self {
        let mut helper = Self::from_parts(
            Arc::clone(&self.tt),
            EngineConfig {
                multi_pv: 1,
                ..self.config.clone()
            },
            thread_id,
        );
        helper.abort = Arc::clone(&self.abort);
        helper.secondary_abort = Some(Arc::clone(stop));
        // One node budget for the whole search, shared with the main thread.
        helper.shared_nodes = Arc::clone(&self.shared_nodes);
        helper.game_history = self.game_history.clone();
        helper.rng = self.rng ^ ((thread_id as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        helper
    }

    /// Returns `true` if the search should stop — either an external abort
    /// token fired, a helper was retired, or an internal hard limit has
    /// already been tripped.
    #[inline]
    fn should_stop(&self) -> bool {
        self.stopped
            || self.abort.load(Ordering::Relaxed)
            || self
                .secondary_abort
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
    }

    /// Advances the xorshift state and returns the next pseudo-random value.
    #[inline]
    fn next_rng(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// Combined butterfly + continuation history score of a quiet move.
    #[inline]
    fn quiet_history_score(
        &self,
        board: &Board,
        mv: &ChessMove,
        ctx: Option<ContinuationContext>,
    ) -> i32 {
        let butterfly = self.history[mv.from.index()][mv.to.index()];
        let continuation = match board.get(mv.from) {
            Some(piece) => self.continuation.get(ctx, piece_slot(piece), mv.to.index()),
            None => 0,
        };
        butterfly + continuation
    }

    /// Scores and sorts the moves of a node for optimal alpha-beta pruning.
    ///
    /// Priority (descending):
    /// 1. TT best move ([`ORDER_TT_MOVE`])
    /// 2. Queen promotions ([`ORDER_PROMOTION`] + MVV-LVA)
    /// 3. Good captures, SEE >= 0 ([`ORDER_GOOD_CAPTURE`] + MVV-LVA)
    /// 4. Killer moves ([`ORDER_KILLER_0`] / [`ORDER_KILLER_1`])
    /// 5. Counter-move of the opponent's previous move ([`ORDER_COUNTER_MOVE`])
    /// 6. Quiet moves by butterfly + continuation history score
    /// 7. Losing captures, SEE < 0 ([`ORDER_BAD_CAPTURE`] + MVV-LVA)
    fn order_moves(
        &self,
        moves: &[ChessMove],
        pos: &SearchPosition,
        tt_move: Option<&ChessMove>,
        ply: usize,
        counter_move: Option<&ChessMove>,
        ctx: Option<ContinuationContext>,
    ) -> Vec<(ChessMove, i32)> {
        let board = &pos.board;
        let killers = &self.killers[ply];
        let mut scored: Vec<(ChessMove, i32)> = moves
            .iter()
            .map(|mv| {
                let is_capture = board.get(mv.to).is_some() || mv.is_en_passant;
                let score = if tt_move.is_some_and(|tm| tm == mv) {
                    ORDER_TT_MOVE
                } else if mv.promotion == Some(PieceKind::Queen) {
                    ORDER_PROMOTION + mvv_lva_score(board, mv)
                } else if is_capture {
                    if see(board, mv, pos.turn) >= 0 {
                        ORDER_GOOD_CAPTURE + mvv_lva_score(board, mv)
                    } else {
                        ORDER_BAD_CAPTURE + mvv_lva_score(board, mv)
                    }
                } else if killers[0].as_ref().is_some_and(|k| k == mv) {
                    ORDER_KILLER_0
                } else if killers[1].as_ref().is_some_and(|k| k == mv) {
                    ORDER_KILLER_1
                } else if counter_move.is_some_and(|cm| cm == mv) {
                    ORDER_COUNTER_MOVE
                } else {
                    // History heuristics: always far below the killer tier and
                    // far above the bad-capture tier.
                    self.quiet_history_score(board, mv, ctx)
                };
                (*mv, score)
            })
            .collect();
        sort_moves(&mut scored);
        scored
    }

    /// Checks the wall-clock and node hard limits. Called periodically from
    /// inside the tree (every [`NODE_CHECK_INTERVAL`] nodes) so the relatively
    /// expensive [`Instant::now`] call stays off the hot path.
    /// `true` when the node counter has reached an in-tree re-check boundary.
    #[inline]
    fn at_limit_checkpoint(&self) -> bool {
        let interval = if self.node_limit.is_some() {
            NODE_LIMIT_CHECK_INTERVAL
        } else {
            NODE_CHECK_INTERVAL
        };
        self.stats.nodes & (interval - 1) == 0
    }

    #[inline]
    fn hit_hard_limit(&mut self) -> bool {
        // The wall-clock probe is the expensive half, so it stays on the
        // coarse boundary even when the node budget is polled more often.
        if let Some(deadline) = self.deadline
            && self.stats.nodes & (NODE_CHECK_INTERVAL - 1) == 0
            && Instant::now() >= deadline
        {
            return true;
        }
        if let Some(limit) = self.node_limit {
            // `--nodes` is one budget for the whole search, not one per helper
            // thread. Publish what this thread searched since the last check
            // and stop on the *global* total: dividing the budget per thread
            // made an identical `--nodes` reach depth 12 on one thread and
            // depth 1 on 32, while the reported count overshot several-fold.
            let delta = self.stats.nodes.saturating_sub(self.published_nodes);
            self.published_nodes = self.stats.nodes;
            if self.shared_nodes.fetch_add(delta, Ordering::Relaxed) + delta >= limit {
                return true;
            }
        }
        false
    }

    /// Returns `true` if the position with the given `hash` has already
    /// occurred — either earlier on the current search path or in the
    /// pre-search game history. Only positions within the current
    /// reversible-move window (`halfmove_clock` plies back) can repeat, so the
    /// scan stops there. A single match is treated as a draw.
    fn is_repetition(&self, hash: u64, halfmove_clock: u32, ply: i32) -> bool {
        let window = halfmove_clock as usize;
        let mut scanned = 0usize;

        // In-tree ancestors, most recent first: path[ply-1] down to path[0].
        let mut idx = ply as usize;
        while idx > 0 && scanned < window {
            idx -= 1;
            scanned += 1;
            if self.path[idx] == hash {
                return true;
            }
        }

        // Continue into the real game's history (most recent first).
        for &h in self.game_history.iter().rev() {
            if scanned >= window {
                break;
            }
            scanned += 1;
            if h == hash {
                return true;
            }
        }

        false
    }

    /// Runs iterative deepening search to the specified depth.
    ///
    /// Returns the best move and evaluation at the target depth.
    /// Convenience wrapper around [`SearchEngine::search_limited`] with a
    /// pure depth limit and no progress reporting.
    pub fn search(&mut self, pos: &SearchPosition, max_depth: i32) -> SearchResult {
        self.search_limited(pos, &SearchLimits::depth(max_depth), None)
    }

    /// Runs iterative deepening search under the given [`SearchLimits`].
    ///
    /// The search stops when the depth, time, or node budget is exhausted
    /// (or the abort token fires) and returns the best result found so far.
    /// After each completed iteration, `on_iteration` is invoked once per
    /// MultiPV line with a progress snapshot — used for live CLI displays
    /// and UCI `info` lines.
    ///
    /// The pipeline is: opening book → endgame tablebase → alpha-beta search
    /// (single-threaded, or Lazy SMP across [`EngineConfig::threads`] cores).
    pub fn search_limited(
        &mut self,
        pos: &SearchPosition,
        limits: &SearchLimits,
        on_iteration: Option<&mut dyn FnMut(&IterationInfo)>,
    ) -> SearchResult {
        let start = Instant::now();
        self.seldepth = 0;
        self.stats = SearchStats::default();

        // Seed the RNG so skill-limited play varies between identical
        // positions instead of always producing the same weak move.
        self.rng ^= pos.hash ^ epoch_nanos();

        // Build (and optionally restrict) the root move list.
        let mut legal = pos.legal_moves();
        if legal.is_empty() {
            // Terminal position: there is no move to search, but the score is
            // emphatically not zero. Reporting 0 for a checkmate makes the
            // mating move look like a ~30000 cp blunder to every caller that
            // compares the evaluation before and after it — which is what
            // dragged both sides to 0.0% accuracy on any decisive game.
            let mut result = SearchResult::empty(start.elapsed().as_millis() as u64);
            result.score = if pos.is_in_check() {
                -MATE_SCORE
            } else {
                DRAW_SCORE
            };
            return result;
        }
        if !limits.search_moves.is_empty() {
            legal.retain(|mv| limits.search_moves.contains(mv));
            // `searchmoves` matching nothing is a caller mistake, not a
            // terminal position — leave the score at zero.
            if legal.is_empty() {
                return SearchResult::empty(start.elapsed().as_millis() as u64);
            }
        }

        // 1. Opening book — an instant, zero-node answer.
        if let Some(book_move) = self.probe_book(pos, &legal) {
            let mut result = SearchResult::empty(start.elapsed().as_millis() as u64);
            result.best_move = Some(book_move);
            result.pv = vec![book_move];
            result.pv_lines = vec![PvLine {
                rank: 1,
                score: 0,
                mate_in: None,
                moves: vec![book_move],
            }];
            result.source = MoveSource::Book;
            return result;
        }

        // 2. Endgame tablebase — informational at the root; the verdict is
        //    attached to the result and surfaced by the UI.
        let tablebase = self.probe_tablebase(pos);
        if tablebase
            .as_ref()
            .is_some_and(|tb| tb.is_tablebase_position)
        {
            self.stats.tb_hits += 1;
        }

        // 3. Alpha-beta search.
        //
        // Order the root list up front. A search stopped before it completes
        // even one iteration still has to answer, and it answers with
        // `root_moves.first()` — in raw generation order that is whatever the
        // move generator happened to emit first (`a1b1` for a rook on a1).
        // Ordered, the worst case becomes the engine's best static guess: the
        // transposition-table move, a queen promotion or a winning capture.
        let tt_move = self.tt.probe(pos.hash).and_then(|entry| {
            entry
                .best_move
                .map(|em| em.to_chess_move())
                .filter(|mv| legal.contains(mv))
        });
        self.root_moves = self
            .order_moves(&legal, pos, tt_move.as_ref(), 0, None, None)
            .into_iter()
            .map(|(mv, _)| RootMove::new(mv))
            .collect();

        // Open the shared node budget before any helper can start counting.
        self.shared_nodes.store(0, Ordering::Relaxed);
        self.published_nodes = 0;
        let threads = if THREADS_SUPPORTED {
            self.config.threads.max(1)
        } else {
            1
        };
        let helper_count = threads.saturating_sub(1);

        if helper_count == 0 {
            self.run_iterative(pos, limits, start, on_iteration);
        } else {
            let stop = Arc::new(AtomicBool::new(false));
            let mut helpers: Vec<SearchEngine> = (1..threads)
                .map(|id| self.spawn_helper(id, &stop))
                .collect();
            // Every thread carries the full budget and they all police the
            // same shared counter, so `--nodes N` means N nodes in total
            // however many threads are running.
            let helper_limits = limits.clone();
            let main_limits = limits.clone();
            let root_moves = self.root_moves.clone();

            let helper_stats = std::thread::scope(|scope| {
                let handles: Vec<_> = helpers
                    .iter_mut()
                    .map(|helper| {
                        let child_pos = pos.clone();
                        let child_limits = helper_limits.clone();
                        let child_roots = root_moves.clone();
                        scope.spawn(move || {
                            helper.root_moves = child_roots;
                            helper.run_iterative(&child_pos, &child_limits, start, None);
                            std::mem::take(&mut helper.stats)
                        })
                    })
                    .collect();

                self.run_iterative(pos, &main_limits, start, on_iteration);
                stop.store(true, Ordering::Relaxed);

                handles
                    .into_iter()
                    .filter_map(|handle| handle.join().ok())
                    .collect::<Vec<_>>()
            });

            for stats in &helper_stats {
                self.stats.merge(stats);
            }
        }

        self.finish_result(start, tablebase)
    }

    /// Assembles the final [`SearchResult`] from the root move list.
    fn finish_result(&mut self, start: Instant, tablebase: Option<TablebaseInfo>) -> SearchResult {
        let elapsed = start.elapsed().as_millis() as u64;

        // Each MultiPV pass runs its own aspiration window and re-sorts only
        // its own tail (`sort_root_moves(pv_index)`), so the order the root
        // list is left in is pass order, not score order. Rank once here, or
        // `pv_lines` comes out unsorted and `root_moves.first()` — the move
        // the engine actually plays — can be worse than a line it reported
        // below itself.
        self.root_moves
            .sort_by_key(|rm| std::cmp::Reverse(rm.score));

        let lines: Vec<PvLine> = self
            .root_moves
            .iter()
            // Drop never-scored moves *before* taking the top N: filtering
            // afterwards silently answers a five-line request with four.
            .filter(|rm| rm.score > -INFINITY)
            .take(self.config.multi_pv)
            .enumerate()
            .map(|(i, rm)| PvLine {
                rank: i + 1,
                score: rm.score,
                mate_in: score_to_mate_in(rm.score),
                moves: rm.pv.clone(),
            })
            .collect();

        let best = self.root_moves.first().cloned();
        let chosen = self
            .pick_skill_move()
            .or_else(|| best.as_ref().map(|rm| rm.mv));

        // A skill-limited pick may not be the top line; report its own PV.
        let pv = match chosen {
            Some(mv) => self
                .root_moves
                .iter()
                .find(|rm| rm.mv == mv)
                .map(|rm| rm.pv.clone())
                .unwrap_or_else(|| vec![mv]),
            None => Vec::new(),
        };
        let score = match chosen {
            Some(mv) => self
                .root_moves
                .iter()
                .find(|rm| rm.mv == mv)
                .map(|rm| rm.score)
                .unwrap_or(0),
            None => 0,
        };

        SearchResult {
            best_move: chosen,
            score: if score == -INFINITY { 0 } else { score },
            depth: self.completed_depth,
            seldepth: self.seldepth,
            pv,
            pv_lines: lines,
            stats: self.stats.clone(),
            time_ms: elapsed,
            source: MoveSource::Search,
            tablebase,
            hashfull: self.tt.hashfull(),
        }
    }

    /// Consults the opening book for the root position.
    ///
    /// Returns a legal book move, choosing either the most popular entry or —
    /// with [`EngineConfig::book_variety`] — a weight-proportional sample.
    fn probe_book(&mut self, pos: &SearchPosition, legal: &[ChessMove]) -> Option<ChessMove> {
        if !self.config.use_book {
            return None;
        }
        let book = self.config.book.as_ref()?;
        let entries = book.lookup(&pos.board, pos.turn, &pos.castling, pos.en_passant);
        // Keep only entries that correspond to an actually legal move, and
        // adopt the legal move's flags (castling / en passant).
        let candidates: Vec<(ChessMove, u32)> = entries
            .iter()
            .filter_map(|entry| {
                legal
                    .iter()
                    .find(|mv| {
                        mv.from == entry.chess_move.from
                            && mv.to == entry.chess_move.to
                            && mv.promotion == entry.chess_move.promotion
                    })
                    .map(|mv| (*mv, u32::from(entry.weight).max(1)))
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        if !self.config.book_variety {
            return candidates.iter().max_by_key(|(_, w)| *w).map(|(mv, _)| *mv);
        }
        let total: u64 = candidates.iter().map(|(_, w)| u64::from(*w)).sum();
        let mut ticket = self.next_rng() % total.max(1);
        for (mv, weight) in &candidates {
            let weight = u64::from(*weight);
            if ticket < weight {
                return Some(*mv);
            }
            ticket -= weight;
        }
        candidates.last().map(|(mv, _)| *mv)
    }

    /// Probes the endgame tablebase for the root position, if one is attached
    /// and the position is within its coverage.
    fn probe_tablebase(&self, pos: &SearchPosition) -> Option<TablebaseInfo> {
        let tb = self.config.tablebase.as_ref()?;
        if !tb.is_in_range(&pos.board) {
            return None;
        }
        Some(tb.probe(&pos.board, pos.turn, &pos.castling, pos.en_passant))
    }

    /// Applies the artificial strength limit, if configured.
    ///
    /// Weakened play picks among root moves that are close enough to the best
    /// one, perturbed by a level-dependent amount of noise. This produces the
    /// occasional human-looking inaccuracy instead of the robotic, uniformly
    /// short-sighted play a pure depth cap gives.
    fn pick_skill_move(&mut self) -> Option<ChessMove> {
        let skill = self.config.skill_level?;
        if skill >= MAX_SKILL || self.root_moves.len() < 2 {
            return None;
        }
        let best_score = self.root_moves.first()?.score;
        if best_score <= -INFINITY {
            return None;
        }
        let weakness = i32::from(MAX_SKILL - skill);
        let window = weakness * 30 + 10;
        let noise_span = (weakness as u64) * 20 + 1;

        let mut chosen = self.root_moves[0].mv;
        let mut best_value = i32::MIN;
        for index in 0..self.root_moves.len() {
            let candidate = &self.root_moves[index];
            if candidate.score <= -INFINITY || best_score - candidate.score > window {
                continue;
            }
            let (mv, score) = (candidate.mv, candidate.score);
            let noise = (self.next_rng() % noise_span) as i32;
            let value = score + noise;
            if value > best_value {
                best_value = value;
                chosen = mv;
            }
        }
        Some(chosen)
    }

    /// Runs the iterative-deepening loop over the pre-built root move list.
    fn run_iterative(
        &mut self,
        pos: &SearchPosition,
        limits: &SearchLimits,
        start: Instant,
        mut on_iteration: Option<&mut dyn FnMut(&IterationInfo)>,
    ) {
        let max_depth = limits.max_depth.clamp(1, MAX_DEPTH);
        self.stopped = false;
        self.completed_depth = 0;
        self.deadline = limits.move_time_ms.map(|ms| {
            let budget = ms.saturating_sub(self.config.move_overhead_ms).max(1);
            start + Duration::from_millis(budget)
        });
        self.node_limit = limits.max_nodes;

        // Advance the TT generation so entries from previous searches become
        // preferred replacement candidates (depth-preferred within the current
        // generation, age-out across generations).
        if self.thread_id == 0 {
            self.tt.new_generation();
        }

        // Reset killers; age (rather than clear) the history tables so useful
        // ordering hints survive into the next search.
        for k in &mut self.killers {
            *k = [None; 2];
        }
        for row in self.history.iter_mut() {
            for h in row.iter_mut() {
                *h /= 2;
            }
        }
        self.continuation.decay();
        // The root position must be on the repetition path before ply 1.
        self.path[0] = pos.hash;
        self.excluded.iter_mut().for_each(|e| *e = None);
        self.cont_stack.iter_mut().for_each(|c| *c = None);

        // Seed the root's static eval. `alpha_beta` is never entered at ply 0,
        // so without this every ply-2 node compares against the vector's zero
        // initialiser and `improving` collapses into `static_eval > 0` —
        // permanently false in a lost position, permanently true in a won one,
        // silently mis-tuning RFP margins, LMP budgets and LMR reductions.
        self.eval_stack.iter_mut().for_each(|e| *e = -INFINITY);
        if !pos.is_in_check() {
            self.eval_stack[0] = eval::evaluate(&pos.board, pos.turn);
        }

        let multi_pv = self.config.multi_pv.min(self.root_moves.len()).max(1);
        let soft_ms = limits.effective_soft_ms();
        let mut stable_iterations = 0u32;
        let mut previous_best: Option<ChessMove> = None;
        // Helper threads start one ply ahead on odd ids, which desynchronises
        // their trees and is what makes Lazy SMP pay off.
        let start_depth = 1 + (self.thread_id % 2) as i32;

        for depth in start_depth..=max_depth {
            if self.should_stop() {
                break;
            }
            if self.thread_id == 0
                && depth > 1
                && let Some(soft) = soft_ms
            {
                let budget = if stable_iterations >= STABLE_ITERATIONS {
                    soft * STABLE_TIME_FACTOR / 100
                } else if previous_best.is_some_and(|mv| mv != self.root_moves[0].mv) {
                    soft * (100 + UNSTABLE_TIME_BONUS) / 100
                } else {
                    soft
                };
                if start.elapsed().as_millis() as u64 >= budget {
                    break;
                }
            }
            // The main thread always completes depth 1 — an engine with no
            // move at all is worse than one slightly over budget. Helpers only
            // warm the shared table, so they stop the moment the budget is
            // spent instead of each running a full first iteration.
            let owes_a_move = self.thread_id == 0 && depth <= 1;
            if let Some(max_nodes) = limits.max_nodes
                && !owes_a_move
                && self.shared_nodes.load(Ordering::Relaxed) + self.stats.nodes
                    - self.published_nodes
                    >= max_nodes
            {
                break;
            }

            previous_best = self.root_moves.first().map(|rm| rm.mv);
            self.search_root(pos, depth, multi_pv, start, &mut on_iteration);

            if self.should_stop() {
                break;
            }
            self.completed_depth = depth;
            for rm in &mut self.root_moves {
                rm.previous_score = rm.score;
            }

            if previous_best == self.root_moves.first().map(|rm| rm.mv) {
                stable_iterations += 1;
            } else {
                stable_iterations = 0;
            }

            // `go mate N`: stop as soon as a short enough mate is proven.
            if let Some(target) = limits.mate_in
                && let Some(found) = score_to_mate_in(self.root_moves[0].score)
                && found > 0
                && found <= target
            {
                break;
            }

            log::trace!(
                "thread {} depth {} score {} nodes {} time {}ms",
                self.thread_id,
                depth,
                self.root_moves[0].score,
                self.stats.nodes,
                start.elapsed().as_millis()
            );
        }

        self.restore_interrupted_scores();
    }

    /// Re-instates the last completed iteration's scores after an abort.
    ///
    /// [`Self::search_root_window`] wipes the root scores to `-INFINITY`
    /// before it searches, and every abort path returns without filling them
    /// in again. Left as-is, [`Self::finish_result`] would report a score of
    /// `0` and an empty MultiPV list next to a `best_move` and PV that still
    /// come from the previous iteration — and because the soft limit only
    /// gates the *start* of an iteration, a time-limited search almost always
    /// stops in the middle of one.
    ///
    /// Moves whose score survived the interrupted iteration keep it; the rest
    /// fall back to `previous_score`. The array order is untouched, so the
    /// chosen move never changes as a result of this repair.
    fn restore_interrupted_scores(&mut self) {
        for rm in &mut self.root_moves {
            if rm.score == -INFINITY {
                rm.score = rm.previous_score;
            }
        }
    }

    /// Searches every MultiPV line at one iterative-deepening depth.
    fn search_root(
        &mut self,
        pos: &SearchPosition,
        depth: i32,
        multi_pv: usize,
        start: Instant,
        on_iteration: &mut Option<&mut dyn FnMut(&IterationInfo)>,
    ) {
        for pv_index in 0..multi_pv {
            if self.should_stop() {
                return;
            }

            // Aspiration window seeded from the previous iteration's score.
            let previous = self.root_moves[pv_index].previous_score;
            let mut delta = ASPIRATION_WINDOW;
            let (mut alpha, mut beta) = if depth <= 4 || previous.abs() > MATE_THRESHOLD {
                (-INFINITY, INFINITY)
            } else {
                (
                    (previous - delta).max(-INFINITY),
                    (previous + delta).min(INFINITY),
                )
            };

            loop {
                self.search_root_window(pos, depth, alpha, beta, pv_index);
                if self.should_stop() {
                    return;
                }
                self.sort_root_moves(pv_index);
                let best = self.root_moves[pv_index].score;

                if best <= alpha && alpha > -INFINITY {
                    beta = (alpha + beta) / 2;
                    alpha = (best - delta).max(-INFINITY);
                } else if best >= beta && beta < INFINITY {
                    beta = (best + delta).min(INFINITY);
                } else {
                    break;
                }
                delta += delta / 2 + 12;
                if delta > ASPIRATION_GIVE_UP {
                    alpha = -INFINITY;
                    beta = INFINITY;
                }
            }

            if let Some(cb) = on_iteration.as_mut() {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                let scaled = self.stats.nodes.saturating_mul(1000);
                let line = &self.root_moves[pv_index];
                cb(&IterationInfo {
                    depth,
                    seldepth: self.seldepth,
                    multipv: pv_index + 1,
                    score_cp: line.score,
                    mate_in: score_to_mate_in(line.score),
                    nodes: self.stats.nodes,
                    elapsed_ms,
                    nps: scaled.checked_div(elapsed_ms.max(1)).unwrap_or(scaled),
                    hashfull: self.tt.hashfull(),
                    tb_hits: self.stats.tb_hits,
                    pv: line.pv.iter().map(|m| m.to_string()).collect(),
                });
            }
        }
    }

    /// Searches all root moves from `pv_index` onwards inside one window.
    fn search_root_window(
        &mut self,
        pos: &SearchPosition,
        depth: i32,
        alpha: i32,
        beta: i32,
        pv_index: usize,
    ) {
        let mut alpha = alpha;
        for rm in self.root_moves[pv_index..].iter_mut() {
            rm.score = -INFINITY;
        }

        for index in pv_index..self.root_moves.len() {
            let mv = self.root_moves[index].mv;
            let child = pos.make_move(&mv);
            let gives_check = child.is_in_check();
            let extension = i32::from(gives_check);
            let new_depth = depth - 1 + extension;
            let move_number = index - pv_index;
            let mut score;

            if move_number == 0 {
                score = -self.alpha_beta(&child, new_depth, -beta, -alpha, 1, true, Some(mv));
            } else {
                // Root LMR: trim clearly late, quiet moves.
                let is_quiet = pos.board.get(mv.to).is_none() && !mv.is_en_passant;
                let reduction = if depth >= 3 && move_number >= 3 && is_quiet && !gives_check {
                    (LMR_TABLE[depth.min(63) as usize][move_number.min(63)] as i32 - 1)
                        .clamp(0, new_depth - 1)
                } else {
                    0
                };
                score = -self.alpha_beta(
                    &child,
                    new_depth - reduction,
                    -alpha - 1,
                    -alpha,
                    1,
                    false,
                    Some(mv),
                );
                if score > alpha && reduction > 0 {
                    score =
                        -self.alpha_beta(&child, new_depth, -alpha - 1, -alpha, 1, false, Some(mv));
                }
                if score > alpha && score < beta {
                    score = -self.alpha_beta(&child, new_depth, -beta, -alpha, 1, true, Some(mv));
                }
            }

            if self.should_stop() {
                return;
            }

            self.root_moves[index].score = score;
            if score > alpha || move_number == 0 {
                let mut pv = vec![mv];
                pv.extend(self.extract_pv(&child, depth));
                self.root_moves[index].pv = pv;
            }
            if score > alpha {
                alpha = score;
                if score >= beta {
                    return;
                }
            }
        }
    }

    /// Stable-sorts the root moves from `from` onwards by descending score,
    /// so `root_moves[from]` holds the best remaining line.
    fn sort_root_moves(&mut self, from: usize) {
        self.root_moves[from..].sort_by_key(|rm| std::cmp::Reverse(rm.score));
    }

    /// Principal Variation Search (alpha-beta with PVS enhancements).
    ///
    /// `prev_move` is the opponent's move that led to `pos` (used by the
    /// counter-move heuristic); it is `None` at the root and after a null move.
    #[allow(clippy::too_many_arguments)]
    fn alpha_beta(
        &mut self,
        pos: &SearchPosition,
        mut depth: i32,
        mut alpha: i32,
        mut beta: i32,
        ply: i32,
        is_pv: bool,
        prev_move: Option<ChessMove>,
    ) -> i32 {
        // Prompt cancellation: external abort token or an internal hard stop.
        if self.should_stop() {
            return 0;
        }

        self.stats.nodes += 1;
        self.seldepth = self.seldepth.max(ply);

        // Periodic hard time/node-limit check, kept off the per-node hot path.
        if self.at_limit_checkpoint() && self.hit_hard_limit() {
            self.stopped = true;
            return 0;
        }

        // Hard ply ceiling to prevent out-of-bounds access on the killer table.
        if ply >= MAX_DEPTH {
            return eval::evaluate(&pos.board, pos.turn);
        }

        // Depth exhausted → quiescence search.
        if depth <= 0 {
            return self.quiescence(pos, alpha, beta, ply);
        }

        // Draw detection: 50-move rule check.
        if pos.halfmove_clock >= 100 {
            return DRAW_SCORE;
        }

        // Record this node on the current search line, then test for a draw by
        // repetition against earlier nodes on the path and the pre-search game
        // history. A single recurrence is enough — the side to move can usually
        // force the threefold — so this lets the engine find saving perpetual
        // checks and avoid repeating away a winning position.
        self.path[ply as usize] = pos.hash;
        if ply > 0 && self.is_repetition(pos.hash, pos.halfmove_clock, ply) {
            return DRAW_SCORE;
        }

        // Mate-distance pruning: tighten the window to the best/worst mate that
        // is still reachable from this ply. Cuts off lines that cannot improve
        // on an already-found mate and shortens proven mating sequences.
        alpha = alpha.max(-MATE_SCORE + ply);
        beta = beta.min(MATE_SCORE - ply - 1);
        if alpha >= beta {
            return alpha;
        }

        // A singular-extension verification search re-enters this node with one
        // move masked out; its result describes the *rest* of the node, so TT
        // cutoffs and stores must be bypassed for the duration.
        let excluded = self.excluded[ply as usize];
        let singular_search = excluded.is_some();

        // Probe the transposition table.
        let mut tt_move: Option<ChessMove> = None;
        let mut tt_eval = TT_EVAL_NONE;
        let mut tt_depth = -1;
        let mut tt_score = 0;
        let mut tt_flag = TTFlag::Alpha;
        let mut tt_hit = false;
        if let Some(entry) = self.tt.probe(pos.hash) {
            self.stats.tt_hits += 1;
            tt_hit = true;
            tt_move = entry.best_move.map(|em| em.to_chess_move());
            tt_eval = entry.static_eval;
            tt_depth = entry.depth;
            tt_flag = entry.flag;
            tt_score = denormalize_mate(entry.score, ply);

            // A singular verification only skips the *cutoff*; the cached move
            // and static eval still order and prune its subtree well.
            if !singular_search && !is_pv && entry.depth >= depth {
                match entry.flag {
                    TTFlag::Exact => {
                        self.stats.tt_cutoffs += 1;
                        return tt_score;
                    }
                    TTFlag::Beta if tt_score >= beta => {
                        self.stats.tt_cutoffs += 1;
                        return tt_score;
                    }
                    TTFlag::Alpha if tt_score <= alpha => {
                        self.stats.tt_cutoffs += 1;
                        return tt_score;
                    }
                    _ => {}
                }
            }
        }

        let in_check = pos.is_in_check();

        // Static evaluation, reused by several pruning heuristics. It carries
        // no meaning while in check (no "stand pat" option), so we sentinel it.
        let static_eval = if in_check {
            -INFINITY
        } else if tt_eval != TT_EVAL_NONE {
            tt_eval
        } else {
            eval::evaluate(&pos.board, pos.turn)
        };
        self.eval_stack[ply as usize] = static_eval;

        // "Improving": the side to move stands better than it did two plies
        // ago. Non-improving nodes are pruned and reduced more aggressively.
        let improving = !in_check
            && ply >= 2
            && self.eval_stack[ply as usize - 2] != -INFINITY
            && static_eval > self.eval_stack[ply as usize - 2];

        // Whole-node pruning, only at non-PV nodes outside of check, clear of
        // mate scores and outside a singular verification.
        if !is_pv && !in_check && !singular_search && beta.abs() < MATE_THRESHOLD {
            // Reverse futility pruning (static null move): a position far
            // enough above beta is assumed to hold up under a real search.
            // The margin shrinks when the side to move is improving.
            let rfp_margin = RFP_MARGIN_PER_DEPTH * depth - i32::from(improving) * 20 * depth;
            if depth <= RFP_MAX_DEPTH && static_eval - rfp_margin >= beta {
                return static_eval;
            }

            // Razoring: a position far below alpha at shallow depth is verified
            // by a quiescence search and returned if it confirms the failure.
            if depth <= 2 && static_eval + RAZORING_MARGIN <= alpha {
                let qscore = self.quiescence(pos, alpha, beta, ply);
                if qscore <= alpha {
                    return qscore;
                }
            }

            // Adaptive null-move pruning with a high-depth verification search.
            if depth >= 3 && static_eval >= beta && has_non_pawn_material(pos) {
                let r = NULL_MOVE_BASE_REDUCTION + depth / 4 + ((static_eval - beta) / 200).min(2);
                let null_pos = pos.make_null_move();
                let null_score = -self.alpha_beta(
                    &null_pos,
                    depth - 1 - r,
                    -beta,
                    -beta + 1,
                    ply + 1,
                    false,
                    None,
                );
                if null_score >= beta {
                    self.stats.null_cutoffs += 1;
                    if depth < NULL_MOVE_VERIFICATION_DEPTH {
                        // Trust the cutoff, but never return an unproven mate.
                        return if null_score >= MATE_THRESHOLD {
                            beta
                        } else {
                            null_score
                        };
                    }
                    // Zugzwang guard: verify with a reduced-depth real search.
                    let verify =
                        self.alpha_beta(pos, depth - r, beta - 1, beta, ply, false, prev_move);
                    if verify >= beta {
                        return verify;
                    }
                }
            }
        }

        // Internal Iterative Reduction: with no TT move to order on, the subtree
        // is cheapened by one ply; a later, deeper visit re-searches it with a
        // good move first once the TT is populated.
        if tt_move.is_none() && depth >= IIR_MIN_DEPTH {
            depth -= 1;
        }

        // Quiet-move futility flag for the frontier (captures/checks always run).
        let futile = !in_check
            && !is_pv
            && (1..=3).contains(&depth)
            && static_eval + FUTILITY_MARGINS[depth as usize] <= alpha
            && alpha.abs() < MATE_THRESHOLD;

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

        let killers = self.killers[ply as usize];
        // Counter-move: the best known reply to the opponent's previous move.
        let counter = prev_move.and_then(|pm| self.counter_moves[pm.from.index()][pm.to.index()]);
        // Continuation context: the piece the opponent just moved and where to.
        let cont_ctx = prev_move.and_then(|pm| {
            pos.board.get(pm.to).map(|piece| ContinuationContext {
                slot: piece_slot(piece),
                to: pm.to.index(),
            })
        });
        self.cont_stack[ply as usize] = cont_ctx;
        let scored = self.order_moves(
            &moves,
            pos,
            tt_move.as_ref(),
            ply as usize,
            counter.as_ref(),
            cont_ctx,
        );

        // Singular extension eligibility: a deep, trustworthy TT entry whose
        // move may be the *only* good one in this position.
        let singular_candidate = !singular_search
            && tt_hit
            && depth >= SINGULAR_MIN_DEPTH
            && tt_move.is_some()
            && tt_flag != TTFlag::Alpha
            && tt_depth >= depth - SINGULAR_TT_DEPTH_MARGIN
            && tt_score.abs() < MATE_THRESHOLD;

        let mut best_score = -INFINITY;
        let mut best_move: Option<ChessMove> = None;
        let mut flag = TTFlag::Alpha;
        let mut quiet_moves_searched = 0usize;
        // Moves actually searched so far — the first one gets the full window.
        // This is *not* the ordering index `i`, because pruning and singular
        // exclusion can skip the highest-ranked moves.
        let mut searched = 0usize;
        // Quiet moves tried before a cutoff, penalised via the history malus.
        let mut tried_quiets: Vec<ChessMove> = Vec::new();

        for (i, &(mv, _)) in scored.iter().enumerate() {
            if excluded == Some(mv) {
                continue;
            }
            let is_capture = pos.board.get(mv.to).is_some() || mv.is_en_passant;
            let is_quiet = !is_capture && mv.promotion.is_none();
            let child = pos.make_move(&mv);
            let gives_check = child.is_in_check();

            // Frontier futility pruning of quiet, non-checking moves.
            if futile && is_quiet && !gives_check && i > 0 {
                continue;
            }

            // Late Move Pruning: at low depths, skip late quiet moves entirely.
            // Non-improving nodes get a tighter budget.
            if !is_pv
                && !in_check
                && is_quiet
                && !gives_check
                && (1..=4).contains(&depth)
                && quiet_moves_searched
                    >= LMP_THRESHOLDS[depth as usize] / if improving { 1 } else { 2 }
            {
                continue;
            }

            // History pruning: late quiets that have failed repeatedly before
            // are skipped outright at shallow non-PV nodes.
            if !is_pv
                && !in_check
                && is_quiet
                && !gives_check
                && i > 0
                && depth <= HISTORY_PRUNE_MAX_DEPTH
                && quiet_moves_searched >= 2
                && self.quiet_history_score(&pos.board, &mv, cont_ctx)
                    < -HISTORY_PRUNE_PER_DEPTH * depth
            {
                continue;
            }

            // SEE pruning: skip losing captures at shallow non-PV nodes.
            if depth <= SEE_PRUNE_MAX_DEPTH
                && !is_pv
                && is_capture
                && i > 0
                && see(&pos.board, &mv, pos.turn) < 0
            {
                continue;
            }

            if is_quiet {
                quiet_moves_searched += 1;
                tried_quiets.push(mv);
            }
            searched += 1;

            // Check extension: extend by one ply if the move gives check.
            let mut extension = i32::from(gives_check);

            // Singular extension: if every alternative to the TT move fails
            // low against a reduced window, the TT move stands alone and is
            // worth another ply.
            if extension == 0 && singular_candidate && tt_move == Some(mv) {
                let singular_beta = tt_score - SINGULAR_MARGIN_PER_DEPTH * depth;
                self.excluded[ply as usize] = Some(mv);
                let verify = self.alpha_beta(
                    pos,
                    (depth - 1) / 2,
                    singular_beta - 1,
                    singular_beta,
                    ply,
                    false,
                    prev_move,
                );
                self.excluded[ply as usize] = None;
                if !self.should_stop() && verify < singular_beta {
                    extension = 1;
                    self.stats.singular_extensions += 1;
                }
                // The verification search overwrote the shared per-ply state.
                self.path[ply as usize] = pos.hash;
                self.cont_stack[ply as usize] = cont_ctx;
                self.eval_stack[ply as usize] = static_eval;
            }

            let new_depth = depth - 1 + extension;
            let mut score;

            if searched == 1 {
                // First searched move: full-window PV search.
                score =
                    -self.alpha_beta(&child, new_depth, -beta, -alpha, ply + 1, is_pv, Some(mv));
            } else {
                // Late Move Reductions via the precomputed log-log table.
                let mut reduction = 0;
                if depth >= 3 && is_quiet && !gives_check && !in_check && i >= 2 {
                    let d = depth.min(63) as usize;
                    let m = i.min(63);
                    let mut r = LMR_TABLE[d][m] as i32;
                    if is_pv {
                        r -= 1;
                    }
                    if !improving {
                        r += 1;
                    }
                    if killers[0] == Some(mv) || killers[1] == Some(mv) || counter == Some(mv) {
                        r -= 1;
                    }
                    // Ease the reduction for moves with a strong combined
                    // butterfly + continuation history score.
                    r -= (self.quiet_history_score(&pos.board, &mv, cont_ctx) / 4096).clamp(-2, 2);
                    reduction = r.clamp(0, new_depth - 1);
                    if reduction > 0 {
                        self.stats.lmr_searches += 1;
                    }
                }

                // Reduced zero-window search.
                score = -self.alpha_beta(
                    &child,
                    new_depth - reduction,
                    -alpha - 1,
                    -alpha,
                    ply + 1,
                    false,
                    Some(mv),
                );

                // A reduced move that beat alpha is re-searched at full depth.
                if score > alpha && reduction > 0 {
                    score = -self.alpha_beta(
                        &child,
                        new_depth,
                        -alpha - 1,
                        -alpha,
                        ply + 1,
                        false,
                        Some(mv),
                    );
                }

                // Genuine PV moves are re-searched with the full window.
                if score > alpha && score < beta {
                    score = -self.alpha_beta(
                        &child,
                        new_depth,
                        -beta,
                        -alpha,
                        ply + 1,
                        is_pv,
                        Some(mv),
                    );
                }
            }

            if self.should_stop() {
                return 0;
            }

            if score > best_score {
                best_score = score;
                best_move = Some(mv);

                if score > alpha {
                    alpha = score;
                    flag = TTFlag::Exact;

                    if score >= beta {
                        // Beta cutoff.
                        self.stats.beta_cutoffs += 1;
                        flag = TTFlag::Beta;

                        // Reward a quiet cutoff move, penalise the quiets that
                        // failed before it, and refresh the move-ordering tables.
                        if is_quiet {
                            let bonus = (depth * depth).min(HISTORY_MAX);
                            update_history(
                                &mut self.history[mv.from.index()][mv.to.index()],
                                bonus,
                            );
                            self.update_continuation(&pos.board, &mv, cont_ctx, bonus);
                            for &q in &tried_quiets {
                                if q != mv {
                                    update_history(
                                        &mut self.history[q.from.index()][q.to.index()],
                                        -bonus,
                                    );
                                    self.update_continuation(&pos.board, &q, cont_ctx, -bonus);
                                }
                            }

                            let kp = ply as usize;
                            if self.killers[kp][0] != Some(mv) {
                                self.killers[kp][1] = self.killers[kp][0];
                                self.killers[kp][0] = Some(mv);
                            }
                            if let Some(pm) = prev_move {
                                self.counter_moves[pm.from.index()][pm.to.index()] = Some(mv);
                            }
                        }

                        break;
                    }
                }
            }
        }

        // Every move was pruned away: fall back to the static evaluation rather
        // than reporting a bogus `-INFINITY` (only reachable in odd corners of
        // the pruning logic, but it must never leak into the TT).
        if best_score == -INFINITY {
            return if in_check {
                -MATE_SCORE + ply
            } else {
                static_eval
            };
        }

        // A singular verification search describes the node minus one move, so
        // its score must not be cached as the node's score.
        if !singular_search {
            // Store the result, normalising mate scores to be ply-independent
            // and caching the static eval for parent pruning heuristics.
            self.tt.store_with_eval(
                pos.hash,
                depth,
                normalize_mate(best_score, ply),
                flag,
                best_move.as_ref(),
                if in_check { TT_EVAL_NONE } else { static_eval },
            );
        }

        best_score
    }

    /// Adds a continuation-history bonus/malus for a quiet move.
    #[inline]
    fn update_continuation(
        &mut self,
        board: &Board,
        mv: &ChessMove,
        ctx: Option<ContinuationContext>,
        bonus: i32,
    ) {
        if let Some(piece) = board.get(mv.from) {
            self.continuation
                .update(ctx, piece_slot(piece), mv.to.index(), bonus);
        }
    }

    /// Quiescence search: resolves tactical sequences (captures, promotions,
    /// and — when in check — evasions) so the static evaluation is only
    /// trusted in quiet positions. Uses stand-pat, per-capture delta pruning,
    /// SEE pruning of losing captures, and a transposition-table cutoff/store.
    fn quiescence(&mut self, pos: &SearchPosition, mut alpha: i32, beta: i32, ply: i32) -> i32 {
        if self.should_stop() {
            return 0;
        }

        self.stats.nodes += 1;
        self.stats.quiescence_nodes += 1;
        self.seldepth = self.seldepth.max(ply);

        if self.at_limit_checkpoint() && self.hit_hard_limit() {
            self.stopped = true;
            return 0;
        }

        if ply >= MAX_DEPTH {
            return eval::evaluate(&pos.board, pos.turn);
        }

        let alpha_orig = alpha;

        // Transposition-table probe (early cutoff + move-ordering hint).
        let mut tt_move: Option<ChessMove> = None;
        if let Some(entry) = self.tt.probe(pos.hash) {
            self.stats.tt_hits += 1;
            tt_move = entry.best_move.map(|em| em.to_chess_move());
            let tt_score = denormalize_mate(entry.score, ply);
            match entry.flag {
                TTFlag::Exact => return tt_score,
                TTFlag::Beta if tt_score >= beta => return tt_score,
                TTFlag::Alpha if tt_score <= alpha => return tt_score,
                _ => {}
            }
        }

        let in_check = pos.is_in_check();

        // Stand pat — only when not in check (a checked side must reply).
        let stand_pat = if in_check {
            -INFINITY
        } else {
            let e = eval::evaluate(&pos.board, pos.turn);
            if e >= beta {
                return e;
            }
            if e > alpha {
                alpha = e;
            }
            e
        };

        let moves = pos.legal_moves();
        if moves.is_empty() {
            // No legal moves: checkmate while in check, otherwise stalemate.
            return if in_check {
                -MATE_SCORE + ply
            } else {
                DRAW_SCORE
            };
        }

        // In check: search every evasion. Otherwise: captures & promotions only.
        let mut candidates: Vec<ChessMove> = moves
            .into_iter()
            .filter(|mv| {
                in_check
                    || pos.board.get(mv.to).is_some()
                    || mv.is_en_passant
                    || mv.promotion.is_some()
            })
            .collect();
        if candidates.is_empty() {
            return alpha;
        }

        // Order by MVV-LVA, with the TT move tried first.
        candidates.sort_unstable_by_key(|mv| {
            let tt_bonus = if Some(*mv) == tt_move { 1 << 20 } else { 0 };
            std::cmp::Reverse(tt_bonus + mvv_lva_score(&pos.board, mv))
        });

        let mut best_score = stand_pat;
        let mut best_move: Option<ChessMove> = None;

        for mv in candidates {
            let is_capture = pos.board.get(mv.to).is_some() || mv.is_en_passant;

            // Pruning is only safe when not evading check.
            if !in_check && is_capture {
                // Delta pruning: skip captures that cannot raise alpha.
                if mv.promotion.is_none() {
                    let victim = pos
                        .board
                        .get(mv.to)
                        .map(|p| see_piece_value(p.kind))
                        .unwrap_or_else(|| see_piece_value(PieceKind::Pawn));
                    if stand_pat + victim + QS_DELTA_MARGIN < alpha {
                        continue;
                    }
                }
                // SEE pruning: skip losing captures outright.
                if see(&pos.board, &mv, pos.turn) < 0 {
                    continue;
                }
            }

            let child = pos.make_move(&mv);
            let score = -self.quiescence(&child, -beta, -alpha, ply + 1);

            if self.should_stop() {
                return 0;
            }

            if score > best_score {
                best_score = score;
                best_move = Some(mv);
                if score > alpha {
                    alpha = score;
                    if score >= beta {
                        break;
                    }
                }
            }
        }

        // Store the quiescence result at depth 0 for later reuse.
        let flag = if best_score >= beta {
            TTFlag::Beta
        } else if best_score > alpha_orig {
            TTFlag::Exact
        } else {
            TTFlag::Alpha
        };
        self.tt.store(
            pos.hash,
            0,
            normalize_mate(best_score, ply),
            flag,
            best_move.as_ref(),
        );

        best_score
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

/// SEE piece values for exchange evaluation (P, N, B, R, Q, K).
const SEE_VALUES: [i32; 6] = [100, 325, 335, 500, 975, 20000];

/// Returns the SEE value of a piece kind.
fn see_piece_value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => SEE_VALUES[0],
        PieceKind::Knight => SEE_VALUES[1],
        PieceKind::Bishop => SEE_VALUES[2],
        PieceKind::Rook => SEE_VALUES[3],
        PieceKind::Queen => SEE_VALUES[4],
        PieceKind::King => SEE_VALUES[5],
    }
}

/// Sliding directions used by the SEE attacker scan.
const SEE_DIAGONAL_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
const SEE_STRAIGHT_DIRS: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Finds the least valuable piece of `side` that attacks `target`, given
/// the current occupancy mask `occ` (bit = square index). Pieces removed
/// from `occ` are treated as gone, which naturally exposes x-ray attackers
/// behind them on the same ray.
///
/// Returns the attacker's square and kind, or `None`.
fn see_least_valuable_attacker(
    board: &Board,
    occ: u64,
    target: Square,
    side: Color,
) -> Option<(Square, PieceKind)> {
    let occupied = |sq: Square| occ & (1u64 << sq.index()) != 0;
    let piece_at = |sq: Square, kind: PieceKind| {
        occupied(sq)
            && board
                .get(sq)
                .is_some_and(|p| p.color == side && p.kind == kind)
    };

    // Pawns (a pawn of `side` on (file +- 1, rank - pawn_direction) attacks target).
    let dr = -side.pawn_direction();
    for df in [-1i8, 1i8] {
        if let Some(sq) = target.offset(df, dr)
            && piece_at(sq, PieceKind::Pawn)
        {
            return Some((sq, PieceKind::Pawn));
        }
    }

    // Knights.
    const KNIGHT_OFFSETS: [(i8, i8); 8] = [
        (-2, -1),
        (-2, 1),
        (-1, -2),
        (-1, 2),
        (1, -2),
        (1, 2),
        (2, -1),
        (2, 1),
    ];
    for &(df, dr) in &KNIGHT_OFFSETS {
        if let Some(sq) = target.offset(df, dr)
            && piece_at(sq, PieceKind::Knight)
        {
            return Some((sq, PieceKind::Knight));
        }
    }

    // First blocker on a ray from `target` in direction `(df, dr)`.
    let first_blocker = |df: i8, dr: i8| -> Option<Square> {
        let mut cur = target;
        while let Some(next) = cur.offset(df, dr) {
            if occupied(next) {
                return Some(next);
            }
            cur = next;
        }
        None
    };

    // Bishops (diagonal rays).
    for &(df, dr) in &SEE_DIAGONAL_DIRS {
        if let Some(sq) = first_blocker(df, dr)
            && piece_at(sq, PieceKind::Bishop)
        {
            return Some((sq, PieceKind::Bishop));
        }
    }

    // Rooks (straight rays).
    for &(df, dr) in &SEE_STRAIGHT_DIRS {
        if let Some(sq) = first_blocker(df, dr)
            && piece_at(sq, PieceKind::Rook)
        {
            return Some((sq, PieceKind::Rook));
        }
    }

    // Queens (any ray).
    for &(df, dr) in SEE_DIAGONAL_DIRS.iter().chain(SEE_STRAIGHT_DIRS.iter()) {
        if let Some(sq) = first_blocker(df, dr)
            && piece_at(sq, PieceKind::Queen)
        {
            return Some((sq, PieceKind::Queen));
        }
    }

    // King (adjacent squares).
    for dr in -1i8..=1 {
        for df in -1i8..=1 {
            if df == 0 && dr == 0 {
                continue;
            }
            if let Some(sq) = target.offset(df, dr)
                && piece_at(sq, PieceKind::King)
            {
                return Some((sq, PieceKind::King));
            }
        }
    }

    None
}

/// Static Exchange Evaluation: returns the expected material gain (in SEE
/// centipawns, side-to-move perspective) of playing the capture `mv`,
/// assuming both sides keep recapturing with their least valuable attacker
/// and may stand pat at any point (classic swap algorithm).
///
/// X-rays are handled by removing used attackers from the occupancy mask
/// and rescanning rays. King "captures into check" resolve correctly via
/// the huge king value combined with the stand-pat minimax unwinding.
/// Promotions are not modelled (the move scorer ranks them separately).
fn see(board: &Board, mv: &ChessMove, side: Color) -> i32 {
    let target = mv.to;
    let Some(first_attacker) = board.get(mv.from) else {
        return 0;
    };

    // Build the occupancy mask.
    let mut occ = 0u64;
    for rank in 0..8u8 {
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            if board.get(sq).is_some() {
                occ |= 1u64 << sq.index();
            }
        }
    }

    // Victim of the first capture.
    let first_victim = if mv.is_en_passant {
        // Remove the en-passant-captured pawn from its actual square.
        let captured_rank = (mv.to.rank as i8 - side.pawn_direction()) as u8;
        occ &= !(1u64 << Square::new(mv.to.file, captured_rank).index());
        see_piece_value(PieceKind::Pawn)
    } else {
        match board.get(target) {
            Some(p) => see_piece_value(p.kind),
            None => 0,
        }
    };

    // Swap list: gain[d] = best material balance after d captures, assuming
    // optimal stand-pat decisions resolved in the unwinding loop below.
    let mut gain = [0i32; 36];
    let mut d = 0usize;
    gain[0] = first_victim;

    let mut attacker_kind = first_attacker.kind;
    let mut attacker_sq = mv.from;
    let mut stm = side;

    loop {
        d += 1;
        if d >= gain.len() {
            d -= 1;
            break;
        }
        // If the current attacker is captured in turn, the opponent gains it.
        gain[d] = see_piece_value(attacker_kind) - gain[d - 1];
        // Prune: if both stand-pat options are losing, no need to continue.
        if (-gain[d - 1]).max(gain[d]) < 0 {
            break;
        }
        // The attacker has moved to the target square; remove it from `occ`
        // so x-ray attackers behind it are revealed.
        occ &= !(1u64 << attacker_sq.index());
        stm = stm.opponent();
        match see_least_valuable_attacker(board, occ, target, stm) {
            Some((sq, kind)) => {
                attacker_sq = sq;
                attacker_kind = kind;
            }
            None => break,
        }
    }

    // Negamax the swap list backwards (stand-pat allowed at every level).
    while d > 0 {
        gain[d - 1] = -((-gain[d - 1]).max(gain[d]));
        d -= 1;
    }
    gain[0]
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
    fn test_for_level_scales_with_skill() {
        let weak = SearchLimits::for_level(1);
        let strong = SearchLimits::for_level(MAX_SKILL_LEVEL);
        assert!(strong.max_depth > weak.max_depth);
        assert!(strong.move_time_ms.unwrap() > weak.move_time_ms.unwrap());

        // Out-of-range levels clamp to the valid range.
        assert_eq!(
            SearchLimits::for_level(0).max_depth,
            SearchLimits::for_level(1).max_depth
        );
        assert_eq!(
            SearchLimits::for_level(250).max_depth,
            SearchLimits::for_level(MAX_SKILL_LEVEL).max_depth
        );
    }

    #[test]
    fn test_node_limit_bounds_search() {
        let pos = starting_pos();
        let mut engine = SearchEngine::with_defaults();
        let limits = SearchLimits::nodes(5_000);
        let result = engine.search_limited(&pos, &limits, None);
        assert!(result.best_move.is_some(), "should still return a move");
        // The in-tree check fires every NODE_CHECK_INTERVAL nodes, so allow
        // generous slack above the requested node budget.
        assert!(
            result.stats.nodes < 200_000,
            "node limit should bound the search, got {}",
            result.stats.nodes
        );
    }

    /// A terminal position has no move to search, but returning score 0 for a
    /// checkmate made the mating move look like a ~30000 cp blunder to every
    /// caller that diffs the evaluation across it.
    #[test]
    fn test_terminal_positions_score_the_outcome_not_zero() {
        let mated = crate::game::Game::from_fen(
            "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3",
        )
        .unwrap();
        let pos = SearchPosition::new(
            mated.board.clone(),
            mated.turn,
            mated.castling,
            mated.en_passant,
            mated.halfmove_clock,
        );
        let mut engine = SearchEngine::with_defaults();
        let result = engine.search(&pos, 4);
        assert_eq!(
            score_to_mate_in(result.score),
            Some(0),
            "the side to move is checkmated, got {}",
            result.score
        );
        assert!(result.best_move.is_none(), "a mated side has no move");

        // Stalemate is a draw, which really is zero.
        let stalemate = crate::game::Game::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        let pos = SearchPosition::new(
            stalemate.board.clone(),
            stalemate.turn,
            stalemate.castling,
            stalemate.en_passant,
            stalemate.halfmove_clock,
        );
        let result = engine.search(&pos, 4);
        assert_eq!(result.score, 0);
        assert!(result.best_move.is_none());
    }

    #[test]
    fn test_interrupted_search_keeps_the_last_completed_score() {
        // Black is a queen down, so any honest verdict is clearly winning for
        // White. The node budget is small enough that the search is cut off in
        // the middle of an iteration, which used to wipe every root score and
        // leave the caller with `score == 0` and no MultiPV lines at all.
        let game =
            crate::game::Game::from_fen("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
                .unwrap();
        let pos = SearchPosition::new(
            game.board.clone(),
            game.turn,
            game.castling,
            game.en_passant,
            game.halfmove_clock,
        );
        let mut engine = SearchEngine::with_defaults();
        let result = engine.search_limited(&pos, &SearchLimits::nodes(5_000), None);

        assert!(result.best_move.is_some(), "should still return a move");
        assert!(
            result.score > 300,
            "an interrupted search must report the last completed iteration's \
             score, got {}",
            result.score
        );
        assert!(
            !result.pv_lines.is_empty(),
            "an interrupted search must still report its principal variation"
        );
        assert_eq!(
            result.pv_lines[0].score, result.score,
            "the reported score and the top line must agree"
        );
    }

    #[test]
    fn test_finds_forced_mate_in_two() {
        // K+R vs K: White to move mates in two (1.Kb6 Kb8 2.Rh8#).
        let game = crate::game::Game::from_fen("k7/8/2K5/8/8/8/8/7R w - - 0 1").unwrap();
        let pos = SearchPosition::new(
            game.board.clone(),
            game.turn,
            game.castling,
            game.en_passant,
            game.halfmove_clock,
        );
        let mut engine = SearchEngine::with_defaults();
        let result = engine.search(&pos, 8);
        assert_eq!(
            score_to_mate_in(result.score),
            Some(2),
            "engine should announce a forced mate in two (score {})",
            result.score
        );
    }

    #[test]
    fn test_repetition_against_game_history_saves_lost_position() {
        // White is down a whole queen (lone knight vs queen) and is losing.
        // But the knight move Ng1-f3 returns to a position that already
        // occurred earlier in the game — a draw by repetition. The engine must
        // recognise that saving resource and evaluate the position as a draw
        // rather than the lost score the raw material would suggest.
        let game = crate::game::Game::from_fen("k7/8/8/8/1q6/8/8/6NK w - - 40 1").unwrap();
        let pos = SearchPosition::new(
            game.board.clone(),
            game.turn,
            game.castling,
            game.en_passant,
            game.halfmove_clock,
        );

        // Control: with no history, being down a queen is simply losing.
        let mut control = SearchEngine::with_defaults();
        let losing = control.search(&pos, 5);
        assert!(
            losing.score < -300,
            "control: down a queen should be losing, got {}",
            losing.score
        );

        // Tell the engine the position reached after Ng1-f3 already occurred.
        let nf3 = pos
            .legal_moves()
            .into_iter()
            .find(|m| {
                m.from == Square::from_algebraic("g1").unwrap()
                    && m.to == Square::from_algebraic("f3").unwrap()
            })
            .expect("Ng1-f3 must be legal");
        let repeated = pos.make_move(&nf3).hash;

        let mut engine = SearchEngine::with_defaults();
        engine.set_game_history(&[repeated]);
        let saved = engine.search(&pos, 5);
        assert!(
            saved.score.abs() < 50,
            "Nf3 repeats a prior position → engine holds the draw, got {}",
            saved.score
        );
        assert_eq!(
            saved.best_move,
            Some(nf3),
            "engine should choose the saving repetition"
        );
    }

    #[test]
    fn test_search_does_not_clear_external_abort_token() {
        let pos = starting_pos();
        let mut engine = SearchEngine::with_defaults();
        let token = Arc::new(AtomicBool::new(true));
        engine.set_abort_token(token.clone());

        let _ = engine.search(&pos, 3);

        assert!(
            token.load(Ordering::Relaxed),
            "search must not reset externally owned abort token"
        );
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
        let tt = TranspositionTable::new(1);
        let key = 0x12345678;
        tt.store(key, 5, 100, TTFlag::Exact, None);
        let entry = tt.probe(key);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().score, 100);
        assert_eq!(entry.unwrap().depth, 5);
    }

    /// Verify that mate scores stored at one ply are correctly adjusted when
    /// probed at a different ply (i.e. the TT normalization round-trips).
    ///
    /// Engine convention: a forced mate in M plies from a node at ply P scores
    /// as `MATE_SCORE - (P + M)`.  This is because checkmate at ply P+M returns
    /// `-MATE_SCORE + (P+M)`, and each negamax level inverts the sign.
    /// Correspondingly a forced loss in M plies is `-MATE_SCORE + (P + M)`.
    #[test]
    fn test_tt_mate_score_normalization() {
        let mate_in_plies: i32 = 3; // plies until forced checkmate from the store node
        let ply_store: i32 = 2;
        let ply_probe: i32 = 4;

        // Score at ply_store for "mate in mate_in_plies from ply_store".
        let local_score_at_ply_store: i32 = MATE_SCORE - (ply_store + mate_in_plies);

        // Normalize (as done in alpha_beta store path).
        let normalized = if local_score_at_ply_store > MATE_THRESHOLD {
            local_score_at_ply_store + ply_store
        } else if local_score_at_ply_store < -MATE_THRESHOLD {
            local_score_at_ply_store - ply_store
        } else {
            local_score_at_ply_store
        };

        // The normalized value is the ply-independent distance-to-mate.
        assert_eq!(normalized, MATE_SCORE - mate_in_plies);

        // Denormalize at probe ply (as done in alpha_beta probe path).
        let denormalized = if normalized > MATE_THRESHOLD {
            normalized - ply_probe
        } else if normalized < -MATE_THRESHOLD {
            normalized + ply_probe
        } else {
            normalized
        };

        // At ply_probe the score should equal "mate in mate_in_plies from ply_probe".
        let expected_at_ply_probe: i32 = MATE_SCORE - (ply_probe + mate_in_plies);
        assert_eq!(
            denormalized, expected_at_ply_probe,
            "Mate score at probe ply should equal MATE_SCORE - (ply_probe + mate_in_plies)"
        );

        // Round-trip when probe ply == store ply must be exact identity.
        let denormalized_same_ply = if normalized > MATE_THRESHOLD {
            normalized - ply_store
        } else if normalized < -MATE_THRESHOLD {
            normalized + ply_store
        } else {
            normalized
        };
        assert_eq!(
            denormalized_same_ply, local_score_at_ply_store,
            "Round-trip at same ply must be identity"
        );

        // Verify symmetry for a losing score (opponent has mate in mate_in_plies from ply_store).
        let loss_score: i32 = -MATE_SCORE + (ply_store + mate_in_plies);
        let norm_loss = if loss_score > MATE_THRESHOLD {
            loss_score + ply_store
        } else if loss_score < -MATE_THRESHOLD {
            loss_score - ply_store
        } else {
            loss_score
        };
        // Normalized losing score is the ply-independent distance-to-loss.
        assert_eq!(norm_loss, -MATE_SCORE + mate_in_plies);
        let denorm_loss = if norm_loss > MATE_THRESHOLD {
            norm_loss - ply_store
        } else if norm_loss < -MATE_THRESHOLD {
            norm_loss + ply_store
        } else {
            norm_loss
        };
        assert_eq!(
            denorm_loss, loss_score,
            "Losing mate round-trip must be identity"
        );
    }

    #[test]
    fn test_mvv_lva_en_passant_scores_pawn_capture() {
        // Set up a board with a white pawn at e5 and a black pawn at d5.
        // The en passant capture (e5xd6) targets the empty d6 square, so
        // without the fix board.get(mv.to) returns None (victim = 0).
        // With the fix, mvv_lva_score returns pawn×10 - pawn = 9.
        let mut board = Board::default();
        board.set(
            Square::new(4, 4),
            Some(Piece::new(PieceKind::Pawn, Color::White)),
        ); // e5
        board.set(
            Square::new(3, 4),
            Some(Piece::new(PieceKind::Pawn, Color::Black)),
        ); // d5

        let ep_move = ChessMove {
            from: Square::new(4, 4), // e5
            to: Square::new(3, 5),   // d6 (empty en passant target)
            promotion: None,
            is_castling: false,
            is_en_passant: true,
        };

        let score = mvv_lva_score(&board, &ep_move);
        // victim (pawn=1) * 10 - attacker (pawn=1) = 9
        assert_eq!(
            score, 9,
            "en passant must be scored as a pawn capture (victim non-zero)"
        );
        assert!(score > 0, "mvv_lva_score for en passant must be positive");
    }

    /// Verify that the incremental Zobrist hash in `make_move` always matches
    /// the full `hash_position` recomputation.
    #[test]
    fn test_incremental_hash_matches_full_hash() {
        let pos = starting_pos();
        for mv in pos.legal_moves() {
            let child = pos.make_move(&mv);
            let expected =
                zobrist::hash_position(&child.board, child.turn, &child.castling, child.en_passant);
            assert_eq!(
                child.hash, expected,
                "Incremental hash mismatch after move {:?}",
                mv
            );
        }
    }

    /// Verify incremental hash is correct in a position where castling and
    /// en passant may become available in child positions.
    #[test]
    fn test_incremental_hash_castling_and_ep() {
        // Set up a position after 1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5
        // White can castle kingside; en passant may become available in child
        // positions after a pawn double-push (not present in the initial pos).
        let mut board = Board::starting_position();
        let moves = [
            ChessMove::simple(Square::new(4, 1), Square::new(4, 3)), // e4
            ChessMove::simple(Square::new(4, 6), Square::new(4, 4)), // e5
            ChessMove::simple(Square::new(6, 0), Square::new(5, 2)), // Nf3
            ChessMove::simple(Square::new(1, 7), Square::new(2, 5)), // Nc6
            ChessMove::simple(Square::new(5, 0), Square::new(2, 3)), // Bc4
            ChessMove::simple(Square::new(5, 7), Square::new(2, 4)), // Bc5
        ];
        let colors = [
            Color::White,
            Color::Black,
            Color::White,
            Color::Black,
            Color::White,
            Color::Black,
        ];
        for (mv, col) in moves.iter().zip(colors.iter()) {
            movegen::apply_move_to_board(&mut board, mv, *col);
        }
        // f1 and g1 are already clear after the Bc4 and Nf3 moves above.
        let castling = CastlingRights::default(); // All castling rights enabled by default
        let pos = SearchPosition::new(board, Color::White, castling, None, 0);

        for mv in pos.legal_moves() {
            let child = pos.make_move(&mv);
            let expected =
                zobrist::hash_position(&child.board, child.turn, &child.castling, child.en_passant);
            assert_eq!(
                child.hash, expected,
                "Incremental hash mismatch after move {:?}",
                mv
            );
        }
    }

    /// Verify that the incremental hash is correct for an en-passant capture.
    #[test]
    fn test_incremental_hash_en_passant_capture() {
        // Position: white pawn on e5, black pawn just double-pushed to d5.
        // En passant target is d6 (Square::new(3, 5)).
        let mut board = Board::default();
        board.set(
            Square::new(4, 4), // e5
            Some(Piece::new(PieceKind::Pawn, Color::White)),
        );
        board.set(
            Square::new(3, 4), // d5
            Some(Piece::new(PieceKind::Pawn, Color::Black)),
        );
        // Add kings so that the position is minimally legal
        board.set(
            Square::new(4, 0), // e1
            Some(Piece::new(PieceKind::King, Color::White)),
        );
        board.set(
            Square::new(4, 7), // e8
            Some(Piece::new(PieceKind::King, Color::Black)),
        );

        let ep_square = Square::new(3, 5); // d6
        let pos = SearchPosition::new(
            board,
            Color::White,
            CastlingRights::default(),
            Some(ep_square),
            0,
        );

        // Find the en-passant capture in the legal moves
        let ep_move = pos
            .legal_moves()
            .into_iter()
            .find(|m| m.is_en_passant)
            .expect("en-passant capture must be legal in this position");

        let child = pos.make_move(&ep_move);
        let expected =
            zobrist::hash_position(&child.board, child.turn, &child.castling, child.en_passant);
        assert_eq!(
            child.hash, expected,
            "Incremental hash mismatch after en-passant capture {:?}",
            ep_move
        );
        // The captured black pawn should be gone
        assert!(
            child.board.get(Square::new(3, 4)).is_none(),
            "Captured en-passant pawn should be removed from d5"
        );
    }

    /// Verify that `make_null_move` produces the correct incremental hash.
    #[test]
    fn test_null_move_incremental_hash() {
        let pos = starting_pos();
        let null = pos.make_null_move();
        let expected =
            zobrist::hash_position(&null.board, null.turn, &null.castling, null.en_passant);
        assert_eq!(null.hash, expected, "Null move incremental hash mismatch");
    }

    // -----------------------------------------------------------------------
    // Perft, mate-in-N, and TT-reuse tests
    // -----------------------------------------------------------------------

    /// Minimal FEN parser for tests only. Accepts the four mandatory fields
    /// (placement, side, castling, en-passant) and the optional halfmove
    /// clock. Panics on invalid input — intentional, tests should use
    /// well-formed FENs.
    fn position_from_fen(fen: &str) -> SearchPosition {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        assert!(parts.len() >= 4, "FEN must have at least 4 fields");

        let mut board = Board::default();
        let ranks: Vec<&str> = parts[0].split('/').collect();
        assert_eq!(ranks.len(), 8, "FEN placement must have 8 ranks");
        for (row_idx, row) in ranks.iter().enumerate() {
            let rank = 7 - row_idx as u8;
            let mut file: u8 = 0;
            for ch in row.chars() {
                if let Some(d) = ch.to_digit(10) {
                    file += d as u8;
                } else {
                    let piece =
                        Piece::from_fen_char(ch).unwrap_or_else(|| panic!("bad FEN piece {ch}"));
                    board.set(Square::new(file, rank), Some(piece));
                    file += 1;
                }
            }
            assert_eq!(file, 8, "FEN rank must cover 8 files");
        }

        let turn = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            other => panic!("bad FEN side {other}"),
        };

        let mut castling = CastlingRights::default();
        // CastlingRights::default() may grant all rights; reset and parse.
        castling.white.kingside = false;
        castling.white.queenside = false;
        castling.black.kingside = false;
        castling.black.queenside = false;
        if parts[2] != "-" {
            for ch in parts[2].chars() {
                match ch {
                    'K' => castling.white.kingside = true,
                    'Q' => castling.white.queenside = true,
                    'k' => castling.black.kingside = true,
                    'q' => castling.black.queenside = true,
                    other => panic!("bad castling char {other}"),
                }
            }
        }

        let en_passant = if parts[3] == "-" {
            None
        } else {
            Some(Square::from_algebraic(parts[3]).expect("bad EP square"))
        };

        let halfmove = parts
            .get(4)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        SearchPosition::new(board, turn, castling, en_passant, halfmove)
    }

    /// Counts leaf nodes at the given depth (standard perft).
    fn perft(pos: &SearchPosition, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        let moves = pos.legal_moves();
        if depth == 1 {
            return moves.len() as u64;
        }
        let mut total = 0u64;
        for mv in moves {
            total += perft(&pos.make_move(&mv), depth - 1);
        }
        total
    }

    #[test]
    fn perft_startpos_depth_1() {
        let pos = starting_pos();
        assert_eq!(perft(&pos, 1), 20);
    }

    #[test]
    fn perft_startpos_depth_2() {
        let pos = starting_pos();
        assert_eq!(perft(&pos, 2), 400);
    }

    #[test]
    fn perft_startpos_depth_3() {
        let pos = starting_pos();
        assert_eq!(perft(&pos, 3), 8_902);
    }

    /// Depth-4 startpos perft. Slower (~200k nodes); opt-in via
    /// `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn perft_startpos_depth_4() {
        let pos = starting_pos();
        assert_eq!(perft(&pos, 4), 197_281);
    }

    #[test]
    fn perft_kiwipete_depth_1() {
        let pos = position_from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        );
        assert_eq!(perft(&pos, 1), 48);
    }

    #[test]
    fn perft_kiwipete_depth_2() {
        let pos = position_from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        );
        assert_eq!(perft(&pos, 2), 2_039);
    }

    /// Kiwipete depth-3 perft (~98k nodes). Opt-in via
    /// `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn perft_kiwipete_depth_3() {
        let pos = position_from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        );
        assert_eq!(perft(&pos, 3), 97_862);
    }

    /// Verify search prefers an immediate back-rank mate and returns a
    /// mate-range score for the position `6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1`.
    #[test]
    fn test_search_finds_back_rank_mate_in_one() {
        // Back-rank mate: White rook on a1, Black king trapped on g8 by own pawns.
        // FEN: `6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1`
        // 1.Ra8# is mate (Black king has no escape — f7, g7, h7 blocked).
        let pos = position_from_fen("6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1");
        let mut engine = SearchEngine::with_defaults();
        let result = engine.search(&pos, 4);
        assert!(result.best_move.is_some());
        let mv = result.best_move.unwrap();
        // Best move should land the rook on the 8th rank to deliver mate.
        assert_eq!(
            mv.to.rank, 7,
            "Mating move should be a rook lift to the 8th rank, got {:?}",
            mv
        );
        assert!(
            result.score > MATE_THRESHOLD,
            "Score must be in mate range, got {}",
            result.score
        );
    }

    /// Performance sanity benchmark: prints nodes-per-second for a depth-10
    /// search from the starting position. Opt-in via
    /// `cargo test --release -- --ignored bench_nps --nocapture`.
    #[test]
    #[ignore]
    fn bench_nps_depth_10_startpos() {
        let pos = starting_pos();
        let mut engine = SearchEngine::with_defaults();
        let result = engine.search(&pos, 10);
        let nodes = result.stats.nodes + result.stats.quiescence_nodes;
        let scaled = nodes * 1000;
        let nps = scaled.checked_div(result.time_ms).unwrap_or(scaled);
        println!(
            "bench_nps_depth_10_startpos: nodes={} qnodes={} time_ms={} nps={}",
            result.stats.nodes, result.stats.quiescence_nodes, result.time_ms, nps
        );
        assert!(result.best_move.is_some());
    }

    /// Diagnostic: prints node counts for a few fixed positions at fixed
    /// depth. Used to compare pruning effectiveness across engine changes.
    /// Opt-in via `cargo test --release -- --ignored bench_nodes --nocapture`.
    #[test]
    #[ignore]
    fn bench_nodes_fixed_positions() {
        let cases = [
            ("startpos", starting_pos(), 9),
            (
                "kiwipete",
                position_from_fen(
                    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
                ),
                8,
            ),
            (
                "endgame",
                position_from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
                10,
            ),
        ];
        for (name, pos, depth) in cases {
            let mut engine = SearchEngine::with_defaults();
            let result = engine.search(&pos, depth);
            println!(
                "bench_nodes {name}: depth={} nodes={} qnodes={} time_ms={} score={}",
                depth,
                result.stats.nodes,
                result.stats.quiescence_nodes,
                result.time_ms,
                result.score
            );
        }
    }

    /// Verify the transposition table is actually reused across iterative
    /// deepening iterations and across two consecutive searches.
    #[test]
    fn test_tt_reuse_across_searches() {
        let pos = starting_pos();
        let mut engine = SearchEngine::with_defaults();

        // First search at depth 4 populates the TT.
        let r1 = engine.search(&pos, 4);
        assert!(r1.stats.tt_hits > 0, "depth-4 ID must accumulate TT hits");

        // Second search at the same depth should benefit even more: every
        // root child has its TT entry available immediately.
        let r2 = engine.search(&pos, 4);
        assert!(
            r2.stats.tt_hits >= r1.stats.tt_hits / 2,
            "second search should retain meaningful TT reuse (r1={}, r2={})",
            r1.stats.tt_hits,
            r2.stats.tt_hits,
        );
    }
}
