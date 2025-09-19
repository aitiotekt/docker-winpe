//! Shared types for winpe-agent API.
//!
//! This module defines all request/response structures used by both
//! the agent server and client.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Health API
// ============================================================================

/// Server capabilities reported by health endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    /// Whether ConPTY is available for terminal sessions.
    pub conpty: bool,
    /// Whether automation API is available.
    pub automation: bool,
    /// Whether terminal API is available.
    pub terminal: bool,
}

/// Response from `GET /api/v1/health`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Server status, typically "ok".
    pub status: String,
    /// Server version string.
    pub version: String,
    /// Available capabilities.
    pub capabilities: Capabilities,
}

// ============================================================================
// Automation API
// ============================================================================

/// Shell type for command execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    #[default]
    Cmd,
    Powershell,
}

/// Request body for `POST /api/v1/automation/exec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecRequest {
    /// Shell to use for execution.
    #[serde(default)]
    pub shell: Shell,
    /// Command to execute.
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Environment variables to set.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Timeout in milliseconds (server-enforced).
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Output encoding hint (default: utf-8).
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

fn default_timeout() -> u64 {
    600_000 // 10 minutes
}

fn default_encoding() -> String {
    "utf-8".to_string()
}

/// Response from `POST /api/v1/automation/exec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResponse {
    /// Process exit code.
    pub exit_code: i32,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
}

/// SSE event types for streaming execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum ExecStreamEvent {
    /// Stdout chunk.
    #[serde(rename = "stdout")]
    Stdout { chunk: String },
    /// Stderr chunk.
    #[serde(rename = "stderr")]
    Stderr { chunk: String },
    /// Process exited.
    #[serde(rename = "exit")]
    Exit { exit_code: i32, duration_ms: u64 },
}

// ============================================================================
// Terminal API (Sessions)
// ============================================================================

/// Session initialization options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionInit {
    /// Force UTF-8 mode (chcp 65001 for cmd, encoding commands for PowerShell).
    #[serde(default)]
    pub force_utf8: bool,
}

/// Request body for `POST /api/v1/sessions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreateRequest {
    /// Shell to spawn.
    #[serde(default)]
    pub shell: Shell,
    /// Working directory.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Terminal columns.
    #[serde(default = "default_cols")]
    pub cols: u16,
    /// Terminal rows.
    #[serde(default = "default_rows")]
    pub rows: u16,
    /// Idle timeout in seconds before auto-termination.
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_sec: u64,
    /// Initialization options.
    #[serde(default)]
    pub init: SessionInit,
}

fn default_cols() -> u16 {
    120
}

fn default_rows() -> u16 {
    30
}

fn default_idle_timeout() -> u64 {
    600 // 10 minutes
}

/// Response from `POST /api/v1/sessions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreateResponse {
    /// Session ID (ULID format).
    pub id: String,
    /// WebSocket URL path for this session.
    pub ws_url: String,
    /// Creation timestamp (ISO 8601).
    pub created_at: String,
}

/// Session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Running,
    Exited,
}

/// Session info returned by list/get endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session ID.
    pub id: String,
    /// Shell type.
    pub shell: Shell,
    /// Process ID of the shell.
    pub pid: u32,
    /// Current state.
    pub state: SessionState,
    /// Whether a client is currently attached via WebSocket.
    pub attached: bool,
    /// Terminal columns.
    pub cols: u16,
    /// Terminal rows.
    pub rows: u16,
    /// Creation timestamp.
    pub created_at: String,
    /// Last activity timestamp.
    pub last_activity_at: String,
}

/// Signal types for terminal sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    CtrlC,
    CtrlBreak,
    Terminate,
}

/// Request body for `POST /api/v1/sessions/{id}/signal`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalRequest {
    pub signal: Signal,
}

// ============================================================================
// WebSocket Protocol
// ============================================================================

/// Control messages sent over WebSocket text frames (client -> server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsControlMessage {
    /// Resize the terminal.
    #[serde(rename = "resize")]
    Resize { cols: u16, rows: u16 },
    /// Send a signal.
    #[serde(rename = "signal")]
    Signal { name: String },
    /// Ping for keepalive.
    #[serde(rename = "ping")]
    Ping { t: u64 },
}

/// Control messages sent over WebSocket text frames (server -> client).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsServerMessage {
    /// Pong response to ping.
    #[serde(rename = "pong")]
    Pong { t: u64 },
}

// ============================================================================
// Error Types
// ============================================================================

/// Error codes for API responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    BadRequest,
    NotFound,
    Timeout,
    Internal,
    NotSupported,
}

/// Error details for API responses.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorDetails {
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Unified error response structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: ApiErrorInner,
}

/// Inner error structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorInner {
    /// Error code.
    pub code: ErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Additional details.
    #[serde(default, skip_serializing_if = "is_empty_details")]
    pub details: ErrorDetails,
}

fn is_empty_details(details: &ErrorDetails) -> bool {
    details.extra.is_empty()
}

impl ApiError {
    /// Create a new API error.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            error: ApiErrorInner {
                code,
                message: message.into(),
                details: ErrorDetails::default(),
            },
        }
    }

    /// Create an error with additional details.
    pub fn with_details(
        code: ErrorCode,
        message: impl Into<String>,
        details: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            error: ApiErrorInner {
                code,
                message: message.into(),
                details: ErrorDetails { extra: details },
            },
        }
    }
}
