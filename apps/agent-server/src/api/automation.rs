//! Automation API endpoints for single command execution.

use axum::{
    Json, Router,
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
    routing::post,
};
use futures::stream::Stream;
use std::convert::Infallible;
use std::time::Instant;
use winpe_agent_core::{ApiError, ErrorCode, ExecRequest, ExecResponse};

use crate::automation::executor;

/// Create automation router.
pub fn router() -> Router {
    Router::new()
        .route("/automation/exec", post(exec_handler))
        .route("/automation/exec_stream", post(exec_stream_handler))
}

/// POST /api/v1/automation/exec
#[axum::debug_handler]
async fn exec_handler(Json(req): Json<ExecRequest>) -> impl IntoResponse {
    let start = Instant::now();

    match executor::execute_command(&req).await {
        Ok((exit_code, stdout, stderr)) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            (
                StatusCode::OK,
                Json(ExecResponse {
                    exit_code,
                    stdout,
                    stderr,
                    duration_ms,
                }),
            )
                .into_response()
        }
        Err(e) => match e {
            executor::ExecError::Timeout => {
                let mut details = std::collections::HashMap::new();
                details.insert(
                    "timeout_ms".to_string(),
                    serde_json::Value::Number(req.timeout_ms.into()),
                );
                (
                    StatusCode::REQUEST_TIMEOUT,
                    Json(ApiError::with_details(
                        ErrorCode::Timeout,
                        "Process exceeded timeout",
                        details,
                    )),
                )
                    .into_response()
            }
            executor::ExecError::ProcessCreationFailed(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(ErrorCode::Internal, msg)),
            )
                .into_response(),
            executor::ExecError::NotSupported(msg) => (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(ErrorCode::NotSupported, msg)),
            )
                .into_response(),
        },
    }
}

/// POST /api/v1/automation/exec_stream
async fn exec_stream_handler(
    Json(req): Json<ExecRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let start = Instant::now();

    let stream = async_stream::stream! {
        match executor::execute_command_stream(&req).await {
            Ok(mut rx) => {
                while let Some(event) = rx.recv().await {
                    match event {
                        executor::StreamEvent::Stdout(chunk) => {
                            let data = serde_json::json!({ "chunk": chunk });
                            yield Ok(Event::default().event("stdout").data(data.to_string()));
                        }
                        executor::StreamEvent::Stderr(chunk) => {
                            let data = serde_json::json!({ "chunk": chunk });
                            yield Ok(Event::default().event("stderr").data(data.to_string()));
                        }
                        executor::StreamEvent::Exit(exit_code) => {
                            let duration_ms = start.elapsed().as_millis() as u64;
                            let data = serde_json::json!({
                                "exit_code": exit_code,
                                "duration_ms": duration_ms
                            });
                            yield Ok(Event::default().event("exit").data(data.to_string()));
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                let error_msg = match e {
                    executor::ExecError::Timeout => "Process exceeded timeout",
                    executor::ExecError::ProcessCreationFailed(ref msg) => msg.as_str(),
                    executor::ExecError::NotSupported(ref msg) => msg.as_str(),
                };
                let data = serde_json::json!({ "error": error_msg });
                yield Ok(Event::default().event("error").data(data.to_string()));
            }
        }
    };

    Sse::new(stream)
}
