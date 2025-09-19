//! winpe-agent-client: CLI client for WinPE Agent.
//!
//! Supports three modes:
//! - `exec`: Execute a single command
//! - `tui`: Interactive TUI terminal
//! - `web`: Open browser to web UI

mod exec;
mod tui;
mod web;

use clap::{Parser, Subcommand};

/// WinPE Agent CLI Client
#[derive(Parser)]
#[command(name = "winpe-agent-client")]
#[command(version, about, long_about = None)]
struct Cli {
    /// Server base URL
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    url: String,

    /// Optional bearer token for authentication
    #[arg(long)]
    token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a single command via Automation API
    Exec {
        /// Shell to use (cmd or powershell)
        #[arg(long, default_value = "cmd")]
        shell: String,

        /// Working directory
        #[arg(long)]
        cwd: Option<String>,

        /// Timeout in milliseconds
        #[arg(long, default_value = "600000")]
        timeout: u64,

        /// Output in JSON format
        #[arg(long)]
        json: bool,

        /// Command and arguments
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Open interactive TUI terminal
    Tui {
        /// Shell to use (cmd or powershell)
        #[arg(long, default_value = "cmd")]
        shell: String,

        /// Terminal columns
        #[arg(long, default_value = "120")]
        cols: u16,

        /// Terminal rows
        #[arg(long, default_value = "30")]
        rows: u16,
    },

    /// Open browser to web UI
    Web,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Exec {
            shell,
            cwd,
            timeout,
            json,
            command,
        } => {
            exec::run(
                &cli.url,
                cli.token.as_deref(),
                &shell,
                cwd.as_deref(),
                timeout,
                json,
                &command,
            )
            .await
        }
        Commands::Tui { shell, cols, rows } => {
            tui::run(&cli.url, cli.token.as_deref(), &shell, cols, rows).await
        }
        Commands::Web => web::run(&cli.url),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
