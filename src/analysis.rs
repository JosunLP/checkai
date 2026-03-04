//! Game analysis module for the CheckAI chess engine.
//!
//! Provides deep, asynchronous analysis of chess games with:
//! - Move quality classification (Best / Excellent / Good / Inaccuracy / Mistake / Blunder)
//! - Centipawn loss computation
//! - Opening book annotation
//! - Endgame tablebase integration
//! - Minimum 30-ply search depth
//!
//! ## Architecture
//!
//! All analysis runs on a **separate worker pool**, fully decoupled from
//! the core engine's game loop. The analysis pipeline operates on
//! **read-only snapshots** of the game state — no mutable references to
//! live game data are ever held.
//!
//! Analysis results are routed exclusively to the `/api/analysis/*`
//! endpoints, which are architecturally separated from the player-facing
//! `/api/games/*` endpoints.
//!
//! ## Pipeline
//!
//! For each position in the game:
//!
//! ```text
//! Position → Opening book? ──Yes──▶ Book annotation
//!                 │ No
//!                 ▼
//!            Tablebase? ──Yes──▶ Tablebase result (WDL/DTZ)
//!                 │ No
//!                 ▼
//!            Deep search (min. 30 plies)
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::game::Game;
use crate::opening_book::{BookMoveInfo, OpeningBook};
use crate::search::{MAX_DEPTH, SearchEngine, SearchPosition};
use crate::storage;
use crate::tablebase::{SyzygyTablebase, TablebaseInfo, WDL};
use crate::types::*;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the analysis engine.
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Minimum search depth in plies (must be ≥ 30).
    pub min_depth: u32,
    /// Path to a Polyglot opening book file (`.bin`).
    pub book_path: Option<PathBuf>,
    /// Path to a Syzygy tablebase directory.
    pub tablebase_path: Option<PathBuf>,
    /// Transposition table size in MB.
    pub tt_size_mb: usize,
    /// Maximum number of jobs retained in memory.
    pub max_jobs_retained: usize,
    /// Maximum number of active jobs (Queued + InProgress).
    pub max_concurrent_jobs: usize,
    /// Time-to-live in seconds for finished jobs (Completed/Failed/Cancelled).
    ///
    /// If `None`, finished jobs are only removed by capacity-based eviction.
    pub completed_job_ttl_secs: Option<u64>,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            min_depth: 30,
            book_path: None,
            tablebase_path: None,
            tt_size_mb: 64,
            max_jobs_retained: 256,
            max_concurrent_jobs: 4,
            completed_job_ttl_secs: Some(60 * 60),
        }
    }
}

/// Error returned when an analysis submission cannot be accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisSubmitError {
    /// Too many active jobs are already queued/running.
    ConcurrentLimitExceeded {
        active_jobs: usize,
        max_concurrent_jobs: usize,
    },
    /// Job store reached capacity and no further eviction was possible.
    JobStoreLimitExceeded {
        stored_jobs: usize,
        max_jobs_retained: usize,
    },
}

// ---------------------------------------------------------------------------
// Move quality classification
// ---------------------------------------------------------------------------

/// Quality classification for a played move.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
pub enum MoveQuality {
    /// The played move is the engine's top choice.
    Best,
    /// ≤ 10 centipawn loss — nearly optimal.
    Excellent,
    /// 11–25 centipawn loss — solid play.
    Good,
    /// 26–50 centipawn loss — slight inaccuracy.
    Inaccuracy,
    /// 51–100 centipawn loss — significant error.
    Mistake,
    /// > 100 centipawn loss or misses forced mate.
    Blunder,
    /// The move is a known book move (not evaluated against search).
    Book,
}

impl MoveQuality {
    /// Classifies move quality based on centipawn loss.
    pub fn from_cp_loss(cp_loss: i32) -> Self {
        match cp_loss {
            0 => MoveQuality::Best,
            1..=10 => MoveQuality::Excellent,
            11..=25 => MoveQuality::Good,
            26..=50 => MoveQuality::Inaccuracy,
            51..=100 => MoveQuality::Mistake,
            _ => MoveQuality::Blunder,
        }
    }
}

impl std::fmt::Display for MoveQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveQuality::Best => write!(f, "Best"),
            MoveQuality::Excellent => write!(f, "Excellent"),
            MoveQuality::Good => write!(f, "Good"),
            MoveQuality::Inaccuracy => write!(f, "Inaccuracy"),
            MoveQuality::Mistake => write!(f, "Mistake"),
            MoveQuality::Blunder => write!(f, "Blunder"),
            MoveQuality::Book => write!(f, "Book"),
        }
    }
}

// ---------------------------------------------------------------------------
// Analysis result types
// ---------------------------------------------------------------------------

/// Annotation for a single move in the analysis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct MoveAnnotation {
    /// Move number (fullmove number).
    pub move_number: u32,
    /// Which side played.
    pub side: Color,
    /// The move that was actually played.
    pub played_move: MoveJson,
    /// The best move according to analysis.
    pub best_move: MoveJson,
    /// Evaluation of the position after the played move (centipawns).
    pub played_eval: i32,
    /// Evaluation of the position after the best move (centipawns).
    pub best_eval: i32,
    /// Centipawn loss (best_eval - played_eval, ≥ 0).
    pub centipawn_loss: i32,
    /// Quality classification.
    pub quality: MoveQuality,
    /// Whether the position was in the opening book.
    pub is_book_move: bool,
    /// Whether the position was in the tablebase.
    pub is_tablebase_position: bool,
    /// Opening book information (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_info: Option<BookMoveInfo>,
    /// Tablebase information (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tablebase_info: Option<TablebaseInfo>,
    /// Search depth achieved for this position.
    pub search_depth: u32,
    /// Principal variation (best continuation).
    pub principal_variation: Vec<String>,
}

