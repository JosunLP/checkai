//! REST API endpoints for game analysis.
//!
//! These endpoints are **architecturally separated** from the player-facing
//! `/api/games/*` endpoints. Analysis results are only accessible through
//! `/api/analysis/*`, enforcing strict data isolation.

use actix_web::{HttpResponse, Responder, web};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::analysis::{
    AnalysisJobSummary, AnalysisManager, AnalysisSubmitError, DeleteJobOutcome, PositionRequest,
};
use crate::api::AppState;
use crate::game::Game;
use crate::opening_book::BookMoveInfo;
use crate::search::{MoveSource, score_to_mate_in};
use crate::storage::ArchiveLoadError;
use crate::tablebase::TablebaseInfo;
use crate::types::{Color, MoveJson};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// Request to analyze a game.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AnalyzeGameRequest {
    /// Search depth (minimum 30, default: configured value).
    pub depth: Option<u32>,
}

/// Generic error body.
#[derive(Debug, Serialize, ToSchema)]
pub struct AnalysisErrorResponse {
    pub error: String,
}

/// Response after submitting an analysis job.
#[derive(Debug, Serialize, ToSchema)]
pub struct SubmitAnalysisResponse {
    /// The unique job ID.
    pub job_id: String,
    /// Informational message.
    pub message: String,
}

/// Response with a list of analysis jobs.
#[derive(Debug, Serialize, ToSchema)]
pub struct AnalysisJobListResponse {
    /// All analysis jobs.
    pub jobs: Vec<AnalysisJobSummary>,
    /// Total number of jobs.
    pub count: usize,
}

/// Request for an immediate, single-position analysis.
///
/// Exactly one of `fen` or `game_id` identifies the position; when both are
/// given, `fen` wins.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AnalyzePositionRequest {
    /// Position to analyse, as a 4–6 field FEN string.
    pub fen: Option<String>,
    /// Analyse the current position of this active game instead.
    pub game_id: Option<String>,
    /// Maximum search depth in plies.
    pub depth: Option<u32>,
    /// Time budget in milliseconds (default 1000, capped at 60 000).
    pub movetime_ms: Option<u64>,
    /// Number of principal variations to report (1–16).
    pub multi_pv: Option<usize>,
    /// Lazy SMP search threads (1–64).
    pub threads: Option<usize>,
}

/// One principal variation of a position analysis.
#[derive(Debug, Serialize, ToSchema)]
pub struct PvLineResponse {
    /// 1-based rank of this line (1 = best).
    pub rank: usize,
    /// Score in centipawns from the side to move's perspective.
    pub score_cp: i32,
    /// Score in centipawns from White's perspective.
    pub score_white_cp: i32,
    /// Full moves until mate, if the line is a forced mate.
    pub mate_in: Option<i32>,
    /// The line in long algebraic notation.
    pub moves: Vec<String>,
}

