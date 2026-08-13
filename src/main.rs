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
//! - **Analysis Engine**: An alpha-beta / PVS search (iterative
//!   deepening, transposition table, null-move pruning, LMR, SEE,
//!   futility/razoring, killer/history/counter-move ordering, and
//!   quiescence search) paired with a PeSTO-style evaluation. Used by
//!   the analysis API and the terminal commands below.
//!
//! - **Animated Terminal CLI**: Play against the built-in engine or a
//!   second human, watch engine-vs-engine games, analyze positions and
//!   games, benchmark the engine, run perft, or speak UCI for chess
//!   GUIs. Animated boards and search progress (via `crossterm` and
//!   `indicatif`) render on a TTY and degrade to plain text otherwise
//!   (also honoring `--no-color` / `NO_COLOR`).
//!
//! ## Commands
//!
//! | Command   | Description                                            |
//! |-----------|--------------------------------------------------------|
//! | `serve`   | Start the REST + WebSocket API server with Swagger UI  |
//! | `play`    | Play in the terminal (vs the engine by default)        |
//! | `watch`   | Watch the engine play itself (engine-vs-engine)        |
//! | `analyze` | Analyze a position (`--fen`) or a game (`--moves`)     |
//! | `bench`   | Run the fixed engine benchmark suite (nodes, NPS)      |
//! | `perft`   | Verify move generation with perft node counts          |
//! | `uci`     | Run as a UCI engine on stdin/stdout (for chess GUIs)   |
//! | `export`  | Export archived games as text, PGN, or JSON            |
//! | `update`  | Update CheckAI to the latest version from GitHub       |
//! | `version` | Print the current version                              |
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
//! # Play a terminal game against the built-in engine
//! checkai play
//!
//! # Run as a UCI engine for a chess GUI / match runner
//! checkai uci
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
pub mod cli;
pub mod engine_time;
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
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand};
use colored::Colorize;
use rust_embed::RustEmbed;
use std::str::FromStr;
use std::sync::Mutex;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::analysis::{AnalysisConfig, AnalysisManager};
use crate::api::{ApiDoc, AppState};
use crate::cli::{CliCommand, CliContext};
use crate::game::GameManager;
use crate::ws::GameBroadcaster;

/// Embedded Vite-built UI assets (compiled into the binary).
/// Built via `cd web && bun run build`.
#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct DistAssets;

