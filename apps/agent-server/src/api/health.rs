//! Health check endpoint.

use axum::{Json, Router, routing::get};
use winpe_agent_core::{Capabilities, HealthResponse, VERSION};

/// Create health router.
pub fn router() -> Router {
    Router::new().route("/health", get(health_handler))
}

/// GET /api/v1/health
async fn health_handler() -> Json<HealthResponse> {
    // Probe ConPTY availability at runtime
    let conpty_available = probe_conpty();

    Json(HealthResponse {
        status: "ok".to_string(),
        version: VERSION.to_string(),
        capabilities: Capabilities {
            conpty: conpty_available,
            automation: true,
            terminal: conpty_available,
        },
    })
}

/// Probe whether ConPTY APIs are available.
#[cfg(windows)]
fn probe_conpty() -> bool {
    use windows_sys::Win32::System::Console::CreatePseudoConsole;
    // If we can reference the function, ConPTY is available
    // The function exists on Windows 10 1809+ and Windows 11
    let _ = CreatePseudoConsole as *const () as usize;
    true
}

#[cfg(not(windows))]
fn probe_conpty() -> bool {
    false
}
