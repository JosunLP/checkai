//! WebSocket support for the CheckAI chess server.
//!
//! This module provides full WebSocket connectivity, mirroring every REST
//! endpoint so that clients can interact with the chess server in a fully
//! reactive, event-driven manner.
//!
//! ## Architecture
//!
//! - **`GameBroadcaster`** — A singleton actor that manages per-game subscriber
//!   lists and broadcasts real-time events (moves, state changes, deletions)
//!   to all connected WebSocket sessions subscribed to a given game.
//!
//! - **`WsSession`** — An actor representing a single WebSocket connection.
//!   Receives JSON commands from the client, delegates them to the
//!   `GameManager`, and forwards real-time events from the broadcaster.
//!
//! ## Client → Server Protocol
//!
//! Clients send JSON messages with an `"action"` field:
//!
//! | Action               | Extra Fields                                    |
//! |----------------------|-------------------------------------------------|
//! | `create_game`        | —                                               |
//! | `list_games`         | —                                               |
//! | `get_game`           | `game_id`                                       |
//! | `delete_game`        | `game_id`                                       |
//! | `submit_move`        | `game_id`, `from`, `to`, `promotion?`           |
//! | `submit_action`      | `game_id`, `action_type`, `reason?`             |
//! | `get_legal_moves`    | `game_id`                                       |
//! | `get_board`          | `game_id`                                       |
//! | `subscribe`          | `game_id`                                       |
//! | `unsubscribe`        | `game_id`                                       |
//! | `list_archived`      | —                                               |
//! | `get_archived`       | `game_id`                                       |
//! | `replay_archived`    | `game_id`, `move_number?`                       |
//! | `get_storage_stats`  | —                                               |
//!
//! Every message may optionally include a `"request_id"` string that will
//! be echoed back in the server response for client-side correlation.
//!
//! ## Server → Client Protocol
//!
//! **Responses** (to a client command):
//! ```json
//! {
//!   "type": "response",
//!   "action": "<action>",
//!   "request_id": "<id or null>",
//!   "success": true,
//!   "data": { ... }
//! }
//! ```
//!
//! **Events** (pushed to subscribers):
//! ```json
//! {
//!   "type": "event",
//!   "event": "game_updated" | "game_created" | "game_deleted",
//!   "game_id": "<uuid>",
//!   "data": { ... }
//! }
//! ```

use actix::prelude::*;
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::api::{board_to_ascii, AppState};
use crate::movegen;
use crate::storage::StorageStats;
use crate::types::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// How often the server sends a WebSocket ping frame to keep the
/// connection alive and detect stale clients.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);

/// Maximum time the server waits for a pong response before
/// considering the connection dead.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Broadcaster messages (actor mailbox protocol)
// ---------------------------------------------------------------------------

/// Message sent by a `WsSession` to register itself with the broadcaster.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    /// The address of the connecting session.
    pub addr: Addr<WsSession>,
    /// Unique identifier for the session.
    pub session_id: Uuid,
}

/// Message sent by a `WsSession` to unregister from the broadcaster.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    /// Unique identifier of the disconnecting session.
    pub session_id: Uuid,
}

/// Message sent by a `WsSession` to subscribe to events for a specific game.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Subscribe {
    /// The session requesting subscription.
    pub session_id: Uuid,
    /// The game to subscribe to.
    pub game_id: Uuid,
}

/// Message sent by a `WsSession` to unsubscribe from a specific game.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Unsubscribe {
    /// The session unsubscribing.
    pub session_id: Uuid,
    /// The game to unsubscribe from.
    pub game_id: Uuid,
}

/// A broadcast event pushed to all sessions subscribed to a game.
#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct BroadcastEvent {
    /// The game this event relates to.
    pub game_id: Uuid,
    /// The event type name (e.g. "game_updated", "game_created", "game_deleted").
    pub event: String,
    /// The JSON-serialized event payload.
    pub payload: String,
}

