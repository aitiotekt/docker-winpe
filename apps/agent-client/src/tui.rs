//! TUI mode: Interactive terminal client using crossterm and raw mode.
//!
//! ## Why not tui-term?
//!
//! While the documentation mentions tui-term as a potential option, we chose to use
//! crossterm + raw terminal mode directly for the following reasons:
//!
//! 1. **Simpler architecture**: We don't need a local terminal emulator buffer since
//!    the server-side ConPTY handles all terminal emulation. We just pass through
//!    raw bytes between stdin/stdout and the WebSocket.
//!
//! 2. **Lower latency**: Direct I/O without intermediate buffering means faster response.
//!
//! 3. **Smaller dependency footprint**: tui-term + ratatui would add significant dependencies
//!    for features we don't need (widgets, layouts, etc.).
//!
//! 4. **Better compatibility**: Raw mode passthrough works with any terminal application
//!    on the server (vim, htop, etc.) without needing to parse or interpret escape sequences.
//!
//! If you need features like local scrollback buffer, split panes, or session tabs,
//! consider migrating to a full tui-term + ratatui implementation.

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::io::{self, Write};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use winpe_agent_core::{SessionCreateRequest, SessionCreateResponse, Shell};

pub async fn run(
    base_url: &str,
    token: Option<&str>,
    shell: &str,
    cols: u16,
    rows: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let shell_enum = match shell.to_lowercase().as_str() {
        "cmd" => Shell::Cmd,
        "powershell" | "pwsh" => Shell::Powershell,
        _ => return Err(format!("Unknown shell: {}", shell).into()),
    };

    // Create session
    let client = reqwest::Client::new();
    let req = SessionCreateRequest {
        shell: shell_enum,
        cwd: None,
        env: HashMap::new(),
        cols,
        rows,
        idle_timeout_sec: 600,
        init: winpe_agent_core::SessionInit { force_utf8: true },
    };

    let mut request = client
        .post(format!("{}/api/v1/sessions", base_url))
        .json(&req);
    if let Some(t) = token {
        request = request.bearer_auth(t);
    }

    let response = request.send().await?;
    if !response.status().is_success() {
        let body = response.text().await?;
        return Err(format!("Failed to create session: {}", body).into());
    }

    let session: SessionCreateResponse = response.json().await?;
    eprintln!("Session created: {}", session.id);

    // Connect WebSocket
    let ws_url = base_url
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let full_ws_url = format!("{}{}", ws_url, session.ws_url);

    let (ws_stream, _) = connect_async(&full_ws_url).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Enable raw mode
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    // Spawn task to read from WebSocket and print to terminal
    let output_handle = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    let mut stdout = io::stdout();
                    let _ = stdout.write_all(&data);
                    let _ = stdout.flush();
                }
                Ok(Message::Text(text)) => {
                    // Control messages (pong, etc.)
                    eprintln!("\r\n[Server]: {}", text);
                }
                Ok(Message::Close(_)) => {
                    break;
                }
                Err(e) => {
                    eprintln!("\r\n[WebSocket error]: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Main input loop
    loop {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => {
                    // Send Ctrl+C
                    let signal = serde_json::json!({"type": "signal", "name": "ctrl_c"});
                    ws_sender
                        .send(Message::Text(signal.to_string().into()))
                        .await?;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('d'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => {
                    // Exit on Ctrl+D
                    break;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('z'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => {
                    // Send Ctrl+Z (suspend)
                    ws_sender.send(Message::Binary(vec![0x1a].into())).await?;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('l'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => {
                    // Send Ctrl+L (clear screen)
                    ws_sender.send(Message::Binary(vec![0x0c].into())).await?;
                }
                Event::Key(KeyEvent {
                    code, modifiers, ..
                }) => {
                    // Handle Ctrl+key combinations
                    if modifiers.contains(KeyModifiers::CONTROL)
                        && let KeyCode::Char(c) = code
                    {
                        // Ctrl+A = 1, Ctrl+B = 2, etc.
                        let ctrl_byte = (c.to_ascii_lowercase() as u8).wrapping_sub(b'a' - 1);
                        if ctrl_byte <= 26 {
                            ws_sender
                                .send(Message::Binary(vec![ctrl_byte].into()))
                                .await?;
                            continue;
                        }
                    }

                    // Convert key to ANSI escape sequences
                    let bytes = keycode_to_bytes(code);
                    if !bytes.is_empty() {
                        ws_sender.send(Message::Binary(bytes.into())).await?;
                    }
                }
                Event::Resize(new_cols, new_rows) => {
                    // Send resize message
                    let resize = serde_json::json!({
                        "type": "resize",
                        "cols": new_cols,
                        "rows": new_rows
                    });
                    ws_sender
                        .send(Message::Text(resize.to_string().into()))
                        .await?;
                }
                _ => {}
            }
        }
    }

    // Cleanup
    output_handle.abort();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    eprintln!("Session ended");
    Ok(())
}

/// Convert a KeyCode to the corresponding ANSI escape sequence bytes.
fn keycode_to_bytes(code: KeyCode) -> Vec<u8> {
    match code {
        // Printable characters
        KeyCode::Char(c) => c.to_string().into_bytes(),

        // Basic control keys
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f], // DEL character
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],

        // Arrow keys (CSI sequences)
        KeyCode::Up => vec![0x1b, b'[', b'A'],
        KeyCode::Down => vec![0x1b, b'[', b'B'],
        KeyCode::Right => vec![0x1b, b'[', b'C'],
        KeyCode::Left => vec![0x1b, b'[', b'D'],

        // Navigation keys (CSI sequences with tilde)
        KeyCode::Home => vec![0x1b, b'[', b'H'], // or \x1b[1~
        KeyCode::End => vec![0x1b, b'[', b'F'],  // or \x1b[4~
        KeyCode::PageUp => vec![0x1b, b'[', b'5', b'~'],
        KeyCode::PageDown => vec![0x1b, b'[', b'6', b'~'],
        KeyCode::Insert => vec![0x1b, b'[', b'2', b'~'],
        KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],

        // Function keys (CSI sequences)
        KeyCode::F(1) => vec![0x1b, b'O', b'P'],
        KeyCode::F(2) => vec![0x1b, b'O', b'Q'],
        KeyCode::F(3) => vec![0x1b, b'O', b'R'],
        KeyCode::F(4) => vec![0x1b, b'O', b'S'],
        KeyCode::F(5) => vec![0x1b, b'[', b'1', b'5', b'~'],
        KeyCode::F(6) => vec![0x1b, b'[', b'1', b'7', b'~'],
        KeyCode::F(7) => vec![0x1b, b'[', b'1', b'8', b'~'],
        KeyCode::F(8) => vec![0x1b, b'[', b'1', b'9', b'~'],
        KeyCode::F(9) => vec![0x1b, b'[', b'2', b'0', b'~'],
        KeyCode::F(10) => vec![0x1b, b'[', b'2', b'1', b'~'],
        KeyCode::F(11) => vec![0x1b, b'[', b'2', b'3', b'~'],
        KeyCode::F(12) => vec![0x1b, b'[', b'2', b'4', b'~'],

        // Other function keys (F13-F24 are less common but supported)
        KeyCode::F(n) if (13..=24).contains(&n) => {
            // F13=25~, F14=26~, etc.
            let num = 24 + (n - 12);
            format!("\x1b[{}~", num).into_bytes()
        }

        // Null/unhandled
        KeyCode::Null => vec![0x00],
        KeyCode::CapsLock | KeyCode::ScrollLock | KeyCode::NumLock => vec![],
        KeyCode::PrintScreen | KeyCode::Pause | KeyCode::Menu => vec![],
        KeyCode::KeypadBegin => vec![],
        KeyCode::Media(_) | KeyCode::Modifier(_) => vec![],

        // Catch-all for any other keys
        _ => vec![],
    }
}
