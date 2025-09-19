//! Terminal API endpoints for ConPTY sessions.

use axum::{
    Json, Router,
    extract::{Path, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use winpe_agent_core::{ApiError, ErrorCode, SessionCreateRequest, SignalRequest};

use crate::terminal::SessionManager;

/// Create terminal router.
pub fn router(session_manager: SessionManager) -> Router {
    Router::new()
        .route("/sessions", post(create_session))
        .route("/sessions", get(list_sessions))
        .route("/sessions/:id", get(get_session))
        .route("/sessions/:id", delete(delete_session))
        .route("/sessions/:id/signal", post(send_signal))
        .route("/sessions/:id/ws", get(websocket_handler))
        .with_state(session_manager)
}

/// POST /api/v1/sessions
#[axum::debug_handler]
async fn create_session(
    State(manager): State<SessionManager>,
    Json(req): Json<SessionCreateRequest>,
) -> impl IntoResponse {
    match manager.create_session(req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(e) => {
            // Check if error is due to ConPTY unavailability
            if e.contains("ConPTY") || e.contains("CreatePseudoConsole") {
                (
                    StatusCode::NOT_IMPLEMENTED,
                    Json(ApiError::new(ErrorCode::NotSupported, e)),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new(ErrorCode::Internal, e)),
                )
                    .into_response()
            }
        }
    }
}

/// GET /api/v1/sessions
async fn list_sessions(State(manager): State<SessionManager>) -> impl IntoResponse {
    Json(manager.list_sessions())
}

/// GET /api/v1/sessions/{id}
async fn get_session(
    State(manager): State<SessionManager>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match manager.get_session(&id) {
        Some(info) => (StatusCode::OK, Json(info)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(ErrorCode::NotFound, "Session not found")),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/sessions/{id}
async fn delete_session(
    State(manager): State<SessionManager>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match manager.terminate_session(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(ErrorCode::NotFound, e)),
        )
            .into_response(),
    }
}

/// POST /api/v1/sessions/{id}/signal
async fn send_signal(
    State(manager): State<SessionManager>,
    Path(id): Path<String>,
    Json(req): Json<SignalRequest>,
) -> impl IntoResponse {
    match manager.send_signal(&id, req.signal).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(ErrorCode::NotFound, e)),
        )
            .into_response(),
    }
}

/// GET /api/v1/sessions/{id}/ws
async fn websocket_handler(
    State(manager): State<SessionManager>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Check if session exists
    if !manager.session_exists(&id) {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(ErrorCode::NotFound, "Session not found")),
        )
            .into_response();
    }

    ws.on_upgrade(move |socket| crate::terminal::ws::handle_websocket(socket, manager, id))
        .into_response()
}
