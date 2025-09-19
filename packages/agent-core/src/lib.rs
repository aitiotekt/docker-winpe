//! winpe-agent-core: Shared types and utilities for WinPE Agent.
//!
//! This crate provides common data structures used by both
//! `winpe-agent-server` and `winpe-agent-client`.

pub mod types;

pub use types::*;

/// API version string.
pub const API_VERSION: &str = "v1";

/// Default server port.
pub const DEFAULT_PORT: u16 = 8080;

/// Server version (from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
