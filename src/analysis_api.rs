//! REST API endpoints for game analysis.
//!
//! These endpoints are **architecturally separated** from the player-facing
//! `/api/games/*` endpoints. Analysis results are only accessible through
//! `/api/analysis/*`, enforcing strict data isolation.

use actix_web::{HttpResponse, Responder, web};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::analysis::{AnalysisJobSummary, AnalysisManager, DeleteJobOutcome};
use crate::api::AppState;
use crate::storage::ArchiveLoadError;

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

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Submit a game for deep analysis.
///
/// Creates an asynchronous analysis job that processes every move in the
/// game with a minimum search depth of 30 plies. The game state is
/// snapshot-ed (cloned) at the time of submission; the analysis operates
/// on the snapshot exclusively.
#[utoipa::path(
    post,
    path = "/api/analysis/game/{game_id}",
    tag = "analysis",
    request_body = AnalyzeGameRequest,
    responses(
        (status = 202, description = "Analysis job submitted", body = SubmitAnalysisResponse),
        (status = 400, description = "Invalid game ID or game has no moves", body = AnalysisErrorResponse),
        (status = 404, description = "Game not found", body = AnalysisErrorResponse),
        (status = 500, description = "Archive load or replay failure", body = AnalysisErrorResponse),
    )
)]
pub async fn analyze_game(
    path: web::Path<String>,
    body: web::Json<AnalyzeGameRequest>,
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

    let job_id = analysis.analyze_game(&snapshot, body.depth).await;

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
            .route("/game/{game_id}", web::post().to(analyze_game))
            .route("/jobs", web::get().to(list_analysis_jobs))
            .route("/jobs/{job_id}", web::get().to(get_analysis_job))
            .route("/jobs/{job_id}", web::delete().to(delete_analysis_job)),
    );
}
