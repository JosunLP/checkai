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

pub mod analysis;
pub mod analysis_api;
pub mod api;
pub mod eval;
pub mod export;
pub mod game;
pub mod i18n;
pub mod movegen;
pub mod opening_book;
pub mod polyglot_keys;
pub mod search;
pub mod storage;
pub mod tablebase;
pub mod terminal;
pub mod types;
pub mod update;
pub mod ws;
pub mod zobrist;

#[macro_use]
extern crate rust_i18n;

// Initialize i18n with locale files from the "locales" directory.
// Falls back to English when a key is missing in the active locale.
rust_i18n::i18n!("locales", fallback = "en");

use actix::Actor;
use actix_cors::Cors;
use actix_web::{App, HttpResponse, HttpServer, middleware, web};
use clap::{Parser, Subcommand};
use rust_embed::RustEmbed;
use std::str::FromStr;
use std::sync::Mutex;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::analysis::{AnalysisConfig, AnalysisManager};
use crate::api::{ApiDoc, AppState};
use crate::game::GameManager;
use crate::ws::GameBroadcaster;

/// Embedded web UI assets (compiled into the binary).
#[derive(RustEmbed)]
#[folder = "web/"]
struct WebAssets;

/// Serves embedded web UI files.
async fn serve_web_asset(path: web::Path<String>) -> HttpResponse {
    let file_path = path.into_inner();
    match WebAssets::get(&file_path) {
        Some(content) => {
            let mime_type = match file_path.rsplit('.').next() {
                Some("html") => "text/html; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("js") => "application/javascript; charset=utf-8",
                Some("json") => "application/json",
                Some("png") => "image/png",
                Some("svg") => "image/svg+xml",
                Some("ico") => "image/x-icon",
                Some("woff2") => "font/woff2",
                Some("woff") => "font/woff",
                _ => "application/octet-stream",
            };
            HttpResponse::Ok()
                .content_type(mime_type)
                .body(content.data.into_owned())
        }
        None => HttpResponse::NotFound().finish(),
    }
}

/// CheckAI — A chess server and CLI for AI agents.
///
/// Provides a REST API with Swagger documentation and a terminal
/// interface for playing chess following FIDE 2023 rules.
#[derive(Parser, Debug)]
#[command(name = "checkai")]
#[command(about = "Chess server for AI agents — FIDE 2023 rules")]
#[command(version)]
struct Cli {
    /// Override the language / locale (e.g. "de", "fr", "zh-CN").
    #[arg(short, long, global = true)]
    lang: Option<String>,

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

        /// Path to a Polyglot opening book (.bin) for analysis.
        #[arg(long)]
        book_path: Option<String>,

        /// Path to a Syzygy tablebase directory for analysis.
        #[arg(long)]
        tablebase_path: Option<String>,

        /// Minimum search depth for analysis (≥ 30).
        #[arg(long, default_value_t = 30)]
        analysis_depth: u32,

        /// Transposition table size in MB for analysis.
        #[arg(long, default_value_t = 64)]
        tt_size_mb: usize,

        /// Maximum number of analysis jobs retained in memory.
        #[arg(long, default_value_t = 256)]
        analysis_max_jobs: usize,

        /// Maximum number of concurrently active analysis jobs (queued + running).
        #[arg(long, default_value_t = 4)]
        analysis_max_concurrent_jobs: usize,

        /// TTL for finished analysis jobs in seconds (0 disables TTL-based eviction).
        #[arg(long, default_value_t = 3600)]
        analysis_completed_ttl_secs: u64,
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

    /// Update CheckAI to the latest version from GitHub.
    Update,

    /// Print the current version.
    Version,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    // Set the active locale: --lang flag takes priority, then system detection
    let locale = match &cli.lang {
        Some(lang) => i18n::normalize_locale(lang).unwrap_or_else(|| "en".to_string()),
        None => i18n::detect_system_locale(),
    };
    rust_i18n::set_locale(&locale);

    // Clean up leftover .old.exe from previous updates (Windows)
    update::cleanup_old_binary();

