//! REST API for the CheckAI chess server.
//!
//! This module provides a full REST API that allows AI agents to:
//! - Create and manage chess games
//! - Query game state (board, legal moves, check status)
//! - Submit moves and special actions (draw, resign)
//! - View game history
//!
//! The API is documented with OpenAPI/Swagger via `utoipa`.
//! Swagger UI is available at `/swagger-ui/`.
//!
//! All endpoints accept and return JSON following the protocol
//! defined in AGENT.md.

use actix::Addr;
use actix_web::{HttpResponse, Responder, web};
use std::sync::Mutex;
use utoipa::OpenApi;

use crate::game::*;
use crate::movegen;
use crate::storage::{ArchiveListResponse, ArchiveSummary, ReplayResponse, StorageStats};
use crate::types::*;
use crate::ws::GameBroadcaster;

/// Shared application state containing the game manager.
///
/// This struct is wrapped in `web::Data` (which uses `Arc` internally)
/// and shared across all HTTP and WebSocket handlers.
pub struct AppState {
    /// The central game manager (protected by a Mutex for thread safety).
    pub game_manager: Mutex<GameManager>,
}

// ---------------------------------------------------------------------------
// OpenAPI definition
// ---------------------------------------------------------------------------

/// OpenAPI documentation for the CheckAI chess API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "CheckAI — Chess API for AI Agents",
        version = "0.1.0",
        description = "A REST API that allows AI agents to play chess against each other. \
            Follows FIDE 2023 Laws of Chess. Agents communicate using JSON \
            game states and move objects as defined in the AGENT.md protocol.",
        license(name = "MIT")
    ),
    paths(
        create_game,
        list_games,
        get_game,
        delete_game,
        submit_move,
        submit_action,
        get_legal_moves,
        get_board_ascii,
        list_archived_games,
        get_archived_game,
        replay_archived_game,
        get_storage_stats,
    ),
    components(schemas(
        CreateGameResponse,
        GameInfoResponse,
        GameListResponse,
        GameSummary,
        MoveResponse,
        LegalMovesResponse,
        ErrorResponse,
        SubmitMoveRequest,
        SubmitActionRequest,
        GameStateJson,
        MoveJson,
        MoveRecord,
        Color,
        CastlingRights,
        SideCastlingRights,
        GameResult,
        GameEndReason,
        ActionJson,
        AgentResponse,
        ArchiveListResponse,
        ArchiveSummary,
        ReplayResponse,
        StorageStats,
    )),
    tags(
        (name = "games", description = "Game management endpoints"),
        (name = "moves", description = "Move submission and legal move queries"),
        (name = "display", description = "Board display and visualization"),
        (name = "archive", description = "Game archive and replay for analysis"),
    )
)]
pub struct ApiDoc;

// ---------------------------------------------------------------------------
// API Handlers
// ---------------------------------------------------------------------------

/// Create a new chess game.
///
/// Initializes a new game with the standard starting position.
/// Returns a unique game ID that must be used in all subsequent requests.
#[utoipa::path(
    post,
    path = "/api/games",
    tag = "games",
    responses(
        (status = 201, description = "Game created successfully", body = CreateGameResponse),
    )
)]
pub async fn create_game(
    data: web::Data<AppState>,
    broadcaster: web::Data<Addr<GameBroadcaster>>,
) -> impl Responder {
    let mut manager = data.game_manager.lock().unwrap();
    let game_id = manager.create_game();

    log::info!("Created new game: {}", game_id);

    // Broadcast a "game_created" event to all WebSocket subscribers
    crate::ws::broadcast_game_event(
        &broadcaster,
        game_id,
        "game_created",
        &serde_json::json!({ "game_id": game_id.to_string() }),
    );

    HttpResponse::Created().json(CreateGameResponse {
        game_id: game_id.to_string(),
        message: t!("api.game_created").to_string(),
    })
}