/// Immediate analysis of a single position.
#[derive(Debug, Serialize, ToSchema)]
pub struct PositionAnalysisResponse {
    /// The position that was analysed, as a full FEN string.
    pub fen: String,
    /// Side to move.
    pub turn: Color,
    /// The engine's chosen move.
    pub best_move: Option<MoveJson>,
    /// Score in centipawns from the side to move's perspective.
    pub score_cp: i32,
    /// Score in centipawns from White's perspective (for evaluation bars).
    pub score_white_cp: i32,
    /// Full moves until mate, if a forced mate was found.
    pub mate_in: Option<i32>,
    /// Static evaluation of the position, side-to-move relative.
    pub static_eval_cp: i32,
    /// Iterative-deepening depth reached.
    pub depth: i32,
    /// Greatest ply reached anywhere in the tree.
    pub seldepth: i32,
    /// Nodes searched.
    pub nodes: u64,
    /// Nodes per second.
    pub nps: u64,
    /// Wall-clock time spent, in milliseconds.
    pub time_ms: u64,
    /// Transposition table fill level in per mille.
    pub hashfull: u32,
    /// Where the move came from: `search`, `book` or `tablebase`.
    pub source: String,
    /// All requested principal variations, best first.
    pub lines: Vec<PvLineResponse>,
    /// Opening-book information for the chosen move, when a book is loaded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book: Option<BookMoveInfo>,
    /// Endgame tablebase verdict, when a tablebase is loaded and in range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tablebase: Option<TablebaseInfo>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Submit a game for deep analysis.
///
/// Creates an asynchronous analysis job that processes every move in the
/// game with a minimum search depth of 30 plies. The game state is
/// snapshotted (cloned) at the time of submission; the analysis operates
/// on the snapshot exclusively.
#[utoipa::path(
    post,
    path = "/api/analysis/game/{game_id}",
    tag = "analysis",
    request_body = Option<AnalyzeGameRequest>,
    responses(
        (status = 202, description = "Analysis job submitted", body = SubmitAnalysisResponse),
        (status = 400, description = "Invalid game ID or game has no moves", body = AnalysisErrorResponse),
        (status = 404, description = "Game not found", body = AnalysisErrorResponse),
        (status = 429, description = "Analysis capacity exceeded", body = AnalysisErrorResponse),
        (status = 500, description = "Archive load or replay failure", body = AnalysisErrorResponse),
    )
)]
pub async fn analyze_game(
    path: web::Path<String>,
    body: Option<web::Json<AnalyzeGameRequest>>,
    data: web::Data<AppState>,
    analysis: web::Data<AnalysisManager>,
) -> impl Responder {
    let game_id_str = path.into_inner();
    let game_id = match uuid::Uuid::parse_str(&game_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(AnalysisErrorResponse {
                error: t!("api.invalid_game_id", id = &game_id_str).to_string(),
            });
        }
    };

    // Obtain a read-only snapshot of the game.
    // We minimise the time spent holding the game_manager lock: only
    // clone the active game (cheap) or the storage handle (three PathBufs).
    // Expensive disk IO + zstd decompression happens *after* the lock is
    // released so other requests are not blocked.
    let (active_snapshot, storage_clone) = {
        let manager = data.game_manager.lock().unwrap();
        if let Some(game) = manager.games.get(&game_id) {
            (Some(game.clone()), None)
        } else {
            (None, Some(manager.storage.clone()))
        }
    };

    let game_snapshot = if let Some(snap) = active_snapshot {
        Some(snap)
    } else if let Some(storage) = storage_clone {
        // Disk IO + zstd decompression happens outside the mutex.
        // NotFound → fall through to 404; all other failures → 500.
        match storage.load_archive(&game_id) {
            Ok(archive) => match archive.replay(archive.move_count()) {
                Ok(game) => Some(game),
                Err(e) => {
                    log::error!("Failed to replay archived game {game_id}: {e}");
                    return HttpResponse::InternalServerError().json(AnalysisErrorResponse {
                        error: t!("analysis.archive_replay_failed").to_string(),
                    });
                }
            },
            Err(ArchiveLoadError::NotFound(_)) => None,
            Err(ArchiveLoadError::Other(e)) => {
                log::error!("Failed to load archived game {game_id}: {e}");
                return HttpResponse::InternalServerError().json(AnalysisErrorResponse {
                    error: t!("analysis.archive_load_failed").to_string(),
                });
            }
        }
    } else {
        None
    };

    let Some(snapshot) = game_snapshot else {
        return HttpResponse::NotFound().json(AnalysisErrorResponse {
            error: t!("api.game_not_found", id = &game_id_str).to_string(),
        });
    };

    if snapshot.move_history.is_empty() {
        return HttpResponse::BadRequest().json(AnalysisErrorResponse {
            error: t!("analysis.game_no_moves").to_string(),
        });
    }

    let depth = body.as_ref().and_then(|b| b.depth);
    let job_id = match analysis.analyze_game(&snapshot, depth).await {
        Ok(id) => id,
        Err(AnalysisSubmitError::ConcurrentLimitExceeded {
            active_jobs,
            max_concurrent_jobs,
        }) => {
            return HttpResponse::TooManyRequests().json(AnalysisErrorResponse {
                error: t!(
                    "analysis.job_limit_exceeded",
                    active = active_jobs,
                    max_active = max_concurrent_jobs,
                    stored = analysis.list_jobs().await.len()
                )
                .to_string(),
            });
        }
        Err(AnalysisSubmitError::JobStoreLimitExceeded {
            stored_jobs,
            max_jobs_retained,
        }) => {
            return HttpResponse::TooManyRequests().json(AnalysisErrorResponse {
                error: t!(
                    "analysis.job_store_limit_exceeded",
                    stored = stored_jobs,
                    max_stored = max_jobs_retained
                )
                .to_string(),
            });
        }
    };

    HttpResponse::Accepted().json(SubmitAnalysisResponse {
        job_id,
        message: t!(
            "analysis.job_submitted",
            id = &game_id_str,
            moves = snapshot.move_history.len()
        )
        .to_string(),
    })
}

