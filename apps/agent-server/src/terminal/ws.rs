//! WebSocket handler for terminal sessions.
//!
//! WebSocket close codes used:
//! - 1000: Normal closure
//! - 1008: Policy violation (e.g., session already attached)
//! - 1011: Unexpected condition (e.g., session not found)

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures::{SinkExt, StreamExt};
use winpe_agent_core::WsControlMessage;

use super::SessionManager;

/// Handle a WebSocket connection for a terminal session.
pub async fn handle_websocket(socket: WebSocket, manager: SessionManager, session_id: String) {
    let session = match manager.get_session_for_ws(&session_id) {
        Some(s) => s,
        None => {
            tracing::error!("Session {} not found", session_id);
            // Send close with 1011 (unexpected condition)
            let (mut sender, _) = socket.split();
            let _ = sender
                .send(Message::Close(Some(CloseFrame {
                    code: 1011,
                    reason: "Session not found".into(),
                })))
                .await;
            return;
        }
    };

    // Check if session is already attached - reject second connection
    {
        let session_guard = session.read().await;
        if session_guard.attached {
            tracing::warn!(
                "Session {} already has a client attached, rejecting",
                session_id
            );
            // Send close with 1008 (policy violation)
            let (mut sender, _) = socket.split();
            let _ = sender
                .send(Message::Close(Some(CloseFrame {
                    code: 1008,
                    reason: "Session already attached".into(),
                })))
                .await;
            return;
        }
    }

    // Mark session as attached
    {
        let mut session_guard = session.write().await;
        session_guard.attached = true;
        session_guard.last_activity = chrono::Utc::now();
    }

    // Subscribe to the output broadcast channel - allows reconnection
    let mut output_rx = {
        let session_guard = session.read().await;
        session_guard.output_tx.subscribe()
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Get input sender
    let input_tx = {
        let session_guard = session.read().await;
        session_guard.input_tx.clone()
    };

    let manager_clone = manager.clone();
    let session_id_clone = session_id.clone();
    let session_clone = session.clone();

    // Spawn task to forward output to WebSocket
    let output_task = tokio::spawn(async move {
        loop {
            match output_rx.recv().await {
                Ok(data) => {
                    if ws_sender.send(Message::Binary(data.into())).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Subscriber lagged behind, continue receiving
                    continue;
                }
            }
        }
        // Send normal close when output ends
        let _ = ws_sender
            .send(Message::Close(Some(CloseFrame {
                code: 1000,
                reason: "Session ended".into(),
            })))
            .await;
    });

    // Handle incoming WebSocket messages
    while let Some(msg) = ws_receiver.next().await {
        // Update last activity
        {
            let mut session_guard = session_clone.write().await;
            session_guard.last_activity = chrono::Utc::now();
        }

        match msg {
            Ok(Message::Binary(data)) => {
                // Raw terminal input
                if input_tx.send(data.to_vec()).await.is_err() {
                    break;
                }
            }
            Ok(Message::Text(text)) => {
                // JSON control message
                match serde_json::from_str::<WsControlMessage>(&text) {
                    Ok(WsControlMessage::Resize { cols, rows }) => {
                        if let Err(e) = manager_clone
                            .resize_session(&session_id_clone, cols, rows)
                            .await
                        {
                            tracing::warn!("Failed to resize session: {}", e);
                        }
                    }
                    Ok(WsControlMessage::Signal { name }) => {
                        let signal = match name.as_str() {
                            "ctrl_c" => Some(winpe_agent_core::Signal::CtrlC),
                            "ctrl_break" => Some(winpe_agent_core::Signal::CtrlBreak),
                            "terminate" => Some(winpe_agent_core::Signal::Terminate),
                            _ => None,
                        };
                        if let Some(sig) = signal
                            && let Err(e) = manager_clone.send_signal(&session_id_clone, sig).await
                        {
                            tracing::warn!("Failed to send signal: {}", e);
                        }
                    }
                    Ok(WsControlMessage::Ping { t: _ }) => {
                        // TODO: Implement pong response (requires refactoring to share ws_sender)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse control message: {}", e);
                    }
                }
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Err(e) => {
                tracing::warn!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Clean up
    output_task.abort();

    // Mark session as detached
    {
        let mut session_guard = session.write().await;
        session_guard.attached = false;
    }

    tracing::info!("WebSocket disconnected for session {}", session_id);
}