/// List all active games.
///
/// Returns a summary of all games currently managed by the server,
/// including their status, current turn, and move number.
#[utoipa::path(
    get,
    path = "/api/games",
    tag = "games",
    responses(
        (status = 200, description = "List of games", body = GameListResponse),
    )
)]
pub async fn list_games(data: web::Data<AppState>) -> impl Responder {
    let manager = data.game_manager.lock().unwrap();
    let summaries: Vec<GameSummary> = manager
        .games
        .values()
        .map(|g| GameSummary {
            game_id: g.id.to_string(),
            turn: g.turn,
            fullmove_number: g.fullmove_number,
            is_over: g.is_over(),
            result: g.result.clone(),
        })
        .collect();

    let total = summaries.len();
    HttpResponse::Ok().json(GameListResponse {
        games: summaries,
        total,
    })
}

/// Get the full state of a game.
///
/// Returns the complete game state including the board position (in the
/// JSON format defined by AGENT.md), castling rights, en passant square,
/// move counters, position history, and game result if the game has ended.
/// This is the same state that would be sent to an AI agent.
#[utoipa::path(
    get,
    path = "/api/games/{game_id}",
    tag = "games",
    params(
        ("game_id" = String, Path, description = "Unique game identifier (UUID)")
    ),
    responses(
        (status = 200, description = "Game state retrieved", body = GameInfoResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
    )
)]
pub async fn get_game(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let game_id_str = path.into_inner();
    let game_id = match uuid::Uuid::parse_str(&game_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: t!("api.invalid_game_id", id = &game_id_str).to_string(),
            });
        }
    };

    let manager = data.game_manager.lock().unwrap();
    match manager.get_game(&game_id) {
        Some(game) => {
            let is_check = movegen::is_in_check(&game.board, game.turn);
            let legal_moves = game.legal_moves();

            HttpResponse::Ok().json(GameInfoResponse {
                game_id: game.id.to_string(),
                state: game.to_game_state_json(),
                is_over: game.is_over(),
                result: game.result.clone(),
                end_reason: game.end_reason.clone(),
                is_check,
                legal_move_count: legal_moves.len(),
                move_history: game.move_history.clone(),
            })
        }
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: t!("api.game_not_found", id = &game_id.to_string()).to_string(),
        }),
    }
}

/// Delete a game.
///
/// Permanently removes a game from the server. This cannot be undone.
#[utoipa::path(
    delete,
    path = "/api/games/{game_id}",
    tag = "games",
    params(
        ("game_id" = String, Path, description = "Unique game identifier (UUID)")
    ),
    responses(
        (status = 200, description = "Game deleted"),
        (status = 404, description = "Game not found", body = ErrorResponse),
    )
)]
pub async fn delete_game(
    path: web::Path<String>,
    data: web::Data<AppState>,
    broadcaster: web::Data<Addr<GameBroadcaster>>,
) -> impl Responder {
    let game_id_str = path.into_inner();
    let game_id = match uuid::Uuid::parse_str(&game_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: t!("api.invalid_game_id", id = &game_id_str).to_string(),
            });
        }
    };

    let mut manager = data.game_manager.lock().unwrap();
    if manager.delete_game(&game_id) {
        log::info!("Deleted game: {}", game_id);

        // Broadcast a "game_deleted" event to all WebSocket subscribers
        crate::ws::broadcast_game_event(
            &broadcaster,
            game_id,
            "game_deleted",
            &serde_json::json!({ "game_id": game_id.to_string() }),
        );

        HttpResponse::Ok().json(serde_json::json!({
            "message": t!("api.game_deleted", id = &game_id.to_string()).to_string()
        }))
    } else {
        HttpResponse::NotFound().json(ErrorResponse {
            error: t!("api.game_not_found", id = &game_id.to_string()).to_string(),
        })
    }
}

