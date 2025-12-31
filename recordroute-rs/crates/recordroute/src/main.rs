use recordroute_common::{AppConfig, logger};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Find project root by looking for .git directory
fn find_project_root() -> Option<PathBuf> {
    let mut current_dir = std::env::current_dir().ok()?;

    loop {
        if current_dir.join(".git").exists() {
            return Some(current_dir);
        }

        if !current_dir.pop() {
            break;
        }
    }

    None
}

/// Load .env file from project root
fn load_dotenv_from_project_root() {
    if let Some(root) = find_project_root() {
        let env_path = root.join(".env");
        if env_path.exists() {
            dotenv::from_path(&env_path).ok();
        }
    } else {
        // Fallback to default dotenv behavior
        dotenv::dotenv().ok();
    }
}

#[derive(Parser)]
#[command(name = "recordroute")]
#[command(about = "RecordRoute - AI-powered transcription and summarization system", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the HTTP/WebSocket server
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to bind to
        #[arg(long, default_value = "8000")]
        port: u16,

        /// Database path
        #[arg(long)]
        db_path: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load environment variables from .env at project root
    // Note: AppConfig::from_env() also loads .env, but we do it here early
    // to ensure any CLI argument overrides work correctly
    load_dotenv_from_project_root();

    // Handle commands
    match cli.command {
        Some(Commands::Serve { host, port, db_path }) => {
            // Override with CLI arguments
            std::env::set_var("SERVER_HOST", &host);
            std::env::set_var("SERVER_PORT", &port.to_string());
            if let Some(db) = &db_path {
                std::env::set_var("DB_BASE_PATH", db);
            }

            // Load config with updated env vars
            let config = AppConfig::from_env()?;

            // Setup logging
            logger::setup_logging(&config.log_dir, &config.log_level)?;

            tracing::info!("RecordRoute starting...");
            tracing::info!("Configuration loaded:");
            tracing::info!("  Host: {}", host);
            tracing::info!("  Port: {}", port);
            tracing::info!("  Database: {}", config.db_base_path.display());

            // Print message that Electron expects
            println!("Server listening on http://{}:{}", host, port);

            // Start server
            recordroute_server::start_server(config).await?;
        }
        None => {
            // Default: start server with default config
            let config = AppConfig::from_env()?;
            logger::setup_logging(&config.log_dir, &config.log_level)?;

            tracing::info!("RecordRoute starting with default configuration...");

            // Print message that Electron expects
            let bind_addr = config.server_bind_address();
            println!("Server listening on http://{}", bind_addr);

            recordroute_server::start_server(config).await?;
        }
    }

    Ok(())
}