/// Serves embedded web UI files from the Vite-built bundle.
async fn serve_web_asset(path: web::Path<String>) -> HttpResponse {
    let file_path = path.into_inner();

    // Map index.html → index.vite.html (Vite output filename)
    let dist_path = if file_path == "index.html" {
        "index.vite.html"
    } else {
        &file_path
    };

    match DistAssets::get(dist_path) {
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

/// CLI help styles for colored output.
const CLI_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

/// CheckAI — A chess server and CLI for AI agents.
///
/// Provides a REST API with Swagger documentation and a terminal
/// interface for playing chess following FIDE 2023 rules.
#[derive(Parser, Debug)]
#[command(name = "checkai")]
#[command(about = "Chess server for AI agents \u{2014} FIDE 2023 rules")]
#[command(long_about = "\
CheckAI is a chess server and CLI designed for AI agents.\n\
\n\
It provides a REST API with Swagger documentation, WebSocket support\n\
for real-time game events, and a terminal interface for local play.\n\
All chess rules follow the FIDE 2023 Laws of Chess.\n\
\n\
Features:\n\
  \u{2022} REST + WebSocket API for AI agent integration\n\
  \u{2022} Swagger UI for interactive API exploration\n\
  \u{2022} Animated terminal play with chess clocks and board themes\n\
  \u{2022} Multi-threaded search (Lazy SMP) with MultiPV analysis\n\
  \u{2022} Opening book, endgame tablebases and full UCI support\n\
  \u{2022} PGN import/export with standard algebraic notation\n\
  \u{2022} Game archival and export (text, PGN, JSON)\n\
  \u{2022} Internationalization (8 languages)")]
#[command(version)]
#[command(styles = CLI_STYLES)]
#[command(after_help = "\
Examples:\n\
  checkai serve              Start the API server on port 8080\n\
  checkai serve --port 3000  Start on a custom port\n\
  checkai play               Play vs the built-in engine (level 5)\n\
  checkai play --time 5+3    Play a five-minute game with increment\n\
  checkai play --vs human    Local two-player game\n\
  checkai watch              Watch an engine-vs-engine showcase\n\
  checkai analyze --pgn g.pgn  Annotate a game from a PGN file\n\
  checkai eval --fen ...     Inspect the evaluation of a position\n\
  checkai bench              Run the engine benchmark suite\n\
  checkai perft 5            Verify move generation vs references\n\
  checkai uci                Speak UCI for chess GUIs/match runners\n\
  checkai export --list      List all archived games\n\
  checkai update             Update to the latest version\n\
\n\
Every engine command accepts --threads, --hash, --book and --tablebase.\n\
\n\
Documentation: https://github.com/JosunLP/checkai")]
struct Cli {
    /// Override the language / locale (e.g. "de", "fr", "zh-CN").
    #[arg(short, long, global = true)]
    lang: Option<String>,

    /// Disable colored output (the NO_COLOR env var is honored too).
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Available subcommands.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the REST + WebSocket API server with Swagger UI.
    #[command(after_help = "\
Examples:\n\
  checkai serve                         Default: http://0.0.0.0:8080\n\
  checkai serve --port 3000             Custom port\n\
  checkai serve --host 127.0.0.1        Bind to localhost only\n\
  checkai serve --book-path book.bin    Enable opening book\n\
  checkai serve --tablebase-path tb/    Enable Syzygy tablebases")]
    Serve {
        /// Port to listen on.
        #[arg(short, long, default_value_t = 8080)]
        #[arg(help_heading = "Server")]
        port: u16,

        /// Host address to bind to.
        #[arg(long, default_value = "0.0.0.0")]
        #[arg(help_heading = "Server")]
        host: String,

        /// Directory for game storage (active + archive).
        #[arg(long, default_value = "data")]
        #[arg(help_heading = "Storage")]
        data_dir: String,

        /// Path to a Polyglot opening book (.bin).
        #[arg(long)]
        #[arg(help_heading = "Analysis")]
        book_path: Option<String>,

        /// Path to a Syzygy tablebase directory.
        #[arg(long)]
        #[arg(help_heading = "Analysis")]
        tablebase_path: Option<String>,

        /// Minimum search depth (\u{2265} 30).
        #[arg(long, default_value_t = 30)]
        #[arg(help_heading = "Analysis")]
        analysis_depth: u32,

        /// Transposition table size in MB.
        #[arg(long, default_value_t = 64)]
        #[arg(help_heading = "Analysis")]
        tt_size_mb: usize,

        /// Maximum analysis jobs retained in memory.
        #[arg(long, default_value_t = 256)]
        #[arg(help_heading = "Analysis")]
        analysis_max_jobs: usize,

        /// Maximum concurrent analysis jobs (queued + running).
        #[arg(long, default_value_t = 4)]
        #[arg(help_heading = "Analysis")]
        analysis_max_concurrent_jobs: usize,

        /// TTL for finished analysis jobs in seconds (0 = no TTL).
        #[arg(long, default_value_t = 3600)]
        #[arg(help_heading = "Analysis")]
        analysis_completed_ttl_secs: u64,
    },

    /// Play chess in the terminal — vs the built-in engine or a human.
    Play(cli::play::PlayArgs),

    /// Watch the engine play itself (engine-vs-engine showcase).
    Watch(cli::watch::WatchArgs),

    /// Analyze a position (--fen), a move list (--moves) or a PGN file.
    Analyze(cli::analyze::AnalyzeArgs),

    /// Inspect the static evaluation, ranked moves, book and tablebase.
    Eval(cli::eval::EvalArgs),

    /// Run the fixed engine benchmark suite (nodes, time, NPS).
    Bench(cli::bench::BenchArgs),

    /// Verify move generation with perft node counts.
    Perft(cli::perft::PerftArgs),

    /// Run as a UCI engine on stdin/stdout (for chess GUIs).
    Uci(cli::uci::UciArgs),

    /// Export archived games in various formats.
    #[command(after_help = "\
Examples:\n\
  checkai export --list                  List all archived games\n\
  checkai export --game-id <UUID>        Export a specific game\n\
  checkai export --all --format pgn      Export all as PGN\n\
  checkai export --all -o games.pgn      Write export to a file")]
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
        #[arg(long)]
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

/// Runtime configuration for starting the HTTP/WebSocket server.
struct ServeConfig {
    host: String,
    port: u16,
    data_dir: String,
    book_path: Option<String>,
    tablebase_path: Option<String>,
    analysis_depth: u32,
    tt_size_mb: usize,
    analysis_max_jobs: usize,
    analysis_max_concurrent_jobs: usize,
    analysis_completed_ttl_secs: u64,
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

    // UCI is machine-facing: plain output, no banners, no animations.
    let ctx = match cli.command {
        Some(Commands::Uci(_)) => CliContext {
            theme: cli::theme::Theme::plain(),
        },
        _ => CliContext::new(cli.no_color),
    };

    match cli.command {
        None => {
            cli::welcome::print_welcome(&ctx.theme);
            Ok(())
        }
        Some(Commands::Serve {
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
        }) => {
            // Check for updates in the background before starting the server
            update::check_for_updates().await;
            run_server(ServeConfig {
                host,
                port,
                data_dir,
                book_path,
                tablebase_path,
                analysis_depth,
                tt_size_mb,
                analysis_max_jobs,
                analysis_max_concurrent_jobs,
                analysis_completed_ttl_secs,
            })
            .await
        }
        Some(Commands::Play(args)) => {
            // Only ping GitHub for updates in interactive sessions;
            // piped/scripted runs stay fast and quiet.
            if ctx.theme.interactive {
                update::check_for_updates().await;
            }
            run_cli_command(args, &ctx)
        }
        Some(Commands::Watch(args)) => run_cli_command(args, &ctx),
        Some(Commands::Analyze(args)) => run_cli_command(args, &ctx),
        Some(Commands::Eval(args)) => run_cli_command(args, &ctx),
        Some(Commands::Bench(args)) => run_cli_command(args, &ctx),
        Some(Commands::Perft(args)) => run_cli_command(args, &ctx),
        Some(Commands::Uci(args)) => run_cli_command(args, &ctx),
        Some(Commands::Export {
            data_dir,
            format,
            game_id,
            list,
            all,
            output,
        }) => {
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
        Some(Commands::Update) => {
            update::perform_update()
                .await
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok(())
        }
        Some(Commands::Version) => {
            println!(
                "{} {}",
                "checkai".green().bold(),
                format!("v{}", update::version()).bold()
            );
            println!("  {} {}", t!("cli.locale_label"), &*rust_i18n::locale());
            println!(
                "  {} https://github.com/JosunLP/checkai",
                t!("cli.docs_label")
            );
            Ok(())
        }
    }
}

/// Dispatches a [`CliCommand`], converting CLI errors to exit-worthy
/// I/O errors for `main`'s signature.
fn run_cli_command(cmd: impl CliCommand, ctx: &CliContext) -> std::io::Result<()> {
    cmd.run(ctx)
        .map_err(|e| std::io::Error::other(e.to_string()))
}

/// Starts the HTTP + WebSocket server with all API routes and Swagger UI.
async fn run_server(cfg: ServeConfig) -> std::io::Result<()> {
    let ServeConfig {
        host,
        port,
        data_dir,
        book_path,
        tablebase_path,
        analysis_depth,
        tt_size_mb,
        analysis_max_jobs,
        analysis_max_concurrent_jobs,
        analysis_completed_ttl_secs,
    } = cfg;

    let openapi = ApiDoc::openapi();

    let game_manager = web::Data::new(AppState {
        game_manager: Mutex::new(GameManager::new(&data_dir)),
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
            // Order matters: actix matches services in registration order and
            // `/api` is a prefix of `/api/analysis`, so the narrower analysis
            // scope has to be registered first or every analysis request is
            // swallowed by the games scope and answered with a 404.
            .configure(analysis_api::configure_analysis_routes)
            .configure(api::configure_routes)
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
    .bind((host.as_str(), port))?
    .run()
    .await
}