/// Submit a move for the current side.
///
/// The move must be legal according to FIDE 2023 rules. The request body
/// follows the AGENT.md move format: `from`, `to`, and optional `promotion`.
///
/// For castling, encode as a king move (e.g. e1→g1 for White kingside).
/// For en passant, encode as a normal pawn capture to the en passant square.
/// For promotion, include the `promotion` field ("Q", "R", "B", or "N").
#[utoipa::path(
    post,
    path = "/api/games/{game_id}/move",
    tag = "moves",
    params(
        ("game_id" = String, Path, description = "Unique game identifier (UUID)")
    ),
    request_body = SubmitMoveRequest,
    responses(
        (status = 200, description = "Move accepted", body = MoveResponse),
        (status = 400, description = "Illegal move or invalid input", body = ErrorResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
    )
)]
pub async fn submit_move(
    path: web::Path<String>,
    body: web::Json<SubmitMoveRequest>,
    data: web::Data<AppState>,
    broadcaster: web::Data<Addr<GameBroadcaster>>,
) -> impl Responder {
    let game_id_str = path.into_inner();
    let game_id = match uuid::Uuid::parse_str(&game_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: t!("api.invalid_game_id", id = &game_id_str).to_string(),
            });
        }
    };

    let mut manager = data.game_manager.lock().unwrap();

    // Scope the mutable game borrow so we can call persist_game afterwards
    let result = {
        let game = match manager.get_game_mut(&game_id) {
            Some(g) => g,
            None => {
                return HttpResponse::NotFound().json(ErrorResponse {
                    error: t!("api.game_not_found", id = &game_id.to_string()).to_string(),
                });
            }
        };

        let move_json = MoveJson {
            from: body.from.clone(),
            to: body.to.clone(),
            promotion: body.promotion.clone(),
        };

        match game.make_move(&move_json) {
            Ok(()) => {
                let is_check = movegen::is_in_check(&game.board, game.turn);
                let message = if game.is_over() {
                    t!(
                        "api.game_over_msg",
                        result = game.result.as_ref().unwrap().to_string(),
                        reason = game.end_reason.as_ref().unwrap().to_string()
                    )
                    .to_string()
                } else if is_check {
                    t!("api.to_move_check", color = game.turn.to_string()).to_string()
                } else {
                    t!("api.to_move", color = game.turn.to_string()).to_string()
                };

                log::info!(
                    "Game {}: Move {}{} accepted. {}",
                    game_id,
                    body.from,
                    body.to,
                    message
                );

                Ok(MoveResponse {
                    success: true,
                    message,
                    state: game.to_game_state_json(),
                    is_over: game.is_over(),
                    result: game.result.clone(),
                    end_reason: game.end_reason.clone(),
                    is_check,
                })
            }
            Err(err) => {
                log::warn!(
                    "Game {}: Illegal move {}{}: {}",
                    game_id,
                    body.from,
                    body.to,
                    err
                );
                Err(err)
            }
        }
    };

    match result {
        Ok(response) => {
            // Persist game state (archive if completed, save if active)
            manager.persist_game(&game_id);

            // Broadcast the game update to all WebSocket subscribers
            crate::ws::broadcast_game_event(
                &broadcaster,
                game_id,
                "game_updated",
                &serde_json::json!({
                    "state": response.state,
                    "is_over": response.is_over,
                    "result": response.result,
                    "end_reason": response.end_reason,
                    "is_check": response.is_check,
                    "message": response.message,
                }),
            );

            HttpResponse::Ok().json(response)
        }
        Err(err) => HttpResponse::BadRequest().json(ErrorResponse { error: err }),
    }
}