/// List all analysis jobs.
///
/// Returns brief summaries of all analysis jobs (queued, in-progress,
/// completed, failed, cancelled).
#[utoipa::path(
    get,
    path = "/api/analysis/jobs",
    tag = "analysis",
    responses(
        (status = 200, description = "List of analysis jobs", body = AnalysisJobListResponse),
    )
)]
pub async fn list_analysis_jobs(analysis: web::Data<AnalysisManager>) -> impl Responder {
    let jobs = analysis.list_jobs().await;
    let count = jobs.len();
    HttpResponse::Ok().json(AnalysisJobListResponse { jobs, count })
}

/// Get the status and results of an analysis job.
///
/// Returns partial progress while the job is running, or complete
/// annotations once finished.
#[utoipa::path(
    get,
    path = "/api/analysis/jobs/{job_id}",
    tag = "analysis",
    responses(
        (status = 200, description = "Analysis job details", body = crate::analysis::AnalysisJob),
        (status = 404, description = "Job not found", body = AnalysisErrorResponse),
    )
)]
pub async fn get_analysis_job(
    path: web::Path<String>,
    analysis: web::Data<AnalysisManager>,
) -> impl Responder {
    let job_id = path.into_inner();
    match analysis.get_job(&job_id).await {
        Some(job) => HttpResponse::Ok().json(job),
        None => HttpResponse::NotFound().json(AnalysisErrorResponse {
            error: t!("analysis.job_not_found", id = &job_id).to_string(),
        }),
    }
}