/// Internal message: deliver a text frame to a single `WsSession`.
#[derive(Message)]
#[rtype(result = "()")]
pub struct WsText(pub String);

// ---------------------------------------------------------------------------
// GameBroadcaster — central event hub (actor)
// ---------------------------------------------------------------------------

/// Singleton actor that manages WebSocket subscriptions and broadcasts
/// real-time game events to all interested clients.
///
/// Each game has a set of subscribed session IDs. When a game event
/// occurs (move, action, deletion), the broadcaster looks up all
/// subscribers and forwards the event payload to their `WsSession` actors.
#[derive(Default)]
pub struct GameBroadcaster {
    /// Map of session ID → session actor address (all connected sessions).
    sessions: HashMap<Uuid, Addr<WsSession>>,
    /// Map of game ID → set of subscribed session IDs.
    subscriptions: HashMap<Uuid, HashSet<Uuid>>,
}

impl GameBroadcaster {
    /// Creates a new broadcaster with empty state.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Actor for GameBroadcaster {
    type Context = Context<Self>;
}

/// Handler for new session connections.
impl Handler<Connect> for GameBroadcaster {
    type Result = ();

    fn handle(&mut self, msg: Connect, _ctx: &mut Context<Self>) {
        log::debug!("WS session {} connected to broadcaster", msg.session_id);
        self.sessions.insert(msg.session_id, msg.addr);
    }
}

/// Handler for session disconnections — removes the session from all
/// subscriptions and the session registry.
impl Handler<Disconnect> for GameBroadcaster {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _ctx: &mut Context<Self>) {
        log::debug!("WS session {} disconnected from broadcaster", msg.session_id);
        self.sessions.remove(&msg.session_id);

        // Remove session from every game subscription set
        for subscribers in self.subscriptions.values_mut() {
            subscribers.remove(&msg.session_id);
        }

        // Clean up empty subscription sets
        self.subscriptions.retain(|_, subs| !subs.is_empty());
    }
}

/// Handler for game subscriptions.
impl Handler<Subscribe> for GameBroadcaster {
    type Result = ();

    fn handle(&mut self, msg: Subscribe, _ctx: &mut Context<Self>) {
        log::debug!(
            "WS session {} subscribed to game {}",
            msg.session_id,
            msg.game_id
        );
        self.subscriptions
            .entry(msg.game_id)
            .or_default()
            .insert(msg.session_id);
    }
}

/// Handler for game unsubscriptions.
impl Handler<Unsubscribe> for GameBroadcaster {
    type Result = ();

    fn handle(&mut self, msg: Unsubscribe, _ctx: &mut Context<Self>) {
        log::debug!(
            "WS session {} unsubscribed from game {}",
            msg.session_id,
            msg.game_id
        );
        if let Some(subscribers) = self.subscriptions.get_mut(&msg.game_id) {
            subscribers.remove(&msg.session_id);
            if subscribers.is_empty() {
                self.subscriptions.remove(&msg.game_id);
            }
        }
    }
}

/// Handler for broadcasting game events to all subscribed sessions.
impl Handler<BroadcastEvent> for GameBroadcaster {
    type Result = ();