/// Submit a special action (draw claim, draw offer, resignation).
///
/// Supported actions:
/// - `resign`: The current side resigns (opponent wins).
/// - `offer_draw`: Offer a draw to the opponent.
/// - `accept_draw`: Accept a pending draw offer.
/// - `claim_draw`: Claim a draw (requires `reason`):
///   - `"threefold_repetition"`: Position occurred 3+ times.
///   - `"fifty_move_rule"`: 50+ moves without pawn move or capture.
#[utoipa::path(
    post,
    path = "/api/games/{game_id}/action",
    tag = "moves",
    params(
        ("game_id" = String, Path, description = "Unique game identifier (UUID)")
    ),
    request_body = SubmitActionRequest,
    responses(
        (status = 200, description = "Action accepted", body = MoveResponse),
        (status = 400, description = "Invalid action", body = ErrorResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
    )
)]
pub async fn submit_action(
    path: web::Path<String>,
    body: web::Json<SubmitActionRequest>,
    data: web::Data<AppState>,
    broadcaster: web::Data<Addr<GameBroadcaster>>,
) -> impl Responder {
    let game_id_str = path.into_inner();
    let game_id = match uuid::Uuid::parse_str(&game_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: t!("api.invalid_game_id", id = &game_id_str).to_string(),
            });
        }
    };

    let mut manager = data.game_manager.lock().unwrap();

    // Scope the mutable game borrow so we can call persist_game afterwards
    let result = {
        let game = match manager.get_game_mut(&game_id) {
            Some(g) => g,
            None => {
                return HttpResponse::NotFound().json(ErrorResponse {
                    error: t!("api.game_not_found", id = &game_id.to_string()).to_string(),
                });
            }
        };

        let action = ActionJson {
            action: body.action.clone(),
            reason: body.reason.clone(),
        };

        match game.process_action(&action) {
            Ok(()) => {
                let is_check = movegen::is_in_check(&game.board, game.turn);
                let message = if game.is_over() {
                    t!(
                        "api.game_over_msg",
                        result = game.result.as_ref().unwrap().to_string(),
                        reason = game.end_reason.as_ref().unwrap().to_string()
                    )
                    .to_string()
                } else {
                    t!("api.action_processed", action = &body.action).to_string()
                };

                log::info!(
                    "Game {}: Action '{}' accepted. {}",
                    game_id,
                    body.action,
                    message
                );

                Ok(MoveResponse {
                    success: true,
                    message,
                    state: game.to_game_state_json(),
                    is_over: game.is_over(),
                    result: game.result.clone(),
                    end_reason: game.end_reason.clone(),
                    is_check,
                })
            }
            Err(err) => {
                log::warn!(
                    "Game {}: Action '{}' rejected: {}",
                    game_id,
                    body.action,
                    err
                );
                Err(err)
            }
        }
    };

    match result {
        Ok(response) => {
            manager.persist_game(&game_id);

            // Broadcast the game update to all WebSocket subscribers
            crate::ws::broadcast_game_event(
                &broadcaster,
                game_id,
                "game_updated",
                &serde_json::json!({
                    "state": response.state,
                    "is_over": response.is_over,
                    "result": response.result,
                    "end_reason": response.end_reason,
                    "is_check": response.is_check,
                    "message": response.message,
                }),
            );

            HttpResponse::Ok().json(response)
        }
        Err(err) => HttpResponse::BadRequest().json(ErrorResponse { error: err }),
    }
}

/// Get all legal moves for the current position.
///
/// Returns a list of all legal moves available to the side to move,
/// in the JSON move format defined by AGENT.md. Useful for agents
/// that want to enumerate their options before choosing.
#[utoipa::path(
    get,
    path = "/api/games/{game_id}/moves",
    tag = "moves",
    params(
        ("game_id" = String, Path, description = "Unique game identifier (UUID)")
    ),
    responses(
        (status = 200, description = "Legal moves retrieved", body = LegalMovesResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
    )
)]
pub async fn get_legal_moves(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let game_id_str = path.into_inner();
    let game_id = match uuid::Uuid::parse_str(&game_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: t!("api.invalid_game_id", id = &game_id_str).to_string(),
            });
        }
    };

    let manager = data.game_manager.lock().unwrap();
    match manager.get_game(&game_id) {
        Some(game) => {
            let legal_moves = game.legal_moves();
            let move_jsons: Vec<MoveJson> = legal_moves.iter().map(|m| m.to_json()).collect();
            let count = move_jsons.len();

            HttpResponse::Ok().json(LegalMovesResponse {
                turn: game.turn,
                moves: move_jsons,
                count,
            })
        }
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: t!("api.game_not_found", id = &game_id.to_string()).to_string(),
        }),
    }
}