/// Cancel or delete an analysis job.
///
/// If the job is queued or in progress, it will be cancelled.
/// A cancelled job is kept on the first delete call and removed on a
/// subsequent delete call. Completed jobs are deleted immediately.
#[utoipa::path(
    delete,
    path = "/api/analysis/jobs/{job_id}",
    tag = "analysis",
    responses(
        (status = 200, description = "Job cancelled or deleted"),
        (status = 404, description = "Job not found", body = AnalysisErrorResponse),
    )
)]
pub async fn delete_analysis_job(
    path: web::Path<String>,
    analysis: web::Data<AnalysisManager>,
) -> impl Responder {
    let job_id = path.into_inner();
    match analysis.delete_job(&job_id).await {
        Some(DeleteJobOutcome::Cancelled) => HttpResponse::Ok().json(serde_json::json!({
            "message": t!("analysis.job_cancelled", id = &job_id).to_string()
        })),
        Some(DeleteJobOutcome::Deleted) => HttpResponse::Ok().json(serde_json::json!({
            "message": t!("analysis.job_deleted", id = &job_id).to_string()
        })),
        None => HttpResponse::NotFound().json(AnalysisErrorResponse {
            error: t!("analysis.job_not_found", id = &job_id).to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Route configuration
// ---------------------------------------------------------------------------

/// Configures the analysis API routes under `/api/analysis`.
///
/// These routes are completely separate from the player-facing
/// `/api/games` routes, enforcing data isolation.
pub fn configure_analysis_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/analysis")
            .route("/position", web::post().to(analyze_position))
            .route("/game/{game_id}", web::post().to(analyze_game))
            .route("/jobs", web::get().to(list_analysis_jobs))
            .route("/jobs/{job_id}", web::get().to(get_analysis_job))
            .route("/jobs/{job_id}", web::delete().to(delete_analysis_job)),
    );
}

// ---------------------------------------------------------------------------
// Immediate position analysis
// ---------------------------------------------------------------------------

/// Analyse a single position and return the verdict immediately.
///
/// This is the interactive counterpart to the job-based game analysis: it
/// runs one bounded search (default one second) and answers in the same
/// request, so a UI can show a live evaluation bar, the best move, and — with
/// `multi_pv` — the top alternatives. The server's opening book and endgame
/// tablebase are reported alongside the search result when configured.
#[utoipa::path(
    post,
    path = "/api/analysis/position",
    tag = "Analysis",
    request_body = AnalyzePositionRequest,
    responses(
        (status = 200, description = "Analysis complete", body = PositionAnalysisResponse),
        (status = 400, description = "Invalid FEN or missing position", body = AnalysisErrorResponse),
        (status = 404, description = "Game not found", body = AnalysisErrorResponse),
    )
)]
pub async fn analyze_position(
    state: web::Data<AppState>,
    analysis: web::Data<AnalysisManager>,
    body: web::Json<AnalyzePositionRequest>,
) -> impl Responder {
    // Resolve the position: an explicit FEN, or the current state of a game.
    let game = match (&body.fen, &body.game_id) {
        (Some(fen), _) => match Game::from_fen(fen) {
            Ok(game) => game,
            Err(err) => {
                return HttpResponse::BadRequest().json(AnalysisErrorResponse {
                    error: t!("cli.invalid_fen", error = err).to_string(),
                });
            }
        },
        (None, Some(game_id)) => {
            let Ok(uuid) = uuid::Uuid::parse_str(game_id) else {
                return HttpResponse::BadRequest().json(AnalysisErrorResponse {
                    error: t!("api.invalid_game_id", id = game_id).to_string(),
                });
            };
            let manager = match state.game_manager.lock() {
                Ok(manager) => manager,
                Err(_) => {
                    return HttpResponse::InternalServerError().json(AnalysisErrorResponse {
                        error: t!("analysis.state_unavailable").to_string(),
                    });
                }
            };
            match manager.get_game(&uuid) {
                Some(game) => game.clone(),
                None => {
                    return HttpResponse::NotFound().json(AnalysisErrorResponse {
                        error: t!("api.game_not_found", id = game_id).to_string(),
                    });
                }
            }
        }
        (None, None) => {
            return HttpResponse::BadRequest().json(AnalysisErrorResponse {
                error: t!("analysis.position_required").to_string(),
            });
        }
    };

    let request = PositionRequest {
        game: game.clone(),
        depth: body.depth,
        movetime_ms: body.movetime_ms,
        multi_pv: body.multi_pv,
        threads: body.threads,
    };

    // The search is CPU-bound; keep it off the async worker threads.
    let manager = analysis.clone();
    let analysed = match web::block(move || manager.analyze_position(&request)).await {
        Ok(analysed) => analysed,
        Err(err) => {
            return HttpResponse::InternalServerError().json(AnalysisErrorResponse {
                error: err.to_string(),
            });
        }
    };

    let turn = game.turn;
    let white_pov = |score: i32| match turn {
        Color::White => score,
        Color::Black => -score,
    };
    let result = &analysed.result;

    HttpResponse::Ok().json(PositionAnalysisResponse {
        fen: format!(
            "{} {} {}",
            game.board
                .to_position_fen(game.turn, &game.castling, game.en_passant),
            game.halfmove_clock,
            game.fullmove_number
        ),
        turn,
        best_move: result.best_move.as_ref().map(|mv| mv.to_json()),
        score_cp: result.score,
        score_white_cp: white_pov(result.score),
        mate_in: score_to_mate_in(result.score),
        static_eval_cp: analysed.static_eval_cp,
        depth: result.depth,
        seldepth: result.seldepth,
        nodes: result.stats.nodes,
        nps: result.nps(),
        time_ms: result.time_ms,
        hashfull: result.hashfull,
        source: match result.source {
            MoveSource::Search => "search",
            MoveSource::Book => "book",
            MoveSource::Tablebase => "tablebase",
        }
        .to_string(),
        lines: result
            .pv_lines
            .iter()
            .map(|line| PvLineResponse {
                rank: line.rank,
                score_cp: line.score,
                score_white_cp: white_pov(line.score),
                mate_in: line.mate_in,
                moves: line.moves.iter().map(|mv| mv.to_string()).collect(),
            })
            .collect(),
        book: analysed.book.clone(),
        tablebase: result.tablebase.clone(),
    })
}
