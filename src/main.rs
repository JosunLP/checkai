//! # CheckAI — Chess Server for AI Agents
//!
//! CheckAI is a Rust application that provides both a terminal interface
//! and a REST + WebSocket API for playing chess. It is designed to facilitate
//! chess games between AI agents, following the FIDE 2023 Laws of Chess.
//!
//! ## Features
//!
//! - **Complete Chess Engine**: Full move generation and validation
//!   following FIDE 2023 rules, including castling, en passant,
//!   promotion, check/checkmate/stalemate detection, and all draw
//!   conditions.
//!
//! - **REST API**: JSON-based API for AI agents to create games,
//!   query state, submit moves, and handle special actions (draw,
//!   resign). Uses the protocol defined in AGENT.md.
//!
//! - **WebSocket API**: Full reactive WebSocket support at `/ws`,
//!   mirroring every REST endpoint. Clients can subscribe to games
//!   and receive real-time push events for moves, state changes,
//!   and game deletions.
//!
//! - **Swagger/OpenAPI Documentation**: Auto-generated API docs
//!   available at `/swagger-ui/`.
//!
//! - **Terminal Interface**: Colored board display with interactive
//!   move input for local two-player games.
//!
//! ## Usage
//!
//! ```bash
//! # Start the API server (default: http://0.0.0.0:8080)
//! checkai serve
//!
//! # Start the API server on a custom port
//! checkai serve --port 3000
//!
//! # Play a local terminal game
//! checkai play
//! ```
//!
//! ## API Endpoints
//!
//! | Method | Path                          | Description                    |
//! |--------|-------------------------------|--------------------------------|
//! | POST   | `/api/games`                  | Create a new game              |
//! | GET    | `/api/games`                  | List all games                 |
//! | GET    | `/api/games/{id}`             | Get game state                 |
//! | DELETE | `/api/games/{id}`             | Delete a game                  |
//! | POST   | `/api/games/{id}/move`        | Submit a move                  |
//! | POST   | `/api/games/{id}/action`      | Submit an action               |
//! | GET    | `/api/games/{id}/moves`       | Get legal moves                |
//! | GET    | `/api/games/{id}/board`       | Get ASCII board                |
//! | GET    | `/ws`                         | WebSocket endpoint             |
//! | GET    | `/swagger-ui/`               | Swagger UI documentation       |

pub mod api;
pub mod export;
pub mod game;
pub mod movegen;
pub mod storage;
pub mod terminal;
pub mod types;
pub mod ws;

use actix::Actor;
use actix_cors::Cors;
use actix_web::{web, App, HttpServer, middleware};
use clap::{Parser, Subcommand};
use std::str::FromStr;
use std::sync::Mutex;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::{ApiDoc, AppState};
use crate::game::GameManager;
use crate::ws::GameBroadcaster;

/// CheckAI — A chess server and CLI for AI agents.
///
/// Provides a REST API with Swagger documentation and a terminal
/// interface for playing chess following FIDE 2023 rules.
#[derive(Parser, Debug)]
#[command(name = "checkai")]
#[command(about = "Chess server for AI agents — FIDE 2023 rules")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the REST API server with Swagger UI.
    Serve {
        /// Port to listen on.
        #[arg(short, long, default_value_t = 8080)]
        port: u16,

        /// Host address to bind to.
        #[arg(long, default_value = "0.0.0.0")]
        host: String,

        /// Directory for game storage (active + archive).
        #[arg(long, default_value = "data")]
        data_dir: String,
    },

    /// Play a chess game in the terminal (two-player).
    Play,

    /// Export archived games in human-readable format.
    Export {
        /// Directory for game storage.
        #[arg(long, default_value = "data")]
        data_dir: String,

        /// Output format: text, pgn, or json.
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Export a specific game by UUID.
        #[arg(short, long)]
        game_id: Option<String>,

        /// List all archived games (no export).
        #[arg(short, long)]
        list: bool,

        /// Export all archived games.
        #[arg(short, long)]
        all: bool,

        /// Write output to a file instead of stdout.
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { port, host, data_dir } => {
            run_server(&host, port, &data_dir).await
        }
        Commands::Play => {
            terminal::run_terminal_game();
            Ok(())
        }
        Commands::Export {
            data_dir,
            format,
            game_id,
            list,
            all,
            output,
        } => {
            let fmt = export::ExportFormat::from_str(&format)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            export::run_export(
                &data_dir,
                fmt,
                game_id.as_deref(),
                list,
                all,
                output.as_deref(),
            )
            .map_err(std::io::Error::other)
        }
    }
}

/// Starts the HTTP + WebSocket server with all API routes and Swagger UI.
async fn run_server(host: &str, port: u16, data_dir: &str) -> std::io::Result<()> {
    let openapi = ApiDoc::openapi();

    let game_manager = web::Data::new(AppState {
        game_manager: Mutex::new(GameManager::new(data_dir)),
    });

    // Start the central WebSocket event broadcaster actor
    let broadcaster = GameBroadcaster::new().start();
    let broadcaster_data = web::Data::new(broadcaster);

    log::info!("Starting CheckAI server on {}:{}", host, port);
    log::info!("Game storage directory: {}", data_dir);
    log::info!("Web UI available at http://{}:{}/", host, port);
    log::info!("Swagger UI available at http://{}:{}/swagger-ui/", host, port);
    log::info!("API base URL: http://{}:{}/api", host, port);
    log::info!("WebSocket endpoint: ws://{}:{}/ws", host, port);

    HttpServer::new(move || {
        // Configure CORS to allow all origins (for development/agent access)
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .app_data(game_manager.clone())
            .app_data(broadcaster_data.clone())
            .configure(api::configure_routes)
            .route("/ws", web::get().to(ws::ws_connect))
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", openapi.clone()),
            )
            // Serve the bQuery web UI static files
            .service(actix_files::Files::new("/web", "web").show_files_listing())
            // Redirect root "/" to the web UI
            .route("/", web::get().to(|| async {
                actix_web::HttpResponse::Found()
                    .append_header(("Location", "/web/index.html"))
                    .finish()
            }))
    })
    .bind((host, port))?
    .run()
    .await
}