    fn handle(&mut self, msg: BroadcastEvent, _ctx: &mut Context<Self>) {
        if let Some(subscribers) = self.subscriptions.get(&msg.game_id) {
            let event_json = build_event_json(&msg.event, &msg.game_id, &msg.payload);
            for session_id in subscribers {
                if let Some(addr) = self.sessions.get(session_id) {
                    addr.do_send(WsText(event_json.clone()));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Client → Server message types
// ---------------------------------------------------------------------------

/// A single JSON message received from a WebSocket client.
///
/// Uses `#[serde(default)]` on optional fields so that clients
/// only need to supply the fields relevant to their action.
#[derive(Debug, Deserialize)]
struct WsClientMessage {
    /// The command to execute (mirrors REST endpoints).
    action: String,

    /// Optional correlation ID echoed back in the response.
    #[serde(default)]
    request_id: Option<String>,

    /// Game UUID (required for game-specific actions).
    #[serde(default)]
    game_id: Option<String>,

    /// Move origin square (for `submit_move`).
    #[serde(default)]
    from: Option<String>,

    /// Move target square (for `submit_move`).
    #[serde(default)]
    to: Option<String>,

    /// Promotion piece (for `submit_move`): "Q", "R", "B", "N" or null.
    #[serde(default)]
    promotion: Option<String>,

    /// Action type for `submit_action`: "resign", "offer_draw", etc.
    #[serde(default)]
    action_type: Option<String>,

    /// Reason for a draw claim (for `submit_action`).
    #[serde(default)]
    reason: Option<String>,

    /// Move number for `replay_archived`.
    #[serde(default)]
    move_number: Option<usize>,
}

// ---------------------------------------------------------------------------
// Server → Client response helpers
// ---------------------------------------------------------------------------

/// Builds a JSON success response string for a client command.
fn build_response(
    action: &str,
    request_id: &Option<String>,
    data: &serde_json::Value,
) -> String {
    serde_json::json!({
        "type": "response",
        "action": action,
        "request_id": request_id,
        "success": true,
        "data": data,
    })
    .to_string()
}

/// Builds a JSON error response string for a client command.
fn build_error_response(
    action: &str,
    request_id: &Option<String>,
    error: &str,
) -> String {
    serde_json::json!({
        "type": "response",
        "action": action,
        "request_id": request_id,
        "success": false,
        "error": error,
    })
    .to_string()
}

/// Builds a JSON event string for broadcasting to subscribers.
fn build_event_json(event: &str, game_id: &Uuid, payload: &str) -> String {
    // Parse the payload so it is embedded as an object, not a string
    let data: serde_json::Value =
        serde_json::from_str(payload).unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "type": "event",
        "event": event,
        "game_id": game_id.to_string(),
        "data": data,
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// WsSession — per-connection actor
// ---------------------------------------------------------------------------

/// Actor representing a single WebSocket client connection.
///
/// Responsibilities:
/// - Parse incoming JSON commands and dispatch them to the `GameManager`
/// - Send JSON responses and error messages back to the client
/// - Maintain a heartbeat (ping/pong) to detect stale connections
/// - Register/unregister with the `GameBroadcaster` for real-time events
pub struct WsSession {
    /// Unique identifier for this session.
    id: Uuid,

    /// Timestamp of the last received pong (or initial connect time).
    last_heartbeat: Instant,

    /// Shared application state (contains the game manager).
    app_state: web::Data<AppState>,

    /// Address of the central broadcaster actor.
    broadcaster: Addr<GameBroadcaster>,
}

impl WsSession {
    /// Creates a new WebSocket session.
    pub fn new(app_state: web::Data<AppState>, broadcaster: Addr<GameBroadcaster>) -> Self {
        Self {
            id: Uuid::new_v4(),
            last_heartbeat: Instant::now(),
            app_state,
            broadcaster,
        }
    }

    /// Starts a periodic heartbeat check. If the client has not responded
    /// to a ping within `CLIENT_TIMEOUT`, the connection is closed.
    fn start_heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.last_heartbeat) > CLIENT_TIMEOUT {
                log::warn!("WS session {} heartbeat timeout, disconnecting", act.id);
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }

    // -----------------------------------------------------------------------
    // Command dispatch
    // -----------------------------------------------------------------------

    /// Top-level command dispatcher. Parses the action field and routes
    /// to the appropriate handler method.
    fn handle_message(&self, text: &str, ctx: &mut ws::WebsocketContext<Self>) {
        let msg: WsClientMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                let err = build_error_response("unknown", &None, &format!("Invalid JSON: {}", e));
                ctx.text(err);
                return;
            }
        };

        let response = match msg.action.as_str() {
            "create_game" => self.handle_create_game(&msg),
            "list_games" => self.handle_list_games(&msg),
            "get_game" => self.handle_get_game(&msg),
            "delete_game" => self.handle_delete_game(&msg),
            "submit_move" => self.handle_submit_move(&msg),
            "submit_action" => self.handle_submit_action(&msg),
            "get_legal_moves" => self.handle_get_legal_moves(&msg),
            "get_board" => self.handle_get_board(&msg),
            "subscribe" => self.handle_subscribe(&msg),
            "unsubscribe" => self.handle_unsubscribe(&msg),
            "list_archived" => self.handle_list_archived(&msg),
            "get_archived" => self.handle_get_archived(&msg),
            "replay_archived" => self.handle_replay_archived(&msg),
            "get_storage_stats" => self.handle_get_storage_stats(&msg),
            _ => build_error_response(
                &msg.action,
                &msg.request_id,
                &format!("Unknown action: '{}'", msg.action),
            ),
        };

        ctx.text(response);
    }

    // -----------------------------------------------------------------------
    // Helper: parse + validate game_id from client message
    // -----------------------------------------------------------------------

    /// Extracts and parses the `game_id` field from a client message.
    /// Returns `Err(response_string)` with a pre-built error if missing or
    /// invalid, so callers can simply return early.
    fn parse_game_id(&self, msg: &WsClientMessage) -> Result<Uuid, String> {
        let id_str = msg
            .game_id
            .as_deref()
            .ok_or_else(|| {
                build_error_response(&msg.action, &msg.request_id, "Missing field: game_id")
            })?;
        Uuid::parse_str(id_str).map_err(|_| {
            build_error_response(
                &msg.action,
                &msg.request_id,
                &format!("Invalid game_id: {}", id_str),
            )
        })
    }

    // -----------------------------------------------------------------------
    // Action handlers (one per REST endpoint equivalent)
    // -----------------------------------------------------------------------

    /// Creates a new chess game (mirrors `POST /api/games`).
    fn handle_create_game(&self, msg: &WsClientMessage) -> String {
        let mut manager = self.app_state.game_manager.lock().unwrap();
        let game_id = manager.create_game();

        log::info!("WS: Created new game: {}", game_id);

        // Broadcast a "game_created" event
        let payload = serde_json::json!({ "game_id": game_id.to_string() }).to_string();
        self.broadcaster.do_send(BroadcastEvent {
            game_id,
            event: "game_created".to_string(),
            payload,
        });

        build_response(
            &msg.action,
            &msg.request_id,
            &serde_json::json!({
                "game_id": game_id.to_string(),
                "message": "New chess game created. White to move.",
            }),
        )
    }

    /// Lists all active games (mirrors `GET /api/games`).
    fn handle_list_games(&self, msg: &WsClientMessage) -> String {
        let manager = self.app_state.game_manager.lock().unwrap();
        let summaries: Vec<serde_json::Value> = manager
            .games
            .values()
            .map(|g| {
                serde_json::json!({
                    "game_id": g.id.to_string(),
                    "turn": g.turn,
                    "fullmove_number": g.fullmove_number,
                    "is_over": g.is_over(),
                    "result": g.result,
                })
            })
            .collect();

        let total = summaries.len();
        build_response(
            &msg.action,
            &msg.request_id,
            &serde_json::json!({ "games": summaries, "total": total }),
        )
    }

    /// Retrieves the full state of a game (mirrors `GET /api/games/{id}`).
    fn handle_get_game(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        let manager = self.app_state.game_manager.lock().unwrap();
        match manager.get_game(&game_id) {
            Some(game) => {
                let is_check = movegen::is_in_check(&game.board, game.turn);
                let legal_moves = game.legal_moves();

                build_response(
                    &msg.action,
                    &msg.request_id,
                    &serde_json::json!({
                        "game_id": game.id.to_string(),
                        "state": game.to_game_state_json(),
                        "is_over": game.is_over(),
                        "result": game.result,
                        "end_reason": game.end_reason,
                        "is_check": is_check,
                        "legal_move_count": legal_moves.len(),
                        "move_history": game.move_history,
                    }),
                )
            }
            None => build_error_response(
                &msg.action,
                &msg.request_id,
                &format!("Game {} not found", game_id),
            ),
        }
    }

    /// Deletes a game (mirrors `DELETE /api/games/{id}`).
    fn handle_delete_game(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        let mut manager = self.app_state.game_manager.lock().unwrap();
        if manager.delete_game(&game_id) {
            log::info!("WS: Deleted game: {}", game_id);

            // Broadcast a "game_deleted" event
            let payload = serde_json::json!({ "game_id": game_id.to_string() }).to_string();
            self.broadcaster.do_send(BroadcastEvent {
                game_id,
                event: "game_deleted".to_string(),
                payload,
            });

            build_response(
                &msg.action,
                &msg.request_id,
                &serde_json::json!({ "message": format!("Game {} deleted", game_id) }),
            )
        } else {
            build_error_response(
                &msg.action,
                &msg.request_id,
                &format!("Game {} not found", game_id),
            )
        }
    }

    /// Submits a move for the current side (mirrors `POST /api/games/{id}/move`).
    fn handle_submit_move(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        // Validate required fields
        let from = match &msg.from {
            Some(f) => f.clone(),
            None => {
                return build_error_response(
                    &msg.action,
                    &msg.request_id,
                    "Missing field: from",
                );
            }
        };
        let to = match &msg.to {
            Some(t) => t.clone(),
            None => {
                return build_error_response(
                    &msg.action,
                    &msg.request_id,
                    "Missing field: to",
                );
            }
        };

        let mut manager = self.app_state.game_manager.lock().unwrap();

        // Scope the mutable borrow so we can call persist_game afterwards
        let result = {
            let game = match manager.get_game_mut(&game_id) {
                Some(g) => g,
                None => {
                    return build_error_response(
                        &msg.action,
                        &msg.request_id,
                        &format!("Game {} not found", game_id),
                    );
                }
            };

            let move_json = MoveJson {
                from: from.clone(),
                to: to.clone(),
                promotion: msg.promotion.clone(),
            };

            match game.make_move(&move_json) {
                Ok(()) => {
                    let is_check = movegen::is_in_check(&game.board, game.turn);
                    let message = if game.is_over() {
                        format!(
                            "Game over: {} ({})",
                            game.result.as_ref().unwrap(),
                            game.end_reason.as_ref().unwrap()
                        )
                    } else if is_check {
                        format!("{} to move. Check!", game.turn)
                    } else {
                        format!("{} to move.", game.turn)
                    };

                    log::info!("WS Game {}: Move {}{} accepted. {}", game_id, from, to, message);

                    Ok(serde_json::json!({
                        "success": true,
                        "message": message,
                        "state": game.to_game_state_json(),
                        "is_over": game.is_over(),
                        "result": game.result,
                        "end_reason": game.end_reason,
                        "is_check": is_check,
                    }))
                }
                Err(err) => {
                    log::warn!("WS Game {}: Illegal move {}{}: {}", game_id, from, to, err);
                    Err(err)
                }
            }
        };

        match result {
            Ok(data) => {
                manager.persist_game(&game_id);

                // Broadcast the game update to all subscribers
                self.broadcaster.do_send(BroadcastEvent {
                    game_id,
                    event: "game_updated".to_string(),
                    payload: data.to_string(),
                });

                build_response(&msg.action, &msg.request_id, &data)
            }
            Err(err) => {
                build_error_response(&msg.action, &msg.request_id, &err)
            }
        }
    }

    /// Submits a special action (mirrors `POST /api/games/{id}/action`).
    fn handle_submit_action(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        let action_type = match &msg.action_type {
            Some(a) => a.clone(),
            None => {
                return build_error_response(
                    &msg.action,
                    &msg.request_id,
                    "Missing field: action_type",
                );
            }
        };

        let mut manager = self.app_state.game_manager.lock().unwrap();

        // Scope the mutable borrow so we can call persist_game afterwards
        let result = {
            let game = match manager.get_game_mut(&game_id) {
                Some(g) => g,
                None => {
                    return build_error_response(
                        &msg.action,
                        &msg.request_id,
                        &format!("Game {} not found", game_id),
                    );
                }
            };

            let action = ActionJson {
                action: action_type.clone(),
                reason: msg.reason.clone(),
            };

            match game.process_action(&action) {
                Ok(()) => {
                    let is_check = movegen::is_in_check(&game.board, game.turn);
                    let message = if game.is_over() {
                        format!(
                            "Game over: {} ({})",
                            game.result.as_ref().unwrap(),
                            game.end_reason.as_ref().unwrap()
                        )
                    } else {
                        format!("Action '{}' processed.", action_type)
                    };

                    log::info!(
                        "WS Game {}: Action '{}' accepted. {}",
                        game_id,
                        action_type,
                        message
                    );

                    Ok(serde_json::json!({
                        "success": true,
                        "message": message,
                        "state": game.to_game_state_json(),
                        "is_over": game.is_over(),
                        "result": game.result,
                        "end_reason": game.end_reason,
                        "is_check": is_check,
                    }))
                }
                Err(err) => {
                    log::warn!(
                        "WS Game {}: Action '{}' rejected: {}",
                        game_id,
                        action_type,
                        err
                    );
                    Err(err)
                }
            }
        };

        match result {
            Ok(data) => {
                manager.persist_game(&game_id);

                // Broadcast the game update to all subscribers
                self.broadcaster.do_send(BroadcastEvent {
                    game_id,
                    event: "game_updated".to_string(),
                    payload: data.to_string(),
                });

                build_response(&msg.action, &msg.request_id, &data)
            }
            Err(err) => {
                build_error_response(&msg.action, &msg.request_id, &err)
            }
        }
    }

    /// Returns all legal moves for the current position
    /// (mirrors `GET /api/games/{id}/moves`).
    fn handle_get_legal_moves(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        let manager = self.app_state.game_manager.lock().unwrap();
        match manager.get_game(&game_id) {
            Some(game) => {
                let legal_moves = game.legal_moves();
                let move_jsons: Vec<MoveJson> =
                    legal_moves.iter().map(|m| m.to_json()).collect();
                let count = move_jsons.len();

                build_response(
                    &msg.action,
                    &msg.request_id,
                    &serde_json::json!({
                        "turn": game.turn,
                        "moves": move_jsons,
                        "count": count,
                    }),
                )
            }
            None => build_error_response(
                &msg.action,
                &msg.request_id,
                &format!("Game {} not found", game_id),
            ),
        }
    }

    /// Returns an ASCII board representation
    /// (mirrors `GET /api/games/{id}/board`).
    fn handle_get_board(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        let manager = self.app_state.game_manager.lock().unwrap();
        match manager.get_game(&game_id) {
            Some(game) => {
                let ascii = board_to_ascii(&game.board, game.turn);
                build_response(
                    &msg.action,
                    &msg.request_id,
                    &serde_json::json!({ "board": ascii }),
                )
            }
            None => build_error_response(
                &msg.action,
                &msg.request_id,
                &format!("Game {} not found", game_id),
            ),
        }
    }

    /// Subscribes the client to real-time events for a game.
    fn handle_subscribe(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        self.broadcaster.do_send(Subscribe {
            session_id: self.id,
            game_id,
        });

        build_response(
            &msg.action,
            &msg.request_id,
            &serde_json::json!({
                "message": format!("Subscribed to game {}", game_id),
                "game_id": game_id.to_string(),
            }),
        )
    }

    /// Unsubscribes the client from real-time events for a game.
    fn handle_unsubscribe(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        self.broadcaster.do_send(Unsubscribe {
            session_id: self.id,
            game_id,
        });

        build_response(
            &msg.action,
            &msg.request_id,
            &serde_json::json!({
                "message": format!("Unsubscribed from game {}", game_id),
                "game_id": game_id.to_string(),
            }),
        )
    }

    /// Lists all archived (completed) games (mirrors `GET /api/archive`).
    fn handle_list_archived(&self, msg: &WsClientMessage) -> String {
        let manager = self.app_state.game_manager.lock().unwrap();
        let archived_ids = match manager.storage.list_archived() {
            Ok(ids) => ids,
            Err(e) => {
                return build_error_response(
                    &msg.action,
                    &msg.request_id,
                    &format!("Failed to list archives: {}", e),
                );
            }
        };

        let mut games = Vec::new();
        for id in &archived_ids {
            if let Ok(archive) = manager.storage.load_archive(id) {
                let compressed_bytes = manager.storage.archive_file_size(id).unwrap_or(0);
                games.push(serde_json::json!({
                    "game_id": id.to_string(),
                    "move_count": archive.move_count(),
                    "result": archive.result,
                    "end_reason": archive.end_reason,
                    "start_timestamp": archive.start_timestamp,
                    "end_timestamp": archive.end_timestamp,
                    "compressed_bytes": compressed_bytes,
                    "raw_bytes": archive.raw_size(),
                }));
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

        build_response(
            &msg.action,
            &msg.request_id,
            &serde_json::json!({
                "games": games,
                "total": total,
                "storage": stats,
            }),
        )
    }

    /// Retrieves details of an archived game (mirrors `GET /api/archive/{id}`).
    fn handle_get_archived(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        let manager = self.app_state.game_manager.lock().unwrap();
        let (archive, _compressed) = match manager.storage.load_any(&game_id) {
            Ok(result) => result,
            Err(e) => {
                return build_error_response(&msg.action, &msg.request_id, &e);
            }
        };

        match archive.replay_full() {
            Ok(game) => {
                let is_check = movegen::is_in_check(&game.board, game.turn);
                build_response(
                    &msg.action,
                    &msg.request_id,
                    &serde_json::json!({
                        "game_id": game_id.to_string(),
                        "at_move": archive.move_count(),
                        "total_moves": archive.move_count(),
                        "state": game.to_game_state_json(),
                        "is_over": game.is_over(),
                        "result": game.result,
                        "is_check": is_check,
                    }),
                )
            }
            Err(e) => build_error_response(
                &msg.action,
                &msg.request_id,
                &format!("Failed to replay game: {}", e),
            ),
        }
    }

    /// Replays an archived game to a specific move number
    /// (mirrors `GET /api/archive/{id}/replay`).
    fn handle_replay_archived(&self, msg: &WsClientMessage) -> String {
        let game_id = match self.parse_game_id(msg) {
            Ok(id) => id,
            Err(e) => return e,
        };

        let manager = self.app_state.game_manager.lock().unwrap();
        let (archive, _compressed) = match manager.storage.load_any(&game_id) {
            Ok(result) => result,
            Err(e) => {
                return build_error_response(&msg.action, &msg.request_id, &e);
            }
        };

        let up_to = msg.move_number.unwrap_or(archive.move_count());

        match archive.replay(up_to) {
            Ok(game) => {
                let is_check = movegen::is_in_check(&game.board, game.turn);
                let actual_move = up_to.min(archive.move_count());
                build_response(
                    &msg.action,
                    &msg.request_id,
                    &serde_json::json!({
                        "game_id": game_id.to_string(),
                        "at_move": actual_move,
                        "total_moves": archive.move_count(),
                        "state": game.to_game_state_json(),
                        "is_over": game.is_over(),
                        "result": game.result,
                        "is_check": is_check,
                    }),
                )
            }
            Err(e) => build_error_response(
                &msg.action,
                &msg.request_id,
                &format!("Failed to replay game: {}", e),
            ),
        }
    }

    /// Returns storage statistics (mirrors `GET /api/archive/stats`).
    fn handle_get_storage_stats(&self, msg: &WsClientMessage) -> String {
        let manager = self.app_state.game_manager.lock().unwrap();
        match manager.storage.stats() {
            Ok(stats) => build_response(
                &msg.action,
                &msg.request_id,
                &serde_json::to_value(&stats).unwrap_or(serde_json::Value::Null),
            ),
            Err(e) => build_error_response(
                &msg.action,
                &msg.request_id,
                &format!("Failed to get storage stats: {}", e),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// WsSession — Actor + StreamHandler implementation
// ---------------------------------------------------------------------------

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    /// Called when the session actor starts. Registers with the broadcaster
    /// and begins the heartbeat timer.
    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("WS session {} started", self.id);

        // Start the heartbeat ping/pong loop
        self.start_heartbeat(ctx);

        // Register this session with the broadcaster
        self.broadcaster.do_send(Connect {
            addr: ctx.address(),
            session_id: self.id,
        });
    }

    /// Called when the session actor stops. Unregisters from the broadcaster.
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        log::info!("WS session {} stopped", self.id);

        // Unregister from the broadcaster
        self.broadcaster.do_send(Disconnect {
            session_id: self.id,
        });
    }
}

/// Handler for incoming WebSocket frames (text, binary, ping, pong, close).
impl actix::StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                // Dispatch the JSON command
                self.handle_message(&text, ctx);
            }
            Ok(ws::Message::Binary(_)) => {
                log::warn!("WS session {}: binary messages not supported", self.id);
                ctx.text(build_error_response(
                    "binary",
                    &None,
                    "Binary messages are not supported. Please send JSON text.",
                ));
            }
            Ok(ws::Message::Ping(data)) => {
                self.last_heartbeat = Instant::now();
                ctx.pong(&data);
            }
            Ok(ws::Message::Pong(_)) => {
                self.last_heartbeat = Instant::now();
            }
            Ok(ws::Message::Close(reason)) => {
                log::info!("WS session {} closed: {:?}", self.id, reason);
                ctx.close(reason);
                ctx.stop();
            }
            Ok(ws::Message::Continuation(_)) => {
                // Continuation frames are handled internally by actix
            }
            Ok(ws::Message::Nop) => {}
            Err(e) => {
                log::error!("WS session {} protocol error: {}", self.id, e);
                ctx.stop();
            }
        }
    }
}

/// Handler for broadcaster-pushed text messages (events forwarded from
/// the `GameBroadcaster` to this session's WebSocket).
impl Handler<WsText> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: WsText, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

// ---------------------------------------------------------------------------
// HTTP → WebSocket upgrade handler
// ---------------------------------------------------------------------------

/// Upgrades an HTTP request to a WebSocket connection.
///
/// This is the entry point registered as a route. It creates a new
/// `WsSession` actor and starts the WebSocket handshake.
pub async fn ws_connect(
    req: HttpRequest,
    stream: web::Payload,
    app_state: web::Data<AppState>,
    broadcaster: web::Data<Addr<GameBroadcaster>>,
) -> Result<HttpResponse, actix_web::Error> {
    let session = WsSession::new(app_state, broadcaster.get_ref().clone());
    log::info!("New WebSocket connection request from {:?}", req.peer_addr());
    ws::start(session, &req, stream)
}

// ---------------------------------------------------------------------------
// Broadcast helper for REST API handlers
// ---------------------------------------------------------------------------

/// Sends a game event through the broadcaster so that all subscribed
/// WebSocket clients receive real-time updates. This function is called
/// from the REST API handlers whenever a game state changes.
pub fn broadcast_game_event(
    broadcaster: &web::Data<Addr<GameBroadcaster>>,
    game_id: Uuid,
    event: &str,
    data: &serde_json::Value,
) {
    broadcaster.do_send(BroadcastEvent {
        game_id,
        event: event.to_string(),
        payload: data.to_string(),
    });
}
