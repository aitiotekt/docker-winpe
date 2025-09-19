//! winpe-agent-server: HTTP/WebSocket server for WinPE Agent.
//!
//! Provides:
//! - Automation API: Execute single commands
//! - Terminal API: ConPTY-backed interactive sessions
//! - Static UI: xterm.js web interface

mod api;
mod automation;
mod terminal;

use axum::Router;
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "winpe_agent_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting winpe-agent-server v{}", winpe_agent_core::VERSION);

    // Initialize session manager
    let session_manager = terminal::SessionManager::new();

    // Start background task to clean up idle sessions
    session_manager.start_cleanup_task();

    // Build the router
    let app = Router::new()
        .nest("/api/v1", api::router(session_manager.clone()))
        .nest_service(
            "/ui",
            ServeDir::new("ui").append_index_html_on_directories(true),
        )
        .layer(TraceLayer::new_for_http());

    // Bind to all interfaces on port 8080
    let addr = SocketAddr::from(([0, 0, 0, 0], winpe_agent_core::DEFAULT_PORT));
    tracing::info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