/// Get an ASCII representation of the current board.
///
/// Returns a text-based visualization of the board position,
/// useful for debugging and terminal display.
#[utoipa::path(
    get,
    path = "/api/games/{game_id}/board",
    tag = "display",
    params(
        ("game_id" = String, Path, description = "Unique game identifier (UUID)")
    ),
    responses(
        (status = 200, description = "Board ASCII art", content_type = "text/plain"),
        (status = 404, description = "Game not found", body = ErrorResponse),
    )
)]
pub async fn get_board_ascii(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let game_id_str = path.into_inner();
    let game_id = match uuid::Uuid::parse_str(&game_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest()
                .content_type("text/plain")
                .body(t!("api.invalid_game_id", id = &game_id_str).to_string());
        }
    };

    let manager = data.game_manager.lock().unwrap();
    match manager.get_game(&game_id) {
        Some(game) => {
            let ascii = board_to_ascii(&game.board, game.turn);
            HttpResponse::Ok().content_type("text/plain").body(ascii)
        }
        None => HttpResponse::NotFound()
            .content_type("text/plain")
            .body(t!("api.game_not_found", id = &game_id.to_string()).to_string()),
    }
}

/// Renders the board as an ASCII art string.
pub fn board_to_ascii(board: &Board, turn: Color) -> String {
    let mut s = String::new();
    s.push_str("  +---+---+---+---+---+---+---+---+\n");
    for rank in (0..8u8).rev() {
        s.push_str(&format!("{} ", rank + 1));
        for file in 0..8u8 {
            let sq = Square::new(file, rank);
            let ch = match board.get(sq) {
                Some(piece) => piece.to_fen_char(),
                None => ' ',
            };
            s.push_str(&format!("| {} ", ch));
        }
        s.push_str("|\n");
        s.push_str("  +---+---+---+---+---+---+---+---+\n");
    }
    s.push_str("    a   b   c   d   e   f   g   h\n");
    s.push_str(&format!(
        "\n  {} {}\n",
        t!("api.board_status", color = turn.to_string()),
        ""
    ));
    s
}

/// Configures all API routes.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route("/games", web::post().to(create_game))
            .route("/games", web::get().to(list_games))
            .route("/games/{game_id}", web::get().to(get_game))
            .route("/games/{game_id}", web::delete().to(delete_game))
            .route("/games/{game_id}/move", web::post().to(submit_move))
            .route("/games/{game_id}/action", web::post().to(submit_action))
            .route("/games/{game_id}/moves", web::get().to(get_legal_moves))
            .route("/games/{game_id}/board", web::get().to(get_board_ascii))
            .route("/archive", web::get().to(list_archived_games))
            .route("/archive/stats", web::get().to(get_storage_stats))
            .route("/archive/{game_id}", web::get().to(get_archived_game))
            .route(
                "/archive/{game_id}/replay",
                web::get().to(replay_archived_game),
            ),
    );
}

// ---------------------------------------------------------------------------
// Archive API Handlers
// ---------------------------------------------------------------------------

/// List all archived (completed) games.
///
/// Returns summaries of all games that have been completed and compressed
/// in the archive, along with storage statistics.
#[utoipa::path(
    get,
    path = "/api/archive",
    tag = "archive",
    responses(
        (status = 200, description = "List of archived games", body = ArchiveListResponse),
    )
)]
pub async fn list_archived_games(data: web::Data<AppState>) -> impl Responder {
    let manager = data.game_manager.lock().unwrap();
    let archived_ids = match manager.storage.list_archived() {
        Ok(ids) => ids,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: t!("api.failed_list_archives", error = &e).to_string(),
            });
        }
    };

    let mut games = Vec::new();
    for id in &archived_ids {
        if let Ok(archive) = manager.storage.load_archive(id) {
            let compressed_bytes = manager.storage.archive_file_size(id).unwrap_or(0);
            games.push(ArchiveSummary {
                game_id: id.to_string(),
                move_count: archive.move_count(),
                result: archive.result.clone(),
                end_reason: archive.end_reason.clone(),
                start_timestamp: archive.start_timestamp,
                end_timestamp: archive.end_timestamp,
                compressed_bytes,
                raw_bytes: archive.raw_size(),
            });
        }
    }

    let total = games.len();
    let stats = manager.storage.stats().unwrap_or(StorageStats {
        active_count: 0,
        archived_count: 0,
        active_bytes: 0,
        archive_bytes: 0,
        total_bytes: 0,
    });

    HttpResponse::Ok().json(ArchiveListResponse {
        games,
        total,
        storage: stats,
    })
}

