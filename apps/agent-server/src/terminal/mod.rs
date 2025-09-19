//! Terminal module for ConPTY-backed interactive sessions.

mod conpty;
mod session;
pub mod ws;

pub use session::SessionManager;
