//! API route handlers.

mod automation;
mod health;
mod terminal;

use crate::terminal::SessionManager;
use axum::Router;

/// Create the API router with all endpoints.
pub fn router(session_manager: SessionManager) -> Router {
    Router::new()
        .merge(health::router())
        .merge(automation::router())
        .merge(terminal::router(session_manager))
}