/// Get details of an archived game.
///
/// Loads a completed game from the compressed archive and returns
/// its full move list and metadata. The game can then be replayed
/// at any position using the replay endpoint.
#[utoipa::path(
    get,
    path = "/api/archive/{game_id}",
    tag = "archive",
    params(
        ("game_id" = String, Path, description = "Unique game identifier (UUID)")
    ),
    responses(
        (status = 200, description = "Archived game details", body = ReplayResponse),
        (status = 404, description = "Game not found in archive", body = ErrorResponse),
    )
)]
pub async fn get_archived_game(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> impl Responder {
    let game_id_str = path.into_inner();
    let game_id = match uuid::Uuid::parse_str(&game_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: t!("api.invalid_game_id", id = &game_id_str).to_string(),
            });
        }
    };

    let manager = data.game_manager.lock().unwrap();
    let (archive, _compressed) = match manager.storage.load_any(&game_id) {
        Ok(result) => result,
        Err(e) => {
            return HttpResponse::NotFound().json(ErrorResponse { error: e });
        }
    };

    // Replay to final position
    match archive.replay_full() {
        Ok(game) => {
            let is_check = movegen::is_in_check(&game.board, game.turn);
            HttpResponse::Ok().json(ReplayResponse {
                game_id: game_id.to_string(),
                at_move: archive.move_count(),
                total_moves: archive.move_count(),
                state: game.to_game_state_json(),
                is_over: game.is_over(),
                result: game.result.clone(),
                is_check,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: t!("api.failed_replay", error = &e).to_string(),
        }),
    }
}

/// Replay an archived game to a specific move number.
///
/// Reconstructs the exact board state at any point in a completed game.
/// This is the primary endpoint for post-game analysis.
///
/// The `move_number` query parameter specifies how many half-moves to
/// replay (0 = starting position, omit = final position).
#[utoipa::path(
    get,
    path = "/api/archive/{game_id}/replay",
    tag = "archive",
    params(
        ("game_id" = String, Path, description = "Unique game identifier (UUID)"),
        ("move_number" = Option<usize>, Query, description = "Half-move number to replay to (0 = start, omit = final)")
    ),
    responses(
        (status = 200, description = "Replayed game state", body = ReplayResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
    )
)]
pub async fn replay_archived_game(
    path: web::Path<String>,
    query: web::Query<ReplayQuery>,
    data: web::Data<AppState>,
) -> impl Responder {
    let game_id_str = path.into_inner();
    let game_id = match uuid::Uuid::parse_str(&game_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: t!("api.invalid_game_id", id = &game_id_str).to_string(),
            });
        }
    };

    let manager = data.game_manager.lock().unwrap();
    let (archive, _compressed) = match manager.storage.load_any(&game_id) {
        Ok(result) => result,
        Err(e) => {
            return HttpResponse::NotFound().json(ErrorResponse { error: e });
        }
    };

    let up_to = query.move_number.unwrap_or(archive.move_count());

    match archive.replay(up_to) {
        Ok(game) => {
            let is_check = movegen::is_in_check(&game.board, game.turn);
            let actual_move = up_to.min(archive.move_count());
            HttpResponse::Ok().json(ReplayResponse {
                game_id: game_id.to_string(),
                at_move: actual_move,
                total_moves: archive.move_count(),
                state: game.to_game_state_json(),
                is_over: game.is_over(),
                result: game.result.clone(),
                is_check,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: t!("api.failed_replay", error = &e).to_string(),
        }),
    }
}

/// Query parameters for the replay endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct ReplayQuery {
    /// Half-move number to replay to.
    pub move_number: Option<usize>,
}

/// Get storage statistics.
///
/// Returns information about disk usage for active and archived games.
#[utoipa::path(
    get,
    path = "/api/archive/stats",
    tag = "archive",
    responses(
        (status = 200, description = "Storage statistics", body = StorageStats),
    )
)]
pub async fn get_storage_stats(data: web::Data<AppState>) -> impl Responder {
    let manager = data.game_manager.lock().unwrap();
    match manager.storage.stats() {
        Ok(stats) => HttpResponse::Ok().json(stats),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: t!("api.failed_stats", error = &e).to_string(),
        }),
    }
}