/// Summary statistics for a complete game analysis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct AnalysisSummary {
    /// Total number of moves analyzed.
    pub total_moves: usize,
    /// Number of best moves.
    pub best_moves: usize,
    /// Number of excellent moves.
    pub excellent_moves: usize,
    /// Number of good moves.
    pub good_moves: usize,
    /// Number of inaccuracies.
    pub inaccuracies: usize,
    /// Number of mistakes.
    pub mistakes: usize,
    /// Number of blunders.
    pub blunders: usize,
    /// Number of book moves.
    pub book_moves: usize,
    /// Average centipawn loss (excluding book moves).
    pub average_centipawn_loss: f64,
    /// White's accuracy percentage.
    pub white_accuracy: f64,
    /// Black's accuracy percentage.
    pub black_accuracy: f64,
    /// Average centipawn loss for White.
    pub white_avg_cp_loss: f64,
    /// Average centipawn loss for Black.
    pub black_avg_cp_loss: f64,
}

/// The complete result of a game analysis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct AnalysisResult {
    /// Per-move annotations.
    pub annotations: Vec<MoveAnnotation>,
    /// Summary statistics.
    pub summary: AnalysisSummary,
    /// Search depth used.
    pub depth: u32,
    /// Whether an opening book was used.
    pub book_available: bool,
    /// Whether a tablebase was available.
    pub tablebase_available: bool,
}

// ---------------------------------------------------------------------------
// Analysis job management
// ---------------------------------------------------------------------------

/// Status of an analysis job.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub enum AnalysisStatus {
    /// The job is waiting in the queue.
    Queued,
    /// The job is currently being processed.
    InProgress {
        /// Number of moves analyzed so far.
        moves_analyzed: usize,
        /// Total moves to analyze.
        total_moves: usize,
    },
    /// The analysis is complete.
    Completed,
    /// The analysis failed.
    Failed {
        /// Error message.
        error: String,
    },
    /// The job was cancelled.
    Cancelled,
}

/// An analysis job tracked by the manager.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct AnalysisJob {
    /// Unique job identifier.
    pub id: String,
    /// Associated game ID (if analyzing a specific game).
    pub game_id: Option<String>,
    /// Current status.
    pub status: AnalysisStatus,
    /// Analysis results (available when status is Completed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AnalysisResult>,
    /// Timestamp when the job was created.
    pub created_at: u64,
    /// Timestamp when the job completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
}

/// Outcome of an [`AnalysisManager::delete_job`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteJobOutcome {
    /// The job was active (Queued/InProgress) and has been cancelled.
    Cancelled,
    /// The job was already finished and has been removed from the store.
    Deleted,
}

/// Brief summary of a job (for listing).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobSummary {
    pub id: String,
    pub game_id: Option<String>,
    pub status: AnalysisStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

// ---------------------------------------------------------------------------
// Analysis manager
// ---------------------------------------------------------------------------

/// The central analysis manager.
///
/// Manages the analysis job queue, opening book, and tablebase.
/// All analysis runs asynchronously on background tasks, completely
/// decoupled from the core game engine.
pub struct AnalysisManager {
    /// Analysis configuration.
    config: AnalysisConfig,
    /// Opening book (loaded once at startup).
    book: Option<OpeningBook>,
    /// Syzygy tablebase (loaded once at startup).
    tablebase: Option<SyzygyTablebase>,
    /// Job store (thread-safe).
    jobs: Arc<RwLock<HashMap<String, AnalysisJob>>>,
    /// Cancellation flags for in-progress jobs.
    cancel_tokens: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
}