    match cli.command {
        Commands::Serve {
            port,
            host,
            data_dir,
            book_path,
            tablebase_path,
            analysis_depth,
            tt_size_mb,
            analysis_max_jobs,
            analysis_max_concurrent_jobs,
            analysis_completed_ttl_secs,
        } => {
            // Check for updates in the background before starting the server
            update::check_for_updates().await;
            run_server(
                &host,
                port,
                &data_dir,
                book_path,
                tablebase_path,
                analysis_depth,
                tt_size_mb,
                analysis_max_jobs,
                analysis_max_concurrent_jobs,
                analysis_completed_ttl_secs,
            )
            .await
        }
        Commands::Play => {
            update::check_for_updates().await;
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
        Commands::Update => {
            update::perform_update()
                .await
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok(())
        }
        Commands::Version => {
            println!("checkai v{}", update::version());
            Ok(())
        }
    }
}

/// Starts the HTTP + WebSocket server with all API routes and Swagger UI.
async fn run_server(
    host: &str,
    port: u16,
    data_dir: &str,
    book_path: Option<String>,
    tablebase_path: Option<String>,
    analysis_depth: u32,
    tt_size_mb: usize,
    analysis_max_jobs: usize,
    analysis_max_concurrent_jobs: usize,
    analysis_completed_ttl_secs: u64,
) -> std::io::Result<()> {
    let openapi = ApiDoc::openapi();

    let game_manager = web::Data::new(AppState {
        game_manager: Mutex::new(GameManager::new(data_dir)),
    });

    // Start the central WebSocket event broadcaster actor
    let broadcaster = GameBroadcaster::new().start();
    let broadcaster_data = web::Data::new(broadcaster);

    // Initialize the analysis manager
    let analysis_config = AnalysisConfig {
        min_depth: analysis_depth.max(30),
        book_path: book_path.map(std::path::PathBuf::from),
        tablebase_path: tablebase_path.map(std::path::PathBuf::from),
        tt_size_mb,
        max_jobs_retained: analysis_max_jobs.max(1),
        max_concurrent_jobs: analysis_max_concurrent_jobs.max(1),
        completed_job_ttl_secs: if analysis_completed_ttl_secs == 0 {
            None
        } else {
            Some(analysis_completed_ttl_secs)
        },
    };
    let analysis_max_jobs = analysis_config.max_jobs_retained;
    let analysis_max_active = analysis_config.max_concurrent_jobs;
    let analysis_ttl_label = analysis_config
        .completed_job_ttl_secs
        .map(|v| v.to_string())
        .unwrap_or_else(|| "disabled".to_string());
    let analysis_manager = web::Data::new(AnalysisManager::new(analysis_config));

    log::info!("Starting CheckAI server on {}:{}", host, port);
    log::info!("Game storage directory: {}", data_dir);
    log::info!("Web UI available at http://{}:{}/", host, port);
    log::info!(
        "Swagger UI available at http://{}:{}/swagger-ui/",
        host,
        port
    );
    log::info!("API base URL: http://{}:{}/api", host, port);
    log::info!("WebSocket endpoint: ws://{}:{}/ws", host, port);
    log::info!(
        "Analysis engine: depth={}, TT={}MB",
        analysis_depth.max(30),
        tt_size_mb
    );
    log::info!(
        "Analysis job limits: max_jobs={}, max_active={}, finished_ttl={}s",
        analysis_max_jobs,
        analysis_max_active,
        analysis_ttl_label
    );

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
            .app_data(analysis_manager.clone())
            .configure(api::configure_routes)
            .configure(analysis_api::configure_analysis_routes)
            .route("/ws", web::get().to(ws::ws_connect))
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", openapi.clone()),
            )
            // Serve the embedded bQuery web UI
            .route("/web/{filename:.*}", web::get().to(serve_web_asset))
            // Redirect root "/" to the web UI
            .route(
                "/",
                web::get().to(|| async {
                    actix_web::HttpResponse::Found()
                        .append_header(("Location", "/web/index.html"))
                        .finish()
                }),
            )
    })
    .bind((host, port))?
    .run()
    .await
}