impl AnalysisManager {
    /// Creates a new analysis manager with the given configuration.
    pub fn new(config: AnalysisConfig) -> Self {
        let mut config = config;
        // Keep limits sane even if config comes from external input.
        config.max_jobs_retained = config.max_jobs_retained.max(1);
        config.max_concurrent_jobs = config.max_concurrent_jobs.max(1);

        // Load opening book
        let book = config
            .book_path
            .as_ref()
            .and_then(|path| match OpeningBook::load(path) {
                Ok(b) => {
                    log::info!(
                        "Opening book loaded: {} entries from {}",
                        b.len(),
                        path.display()
                    );
                    Some(b)
                }
                Err(e) => {
                    log::warn!("Failed to load opening book: {}", e);
                    None
                }
            });

        // Load tablebase
        let tablebase =
            config
                .tablebase_path
                .as_ref()
                .and_then(|path| match SyzygyTablebase::load(path) {
                    Ok(tb) => {
                        log::info!(
                            "Syzygy tablebase loaded: max {} pieces from {}",
                            tb.max_pieces,
                            path.display()
                        );
                        Some(tb)
                    }
                    Err(e) => {
                        log::warn!("Failed to load Syzygy tablebase: {}", e);
                        None
                    }
                });

        Self {
            config,
            book,
            tablebase,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Submits a game for analysis (by game snapshot).
    ///
    /// The game is cloned (read-only snapshot) and analysis runs on a
    /// background task. Returns the job ID immediately.
    pub async fn analyze_game(
        &self,
        game: &Game,
        depth: Option<u32>,
    ) -> Result<String, AnalysisSubmitError> {
        let job_id = Uuid::new_v4().to_string();
        let depth = depth
            .unwrap_or(self.config.min_depth)
            .max(self.config.min_depth);
        let now = storage::unix_timestamp();

        let job = AnalysisJob {
            id: job_id.clone(),
            game_id: Some(game.id.to_string()),
            status: AnalysisStatus::Queued,
            result: None,
            created_at: now,
            completed_at: None,
        };

        let evicted_job_ids = {
            let mut jobs = self.jobs.write().await;

            let evicted = Self::evict_jobs_locked(
                &mut jobs,
                now,
                self.config.completed_job_ttl_secs,
                self.config.max_jobs_retained,
                1,
            );

            let active_jobs = jobs
                .values()
                .filter(|j| Self::is_active_status(&j.status))
                .count();
            if active_jobs >= self.config.max_concurrent_jobs {
                return Err(AnalysisSubmitError::ConcurrentLimitExceeded {
                    active_jobs,
                    max_concurrent_jobs: self.config.max_concurrent_jobs,
                });
            }

            if jobs.len() >= self.config.max_jobs_retained {
                return Err(AnalysisSubmitError::JobStoreLimitExceeded {
                    stored_jobs: jobs.len(),
                    max_jobs_retained: self.config.max_jobs_retained,
                });
            }

            jobs.insert(job_id.clone(), job);
            evicted
        };

        if !evicted_job_ids.is_empty() {
            let mut tokens = self.cancel_tokens.write().await;
            for id in evicted_job_ids {
                tokens.remove(&id);
            }
        }

        // Create cancellation token for this job
        let cancel_token = Arc::new(AtomicBool::new(false));
        {
            let mut tokens = self.cancel_tokens.write().await;
            tokens.insert(job_id.clone(), cancel_token.clone());
        }

        // Create read-only snapshot
        let snapshot = game.clone();
        let jobs = self.jobs.clone();
        let cancel_tokens = self.cancel_tokens.clone();
        let tt_size = self.config.tt_size_mb;

        // Determine book/tablebase availability flags.
        // For tablebase, only report true when actual Syzygy files are present.
        let has_book = self.book.is_some();
        let has_tablebase = self.tablebase.as_ref().is_some_and(|tb| tb.is_available());

        // For book/tablebase probing we pre-probe on the calling thread so
        // the results can be moved into the spawned task without requiring
        // `&self` to be `'static`.
        let book_results = self.pre_probe_book(&snapshot);
        let tablebase_results = self.pre_probe_tablebase(&snapshot);

        let jid = job_id.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                // Update status to InProgress only if the job has not been
                // cancelled before this task actually started running.
                {
                    let mut jobs_lock = jobs.write().await;
                    if let Some(job) = jobs_lock.get_mut(&jid)
                        && matches!(job.status, AnalysisStatus::Queued)
                        && !cancel_token.load(Ordering::Relaxed)
                    {
                        job.status = AnalysisStatus::InProgress {
                            moves_analyzed: 0,
                            total_moves: snapshot.move_history.len(),
                        };
                    }
                }

                let result = run_analysis(RunAnalysisParams {
                    game: &snapshot,
                    depth,
                    tt_size_mb: tt_size,
                    has_book,
                    has_tablebase,
                    book_results: &book_results,
                    tablebase_results: &tablebase_results,
                    jobs: &jobs,
                    job_id: &jid,
                    cancel_token: &cancel_token,
                })
                .await;

                // Store result
                {
                    let mut jobs_lock = jobs.write().await;
                    if let Some(job) = jobs_lock.get_mut(&jid) {
                        // Never overwrite a Cancelled status set by delete_job.
                        if !matches!(job.status, AnalysisStatus::Cancelled) {
                            match result {
                                Ok(analysis) => {
                                    job.status = AnalysisStatus::Completed;
                                    job.result = Some(analysis);
                                }
                                Err(e) => {
                                    job.status = AnalysisStatus::Failed {
                                        error: e.to_string(),
                                    };
                                }
                            }
                            job.completed_at = Some(storage::unix_timestamp());
                        }
                    }
                }

                // Clean up the cancellation token
                {
                    let mut tokens = cancel_tokens.write().await;
                    tokens.remove(&jid);
                }
            });
        });

        Ok(job_id)
    }

    fn is_active_status(status: &AnalysisStatus) -> bool {
        matches!(
            status,
            AnalysisStatus::Queued | AnalysisStatus::InProgress { .. }
        )
    }

    fn is_finished_status(status: &AnalysisStatus) -> bool {
        matches!(
            status,
            AnalysisStatus::Completed | AnalysisStatus::Failed { .. } | AnalysisStatus::Cancelled
        )
    }

    /// Evicts old/finished jobs according to TTL and capacity requirements.
    ///
    /// Returns the list of evicted job IDs so callers can also remove
    /// corresponding cancellation tokens.
    fn evict_jobs_locked(
        jobs: &mut HashMap<String, AnalysisJob>,
        now: u64,
        finished_ttl_secs: Option<u64>,
        max_jobs_retained: usize,
        reserve_slots: usize,
    ) -> Vec<String> {
        let mut evicted_ids = Vec::new();

        // 1) TTL-based eviction for finished jobs.
        if let Some(ttl) = finished_ttl_secs {
            let expired: Vec<String> = jobs
                .iter()
                .filter_map(|(id, job)| {
                    if !Self::is_finished_status(&job.status) {
                        return None;
                    }
                    let finished_at = job.completed_at.unwrap_or(job.created_at);
                    let age = now.saturating_sub(finished_at);
                    (age >= ttl).then(|| id.clone())
                })
                .collect();

            for id in expired {
                if jobs.remove(&id).is_some() {
                    evicted_ids.push(id);
                }
            }
        }

        // 2) Capacity-based eviction (oldest finished jobs first).
        let target_len = max_jobs_retained.saturating_sub(reserve_slots);
        if jobs.len() > target_len {
            let mut finished_candidates: Vec<(String, u64)> = jobs
                .iter()
                .filter(|(_, job)| Self::is_finished_status(&job.status))
                .map(|(id, job)| (id.clone(), job.completed_at.unwrap_or(job.created_at)))
                .collect();
            finished_candidates.sort_by_key(|(_, ts)| *ts);

            for (id, _) in finished_candidates {
                if jobs.len() <= target_len {
                    break;
                }
                if jobs.remove(&id).is_some() {
                    evicted_ids.push(id);
                }
            }
        }

        evicted_ids
    }

    /// Pre-probes the opening book for all positions in the game.
    fn pre_probe_book(&self, game: &Game) -> Vec<Option<BookMoveInfo>> {
        let Some(book) = &self.book else {
            return vec![None; game.move_history.len()];
        };

        let mut results = Vec::new();
        let mut replay = Game::new();

        for record in &game.move_history {
            // Probe the book BEFORE the move is made
            let legal = replay.legal_moves();
            let chess_move = legal.iter().find(|m| {
                m.from.to_algebraic() == record.move_json.from
                    && m.to.to_algebraic() == record.move_json.to
                    && m.promotion
                        == record
                            .move_json
                            .promotion
                            .as_ref()
                            .and_then(|p| match p.as_str() {
                                "Q" => Some(PieceKind::Queen),
                                "R" => Some(PieceKind::Rook),
                                "B" => Some(PieceKind::Bishop),
                                "N" => Some(PieceKind::Knight),
                                _ => None,
                            })
            });

            if let Some(cm) = chess_move {
                let info = book.probe_move(
                    &replay.board,
                    replay.turn,
                    &replay.castling,
                    replay.en_passant,
                    cm,
                );
                results.push(Some(info));
            } else {
                results.push(None);
            }

            // Replay the move to advance the position
            if replay.make_move(&record.move_json).is_err() {
                break;
            }
        }

        results
    }

    /// Pre-probes the tablebase for all positions in the game.
    ///
    /// Only probes when actual Syzygy files are present (`tb.is_available()`),
    /// so that per-move `is_tablebase_position` flags stay consistent with
    /// the top-level `AnalysisResult::tablebase_available` flag.
    fn pre_probe_tablebase(&self, game: &Game) -> Vec<Option<TablebaseInfo>> {
        let Some(tb) = &self.tablebase else {
            return vec![None; game.move_history.len()];
        };

        // No real tablebase files → skip probing entirely so analytical
        // hits don't appear while tablebase_available is false.
        if !tb.is_available() {
            return vec![None; game.move_history.len()];
        }

        let mut results = Vec::new();
        let mut replay = Game::new();

        for record in &game.move_history {
            // Probe the tablebase AFTER the move
            if replay.make_move(&record.move_json).is_err() {
                results.push(None);
                break;
            }
            let info = tb.probe(
                &replay.board,
                replay.turn,
                &replay.castling,
                replay.en_passant,
            );
            if info.is_tablebase_position {
                results.push(Some(info));
            } else {
                results.push(None);
            }
        }

        results
    }

    /// Gets the status and result of an analysis job.
    pub async fn get_job(&self, job_id: &str) -> Option<AnalysisJob> {
        let now = storage::unix_timestamp();
        let (job, evicted) = {
            let mut jobs = self.jobs.write().await;
            let evicted = Self::evict_jobs_locked(
                &mut jobs,
                now,
                self.config.completed_job_ttl_secs,
                self.config.max_jobs_retained,
                0,
            );
            let job = jobs.get(job_id).cloned();
            (job, evicted)
        };

        if !evicted.is_empty() {
            let mut tokens = self.cancel_tokens.write().await;
            for id in evicted {
                tokens.remove(&id);
            }
        }

        job
    }

    /// Lists all analysis jobs (summaries).
    pub async fn list_jobs(&self) -> Vec<AnalysisJobSummary> {
        let now = storage::unix_timestamp();
        let (summaries, evicted) = {
            let mut jobs = self.jobs.write().await;
            let evicted = Self::evict_jobs_locked(
                &mut jobs,
                now,
                self.config.completed_job_ttl_secs,
                self.config.max_jobs_retained,
                0,
            );
            let summaries = jobs
                .values()
                .map(|j| AnalysisJobSummary {
                    id: j.id.clone(),
                    game_id: j.game_id.clone(),
                    status: j.status.clone(),
                    created_at: j.created_at,
                    completed_at: j.completed_at,
                })
                .collect();
            (summaries, evicted)
        };

        if !evicted.is_empty() {
            let mut tokens = self.cancel_tokens.write().await;
            for id in evicted {
                tokens.remove(&id);
            }
        }

        summaries
    }

    /// Cancels an in-progress / queued job or removes a finished job.
    ///
    /// For jobs that are still running (Queued / InProgress) this sets the
    /// cancellation flag so the analysis loop stops at the next iteration,
    /// marks the status as `Cancelled`, and **keeps the job in the store**
    /// (so callers can still retrieve the cancelled status).  A subsequent
    /// call for the same job ID will then fall into the finished-job branch
    /// and remove it entirely.
    ///
    /// Jobs that are already finished (Completed / Failed / Cancelled) are
    /// removed from the store entirely.
    pub async fn delete_job(&self, job_id: &str) -> Option<DeleteJobOutcome> {
        enum Action {
            CancelActive,
            DeleteFinished,
        }

        // Decide and update job state while holding only the jobs lock.
        // Do not await on other locks in this scope.
        let action = {
            let mut jobs = self.jobs.write().await;
            let job = jobs.get_mut(job_id)?;

            match &job.status {
                AnalysisStatus::Queued | AnalysisStatus::InProgress { .. } => {
                    job.status = AnalysisStatus::Cancelled;
                    job.completed_at = Some(storage::unix_timestamp());
                    Action::CancelActive
                }
                // Already finished — safe to remove completely
                AnalysisStatus::Completed
                | AnalysisStatus::Failed { .. }
                | AnalysisStatus::Cancelled => {
                    jobs.remove(job_id);
                    Action::DeleteFinished
                }
            }
        };

        match action {
            Action::CancelActive => {
                // Signal the background task to stop.
                // Done after releasing the jobs lock to avoid lock-order issues.
                let token = {
                    let tokens = self.cancel_tokens.read().await;
                    tokens.get(job_id).cloned()
                };
                if let Some(token) = token {
                    token.store(true, Ordering::Relaxed);
                }
                Some(DeleteJobOutcome::Cancelled)
            }
            Action::DeleteFinished => {
                // Clean up any lingering token after removing the job.
                let mut tokens = self.cancel_tokens.write().await;
                tokens.remove(job_id);
                Some(DeleteJobOutcome::Deleted)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Core analysis logic (runs on blocking thread pool)
// ---------------------------------------------------------------------------

/// Bundled parameters for [`run_analysis`] to keep the argument count small.
struct RunAnalysisParams<'a> {
    game: &'a Game,
    depth: u32,
    tt_size_mb: usize,
    has_book: bool,
    has_tablebase: bool,
    book_results: &'a [Option<BookMoveInfo>],
    tablebase_results: &'a [Option<TablebaseInfo>],
    jobs: &'a Arc<RwLock<HashMap<String, AnalysisJob>>>,
    job_id: &'a str,
    cancel_token: &'a Arc<AtomicBool>,
}

/// Runs the analysis for a game snapshot.
async fn run_analysis(params: RunAnalysisParams<'_>) -> Result<AnalysisResult, String> {
    let RunAnalysisParams {
        game,
        depth,
        tt_size_mb,
        has_book,
        has_tablebase,
        book_results,
        tablebase_results,
        jobs,
        job_id,
        cancel_token,
    } = params;
    let mut engine = SearchEngine::new(tt_size_mb);
    engine.set_abort_token(Arc::clone(cancel_token));
    let mut annotations = Vec::new();
    let total_moves = game.move_history.len();

    // We need to replay the game and analyze each position
    let mut replay = Game::new();
    let mut still_in_book = true;

    for (idx, record) in game.move_history.iter().enumerate() {
        // Check cancellation flag before expensive work
        if cancel_token.load(Ordering::Relaxed) {
            return Err("Analysis cancelled".to_string());
        }
        // Create search position BEFORE the move
        let pos = SearchPosition::new(
            replay.board.clone(),
            replay.turn,
            replay.castling,
            replay.en_passant,
            replay.halfmove_clock,
        );

        // Find the legal move that matches the record
        let legal = replay.legal_moves();
        let played = legal
            .iter()
            .find(|m| {
                m.from.to_algebraic() == record.move_json.from
                    && m.to.to_algebraic() == record.move_json.to
                    && m.promotion
                        == record
                            .move_json
                            .promotion
                            .as_ref()
                            .and_then(|p| match p.as_str() {
                                "Q" => Some(PieceKind::Queen),
                                "R" => Some(PieceKind::Rook),
                                "B" => Some(PieceKind::Bishop),
                                "N" => Some(PieceKind::Knight),
                                _ => None,
                            })
            })
            .cloned()
            .ok_or_else(|| format!("Could not find legal move for record at index {}", idx))?;

        // Check opening book
        let book_info = book_results.get(idx).cloned().flatten();
        let is_book_move = book_info
            .as_ref()
            .is_some_and(|b| b.is_book_move && still_in_book);

        if !is_book_move {
            still_in_book = false;
        }

        // Check tablebase
        let tb_info = tablebase_results.get(idx).cloned().flatten();
        let is_tablebase = tb_info.as_ref().is_some_and(|t| t.is_tablebase_position);

        // Determine annotation
        let annotation = if is_book_move {
            // Book move — annotate as Book quality
            MoveAnnotation {
                move_number: record.move_number,
                side: record.side,
                played_move: record.move_json.clone(),
                best_move: record.move_json.clone(),
                played_eval: 0,
                best_eval: 0,
                centipawn_loss: 0,
                quality: MoveQuality::Book,
                is_book_move: true,
                is_tablebase_position: false,
                book_info,
                tablebase_info: None,
                search_depth: 0,
                principal_variation: Vec::new(),
            }
        } else if is_tablebase {
            // Tablebase position — evaluate using tablebase results.
            // `pre_probe_tablebase` probes AFTER applying the move, so `tb.wdl`
            // is from the *opponent-to-move* perspective. Invert the mapping so
            // quality/cp_loss reflect the player who played this move.
            let tb = tb_info.clone().unwrap();
            let (quality, cp_loss) = match tb.wdl {
                // Opponent wins → we played a losing move
                Some(WDL::Win) => (MoveQuality::Blunder, 200),
                // Opponent nearly wins (50-move draw) → effectively a draw for us
                Some(WDL::CursedWin) => (MoveQuality::Good, 0),
                Some(WDL::Draw) => (MoveQuality::Good, 0),
                // Opponent nearly loses (50-move draw) → effectively a draw for us
                Some(WDL::BlessedLoss) => (MoveQuality::Excellent, 0),
                // Opponent loses → we played the best/winning move
                Some(WDL::Loss) => (MoveQuality::Best, 0),
                None => (MoveQuality::Good, 0),
            };

            MoveAnnotation {
                move_number: record.move_number,
                side: record.side,
                played_move: record.move_json.clone(),
                best_move: record.move_json.clone(),
                played_eval: 0,
                best_eval: 0,
                centipawn_loss: cp_loss,
                quality,
                is_book_move: false,
                is_tablebase_position: true,
                book_info: None,
                tablebase_info: Some(tb),
                search_depth: 0,
                principal_variation: Vec::new(),
            }
        } else {
            // Deep search — clamp depth to i32::MAX before casting to avoid wrap-around
            let depth_i32 = depth.min(i32::MAX as u32) as i32;
            let search_result = engine.search(&pos, depth_i32);

            let best_move = search_result.best_move.unwrap_or(played);

            // Evaluate the played move at the same depth as the best-move search
            // so centipawn loss and quality thresholds are depth-consistent.
            let played_pos = pos.make_move(&played);
            let played_eval_result = engine.search(&played_pos, depth_i32);
            let played_score = -played_eval_result.score; // Negate because it's from the other side

            let best_score = search_result.score;

            // Centipawn loss = best score - played score
            let cp_loss = (best_score - played_score).max(0);

            // Check if played move IS the best move
            let is_best = played.from == best_move.from
                && played.to == best_move.to
                && played.promotion == best_move.promotion;

            let quality = if is_best {
                MoveQuality::Best
            } else {
                MoveQuality::from_cp_loss(cp_loss)
            };

            let pv: Vec<String> = search_result.pv.iter().map(|m| m.to_string()).collect();

            MoveAnnotation {
                move_number: record.move_number,
                side: record.side,
                played_move: record.move_json.clone(),
                best_move: best_move.to_json(),
                played_eval: played_score,
                best_eval: best_score,
                centipawn_loss: cp_loss,
                quality,
                is_book_move: false,
                is_tablebase_position: false,
                book_info: None,
                tablebase_info: None,
                search_depth: search_result.depth as u32,
                principal_variation: pv,
            }
        };

        annotations.push(annotation);

        // Replay the move to advance the position
        if replay.make_move(&record.move_json).is_err() {
            return Err(format!(
                "Failed to replay move at index {idx}: {} to {}",
                record.move_json.from, record.move_json.to
            ));
        }

        // Update progress (skip lock if already cancelled)
        if cancel_token.load(Ordering::Relaxed) {
            return Err("Analysis cancelled".to_string());
        }
        {
            let mut jobs_lock = jobs.write().await;
            if let Some(job) = jobs_lock.get_mut(job_id) {
                job.status = AnalysisStatus::InProgress {
                    moves_analyzed: idx + 1,
                    total_moves,
                };
            }
        }
    }

    // Compute summary
    let summary = compute_summary(&annotations);

    Ok(AnalysisResult {
        annotations,
        summary,
        // Report the effective depth: clamp to i32::MAX (for cast safety) and to
        // MAX_DEPTH (as enforced by SearchEngine::search) so API consumers
        // see the depth that was actually used, not a potentially unclamped request.
        depth: depth.min(i32::MAX as u32).min(MAX_DEPTH as u32),
        book_available: has_book,
        tablebase_available: has_tablebase,
    })
}

/// Computes summary statistics from move annotations.
fn compute_summary(annotations: &[MoveAnnotation]) -> AnalysisSummary {
    let mut best = 0usize;
    let mut excellent = 0usize;
    let mut good = 0usize;
    let mut inaccuracies = 0usize;
    let mut mistakes = 0usize;
    let mut blunders = 0usize;
    let mut book = 0usize;

    let mut white_cp_loss = 0i64;
    let mut black_cp_loss = 0i64;
    let mut white_moves = 0usize;
    let mut black_moves = 0usize;

    for ann in annotations {
        match ann.quality {
            MoveQuality::Best => best += 1,
            MoveQuality::Excellent => excellent += 1,
            MoveQuality::Good => good += 1,
            MoveQuality::Inaccuracy => inaccuracies += 1,
            MoveQuality::Mistake => mistakes += 1,
            MoveQuality::Blunder => blunders += 1,
            MoveQuality::Book => book += 1,
        }

        if !ann.is_book_move {
            match ann.side {
                Color::White => {
                    white_cp_loss += ann.centipawn_loss as i64;
                    white_moves += 1;
                }
                Color::Black => {
                    black_cp_loss += ann.centipawn_loss as i64;
                    black_moves += 1;
                }
            }
        }
    }

    let total_non_book = white_moves + black_moves;
    let total_cp_loss = white_cp_loss + black_cp_loss;

    let average_centipawn_loss = if total_non_book > 0 {
        total_cp_loss as f64 / total_non_book as f64
    } else {
        0.0
    };

    let white_avg = if white_moves > 0 {
        white_cp_loss as f64 / white_moves as f64
    } else {
        0.0
    };

    let black_avg = if black_moves > 0 {
        black_cp_loss as f64 / black_moves as f64
    } else {
        0.0
    };

    // Accuracy formula: 100 * 2^(-avg_cp_loss / 100)
    // This maps 0 cp loss → 100%, 100 cp loss → 50%, etc.
    let white_accuracy = 100.0 * (2.0f64).powf(-white_avg / 100.0);
    let black_accuracy = 100.0 * (2.0f64).powf(-black_avg / 100.0);

    AnalysisSummary {
        total_moves: annotations.len(),
        best_moves: best,
        excellent_moves: excellent,
        good_moves: good,
        inaccuracies,
        mistakes,
        blunders,
        book_moves: book,
        average_centipawn_loss,
        white_accuracy: white_accuracy.min(100.0),
        black_accuracy: black_accuracy.min(100.0),
        white_avg_cp_loss: white_avg,
        black_avg_cp_loss: black_avg,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_quality_classification() {
        assert_eq!(MoveQuality::from_cp_loss(0), MoveQuality::Best);
        assert_eq!(MoveQuality::from_cp_loss(5), MoveQuality::Excellent);
        assert_eq!(MoveQuality::from_cp_loss(15), MoveQuality::Good);
        assert_eq!(MoveQuality::from_cp_loss(35), MoveQuality::Inaccuracy);
        assert_eq!(MoveQuality::from_cp_loss(75), MoveQuality::Mistake);
        assert_eq!(MoveQuality::from_cp_loss(150), MoveQuality::Blunder);
    }

    #[test]
    fn test_summary_empty() {
        let summary = compute_summary(&[]);
        assert_eq!(summary.total_moves, 0);
        assert_eq!(summary.average_centipawn_loss, 0.0);
    }

    #[test]
    fn test_analysis_config_default() {
        let config = AnalysisConfig::default();
        assert_eq!(config.min_depth, 30);
        assert_eq!(config.tt_size_mb, 64);
        assert_eq!(config.max_jobs_retained, 256);
        assert_eq!(config.max_concurrent_jobs, 4);
        assert_eq!(config.completed_job_ttl_secs, Some(3600));
    }

    // Helper: create a manager with default config (no book / tablebase).
    fn make_manager() -> AnalysisManager {
        AnalysisManager::new(AnalysisConfig::default())
    }

    fn make_manager_with_config(config: AnalysisConfig) -> AnalysisManager {
        AnalysisManager::new(config)
    }

    /// Build a game with a realistic move sequence so `analyze_game` has
    /// non-trivial work to do and the job stays active long enough for
    /// `delete_job` to race against it reliably.
    fn make_game_with_moves() -> Game {
        use crate::types::MoveJson;
        let mut game = Game::new();
        let moves = [
            ("e2", "e4"),
            ("e7", "e5"),
            ("g1", "f3"),
            ("b8", "c6"),
            ("f1", "c4"),
            ("g8", "f6"),
            ("d2", "d3"),
            ("f8", "c5"),
            ("c2", "c3"),
            ("d7", "d6"),
            ("b2", "b4"),
            ("c5", "b6"),
            ("a2", "a4"),
            ("a7", "a6"),
            ("b1", "d2"),
            ("e8", "g8"),
        ];
        for (from, to) in moves {
            game.make_move(&MoveJson {
                from: from.to_string(),
                to: to.to_string(),
                promotion: None,
            })
            .expect("test setup move sequence must remain legal");
        }
        game
    }

    #[tokio::test]
    async fn test_delete_job_not_found_returns_none() {
        let mgr = make_manager();
        assert_eq!(mgr.delete_job("nonexistent-job-id").await, None);
    }

    #[tokio::test]
    async fn test_delete_queued_job_returns_cancelled() {
        let mgr = make_manager();
        let game = make_game_with_moves();
        let job_id = mgr
            .analyze_game(&game, None)
            .await
            .expect("submission should succeed");

        // The job should be Queued or InProgress; delete it immediately.
        let outcome = mgr.delete_job(&job_id).await;
        assert_eq!(outcome, Some(DeleteJobOutcome::Cancelled));

        // The job must still exist in the store with Cancelled status.
        let jobs = mgr.list_jobs().await;
        let job = jobs
            .iter()
            .find(|j| j.id == job_id)
            .expect("job must still be in store");
        assert!(matches!(job.status, AnalysisStatus::Cancelled));
    }

    #[tokio::test]
    async fn test_delete_cancelled_job_returns_deleted() {
        let mgr = make_manager();
        let game = make_game_with_moves();
        let job_id = mgr
            .analyze_game(&game, None)
            .await
            .expect("submission should succeed");

        // First call: cancel an active job.
        let first = mgr.delete_job(&job_id).await;
        assert_eq!(first, Some(DeleteJobOutcome::Cancelled));

        // Second call: the job is now Cancelled → should be removed entirely.
        let second = mgr.delete_job(&job_id).await;
        assert_eq!(second, Some(DeleteJobOutcome::Deleted));

        // Job must be gone from the store.
        let jobs = mgr.list_jobs().await;
        assert!(jobs.iter().all(|j| j.id != job_id));
    }

    #[tokio::test]
    async fn test_submit_rejected_when_concurrent_limit_exceeded() {
        let cfg = AnalysisConfig {
            max_concurrent_jobs: 1,
            ..AnalysisConfig::default()
        };
        let mgr = make_manager_with_config(cfg);

        {
            let mut jobs = mgr.jobs.write().await;
            jobs.insert(
                "active-job".to_string(),
                AnalysisJob {
                    id: "active-job".to_string(),
                    game_id: None,
                    status: AnalysisStatus::Queued,
                    result: None,
                    created_at: storage::unix_timestamp(),
                    completed_at: None,
                },
            );
        }

        let game = make_game_with_moves();
        let err = mgr
            .analyze_game(&game, None)
            .await
            .expect_err("submission should be rejected");

        assert!(matches!(
            err,
            AnalysisSubmitError::ConcurrentLimitExceeded {
                max_concurrent_jobs: 1,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn test_submit_rejected_when_store_limit_exceeded_without_evictable_jobs() {
        let cfg = AnalysisConfig {
            max_jobs_retained: 1,
            completed_job_ttl_secs: None,
            ..AnalysisConfig::default()
        };
        let mgr = make_manager_with_config(cfg);

        {
            let mut jobs = mgr.jobs.write().await;
            jobs.insert(
                "active-job".to_string(),
                AnalysisJob {
                    id: "active-job".to_string(),
                    game_id: None,
                    status: AnalysisStatus::InProgress {
                        moves_analyzed: 0,
                        total_moves: 1,
                    },
                    result: None,
                    created_at: storage::unix_timestamp(),
                    completed_at: None,
                },
            );
        }

        let game = make_game_with_moves();
        let err = mgr
            .analyze_game(&game, None)
            .await
            .expect_err("submission should be rejected");

        assert!(matches!(
            err,
            AnalysisSubmitError::JobStoreLimitExceeded {
                max_jobs_retained: 1,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn test_list_jobs_evicts_expired_finished_jobs_by_ttl() {
        let cfg = AnalysisConfig {
            completed_job_ttl_secs: Some(1),
            ..AnalysisConfig::default()
        };
        let mgr = make_manager_with_config(cfg);
        let now = storage::unix_timestamp();

        {
            let mut jobs = mgr.jobs.write().await;
            jobs.insert(
                "old-finished".to_string(),
                AnalysisJob {
                    id: "old-finished".to_string(),
                    game_id: None,
                    status: AnalysisStatus::Completed,
                    result: None,
                    created_at: now.saturating_sub(10),
                    completed_at: Some(now.saturating_sub(10)),
                },
            );
            jobs.insert(
                "fresh-finished".to_string(),
                AnalysisJob {
                    id: "fresh-finished".to_string(),
                    game_id: None,
                    status: AnalysisStatus::Completed,
                    result: None,
                    created_at: now,
                    completed_at: Some(now),
                },
            );
        }

        let jobs = mgr.list_jobs().await;
        assert!(jobs.iter().all(|j| j.id != "old-finished"));
        assert!(jobs.iter().any(|j| j.id == "fresh-finished"));
    }

    #[tokio::test]
    async fn test_tablebase_available_false_when_directory_has_no_syzygy_files() {
        let unique = format!(
            "checkai-empty-tb-{}-{}",
            std::process::id(),
            storage::unix_timestamp()
        );
        let tb_dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&tb_dir).expect("temp tablebase dir should be created");

        let cfg = AnalysisConfig {
            tablebase_path: Some(tb_dir.clone()),
            ..AnalysisConfig::default()
        };
        let mgr = make_manager_with_config(cfg);

        // Empty directory still yields a loaded tablebase object, but it should
        // report unavailable because no .rtbw/.rtbz files were found.
        let has_tablebase = mgr.tablebase.as_ref().is_some_and(|tb| tb.is_available());
        assert!(!has_tablebase);

        // Run analysis directly (without background job machinery) and verify
        // that API-facing availability flag mirrors actual file availability.
        let game = Game::new();

        let jobs = Arc::new(RwLock::new(std::collections::HashMap::new()));
        let cancel_token = Arc::new(AtomicBool::new(false));
        let analysis = run_analysis(RunAnalysisParams {
            game: &game,
            depth: 30,
            tt_size_mb: 64,
            has_book: false,
            has_tablebase,
            book_results: &[],
            tablebase_results: &[],
            jobs: &jobs,
            job_id: "test-job-id",
            cancel_token: &cancel_token,
        })
        .await
        .expect("analysis should complete successfully");

        assert!(!analysis.tablebase_available);

        std::fs::remove_dir_all(&tb_dir).expect("temp tablebase dir should be removed");
    }
}
