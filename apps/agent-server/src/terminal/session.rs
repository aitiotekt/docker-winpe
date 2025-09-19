//! Session management for ConPTY terminal sessions.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use ulid::Ulid;
use winpe_agent_core::{
    SessionCreateRequest, SessionCreateResponse, SessionInfo, SessionState, Shell, Signal,
};

#[cfg(windows)]
use super::conpty::spawn_process;

#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;

/// A Send-safe wrapper for Windows handles.
/// We store as usize for Send safety then convert back when needed.
#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
pub struct SendHandle(pub usize);

#[cfg(windows)]
impl SendHandle {
    pub fn from_handle(h: HANDLE) -> Self {
        Self(h as usize)
    }

    pub fn as_handle(&self) -> HANDLE {
        self.0 as HANDLE
    }
}

#[cfg(windows)]
unsafe impl Send for SendHandle {}
#[cfg(windows)]
unsafe impl Sync for SendHandle {}

/// Wrapper for ConPTY that is Send + Sync.
/// Stores handle as usize for Send safety.
#[cfg(windows)]
pub struct SendablePty {
    handle: usize,
}

#[cfg(windows)]
impl SendablePty {
    pub fn new(handle: windows_sys::Win32::System::Console::HPCON) -> Self {
        Self {
            handle: handle as usize,
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        use windows_sys::Win32::Foundation::S_OK;
        use windows_sys::Win32::System::Console::{COORD, HPCON, ResizePseudoConsole};
        unsafe {
            let size = COORD {
                X: cols as i16,
                Y: rows as i16,
            };
            let result = ResizePseudoConsole(self.handle as HPCON, size);
            if result != S_OK {
                return Err(format!("ResizePseudoConsole failed: 0x{:08X}", result));
            }
            Ok(())
        }
    }
}

#[cfg(windows)]
unsafe impl Send for SendablePty {}
#[cfg(windows)]
unsafe impl Sync for SendablePty {}

#[cfg(windows)]
impl Drop for SendablePty {
    fn drop(&mut self) {
        use windows_sys::Win32::System::Console::{ClosePseudoConsole, HPCON};
        unsafe {
            ClosePseudoConsole(self.handle as HPCON);
        }
    }
}

/// Session state stored in the manager.
pub struct Session {
    pub id: String,
    pub shell: Shell,
    pub pid: u32,
    pub state: SessionState,
    pub attached: bool,
    pub cols: u16,
    pub rows: u16,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    /// Idle timeout in seconds (from creation request).
    pub idle_timeout_sec: u64,

    #[cfg(windows)]
    pub process_handle: SendHandle,
    #[cfg(windows)]
    pub pty: Arc<SendablePty>,

    /// Channel to send input to the session.
    pub input_tx: mpsc::Sender<Vec<u8>>,
    /// Channel to receive output from the session.
    pub output_rx: Option<mpsc::Receiver<Vec<u8>>>,
}

// Ensure Session is Send + Sync
#[cfg(windows)]
unsafe impl Send for Session {}
#[cfg(windows)]
unsafe impl Sync for Session {}

/// Thread-safe session manager using DashMap.
#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, Arc<tokio::sync::RwLock<Session>>>>,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// Start a background task to clean up idle sessions.
    /// Should be called once when the server starts.
    pub fn start_cleanup_task(&self) {
        let sessions = self.sessions.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                Self::cleanup_idle_sessions_internal(&sessions).await;
            }
        });
    }

    /// Internal method to clean up idle sessions.
    async fn cleanup_idle_sessions_internal(
        sessions: &DashMap<String, Arc<tokio::sync::RwLock<Session>>>,
    ) {
        let now = Utc::now();
        let mut to_remove = Vec::new();

        for entry in sessions.iter() {
            let session = match entry.value().try_read() {
                Ok(s) => s,
                Err(_) => continue, // Skip if locked
            };

            // Only clean up detached sessions that have exceeded idle timeout
            if !session.attached {
                let idle_duration = now
                    .signed_duration_since(session.last_activity)
                    .num_seconds();
                if idle_duration > session.idle_timeout_sec as i64 {
                    to_remove.push(entry.key().clone());
                    tracing::info!(
                        "Session {} idle for {}s (timeout: {}s), will be terminated",
                        session.id,
                        idle_duration,
                        session.idle_timeout_sec
                    );
                }
            }
        }

        // Remove and terminate idle sessions
        for id in to_remove {
            if let Some((_, session)) = sessions.remove(&id) {
                #[cfg(windows)]
                {
                    use windows_sys::Win32::Foundation::CloseHandle;
                    use windows_sys::Win32::System::Threading::TerminateProcess;

                    if let Ok(session) = session.try_write() {
                        unsafe {
                            TerminateProcess(session.process_handle.as_handle(), 1);
                            CloseHandle(session.process_handle.as_handle());
                        }
                    }
                }
                tracing::info!("Terminated idle session {}", id);
            }
        }
    }

    /// Create a new terminal session.
    #[cfg(windows)]
    pub async fn create_session(
        &self,
        req: SessionCreateRequest,
    ) -> Result<SessionCreateResponse, String> {
        use std::io::Write;
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, S_OK};
        use windows_sys::Win32::System::Console::{COORD, CreatePseudoConsole, HPCON};
        use windows_sys::Win32::System::Pipes::CreatePipe;

        let id = Ulid::new().to_string();
        let now = Utc::now();

        // Create pipes for ConPTY I/O
        let (pty_handle, input_write, output_read): (HPCON, HANDLE, HANDLE) = unsafe {
            let mut pty_input_read: HANDLE = INVALID_HANDLE_VALUE;
            let mut pty_input_write: HANDLE = INVALID_HANDLE_VALUE;
            let mut pty_output_read: HANDLE = INVALID_HANDLE_VALUE;
            let mut pty_output_write: HANDLE = INVALID_HANDLE_VALUE;

            if CreatePipe(
                &mut pty_input_read,
                &mut pty_input_write,
                std::ptr::null_mut(),
                0,
            ) == 0
            {
                return Err("Failed to create input pipe".to_string());
            }

            if CreatePipe(
                &mut pty_output_read,
                &mut pty_output_write,
                std::ptr::null_mut(),
                0,
            ) == 0
            {
                CloseHandle(pty_input_read);
                CloseHandle(pty_input_write);
                return Err("Failed to create output pipe".to_string());
            }

            let size = COORD {
                X: req.cols as i16,
                Y: req.rows as i16,
            };

            let mut hpc: HPCON = 0;
            let result = CreatePseudoConsole(size, pty_input_read, pty_output_write, 0, &mut hpc);

            if result != S_OK {
                CloseHandle(pty_input_read);
                CloseHandle(pty_input_write);
                CloseHandle(pty_output_read);
                CloseHandle(pty_output_write);
                return Err(format!("CreatePseudoConsole failed: 0x{:08X}", result));
            }

            // Close the handles that the ConPTY now owns
            CloseHandle(pty_input_read);
            CloseHandle(pty_output_write);

            (hpc, pty_input_write, pty_output_read)
        };

        let pty = Arc::new(SendablePty::new(pty_handle));

        // Build command line
        let command_line = match req.shell {
            Shell::Cmd => "cmd.exe".to_string(),
            Shell::Powershell => "powershell.exe -NoLogo -NoProfile".to_string(),
        };

        // Spawn process attached to ConPTY
        let (process_handle_raw, pid) =
            spawn_process(pty_handle, &command_line, req.cwd.as_deref())?;
        // Immediately wrap in SendHandle to satisfy Send requirement
        let process_handle = SendHandle::from_handle(process_handle_raw);

        // Create channels for I/O
        let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(100);
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(100);

        // Wrap handles for Send (convert to usize)
        let input_handle = SendHandle::from_handle(input_write);
        let output_handle = SendHandle::from_handle(output_read);

        // Spawn input writer task using std::thread (not tokio) for raw handles
        std::thread::spawn(move || {
            let mut file = unsafe { std::fs::File::from_raw_handle(input_handle.as_handle()) };
            while let Some(data) = input_rx.blocking_recv() {
                if file.write_all(&data).is_err() {
                    break;
                }
                let _ = file.flush();
            }
        });

        // Spawn output reader task using std::thread
        std::thread::spawn(move || {
            use std::io::Read;
            let mut file = unsafe { std::fs::File::from_raw_handle(output_handle.as_handle()) };
            let mut buffer = [0u8; 4096];
            loop {
                match file.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        if output_tx.blocking_send(buffer[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Send UTF-8 initialization if requested
        if req.init.force_utf8 {
            let init_cmd = match req.shell {
                Shell::Cmd => "chcp 65001\r\n".as_bytes().to_vec(),
                Shell::Powershell => {
                    "[Console]::InputEncoding=[Text.UTF8Encoding]::UTF8;[Console]::OutputEncoding=[Text.UTF8Encoding]::UTF8\r\n"
                        .as_bytes()
                        .to_vec()
                }
            };
            let _ = input_tx.send(init_cmd).await;
        }

        let session = Session {
            id: id.clone(),
            shell: req.shell,
            pid,
            state: SessionState::Running,
            attached: false,
            cols: req.cols,
            rows: req.rows,
            created_at: now,
            last_activity: now,
            idle_timeout_sec: req.idle_timeout_sec,
            process_handle,
            pty,
            input_tx,
            output_rx: Some(output_rx),
        };

        self.sessions
            .insert(id.clone(), Arc::new(tokio::sync::RwLock::new(session)));

        Ok(SessionCreateResponse {
            id: id.clone(),
            ws_url: format!("/api/v1/sessions/{}/ws", id),
            created_at: now.to_rfc3339(),
        })
    }

    #[cfg(not(windows))]
    pub async fn create_session(
        &self,
        _req: SessionCreateRequest,
    ) -> Result<SessionCreateResponse, String> {
        Err("ConPTY is only available on Windows".to_string())
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        let mut result = Vec::new();
        for entry in self.sessions.iter() {
            if let Ok(session) = entry.value().try_read() {
                result.push(SessionInfo {
                    id: session.id.clone(),
                    shell: session.shell,
                    pid: session.pid,
                    state: session.state,
                    attached: session.attached,
                    cols: session.cols,
                    rows: session.rows,
                    created_at: session.created_at.to_rfc3339(),
                    last_activity_at: session.last_activity.to_rfc3339(),
                });
            }
        }
        result
    }

    /// Get session info by ID.
    pub fn get_session(&self, id: &str) -> Option<SessionInfo> {
        self.sessions.get(id).and_then(|entry| {
            entry.try_read().ok().map(|session| SessionInfo {
                id: session.id.clone(),
                shell: session.shell,
                pid: session.pid,
                state: session.state,
                attached: session.attached,
                cols: session.cols,
                rows: session.rows,
                created_at: session.created_at.to_rfc3339(),
                last_activity_at: session.last_activity.to_rfc3339(),
            })
        })
    }

    /// Check if a session exists.
    pub fn session_exists(&self, id: &str) -> bool {
        self.sessions.contains_key(id)
    }

    /// Get session for WebSocket attachment.
    pub fn get_session_for_ws(&self, id: &str) -> Option<Arc<tokio::sync::RwLock<Session>>> {
        self.sessions.get(id).map(|entry| entry.value().clone())
    }

    /// Terminate a session.
    #[cfg(windows)]
    pub async fn terminate_session(&self, id: &str) -> Result<(), String> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::TerminateProcess;

        let session = self
            .sessions
            .remove(id)
            .map(|(_, v)| v)
            .ok_or_else(|| "Session not found".to_string())?;

        let session = session.write().await;
        unsafe {
            TerminateProcess(session.process_handle.as_handle(), 1);
            CloseHandle(session.process_handle.as_handle());
        }

        Ok(())
    }

    #[cfg(not(windows))]
    pub async fn terminate_session(&self, id: &str) -> Result<(), String> {
        self.sessions
            .remove(id)
            .ok_or_else(|| "Session not found".to_string())?;
        Ok(())
    }

    /// Send a signal to a session.
    #[cfg(windows)]
    pub async fn send_signal(&self, id: &str, signal: Signal) -> Result<(), String> {
        let session = self
            .sessions
            .get(id)
            .ok_or_else(|| "Session not found".to_string())?;

        let session = session.read().await;

        match signal {
            Signal::CtrlC => {
                // Send Ctrl+C byte (0x03)
                let _ = session.input_tx.send(vec![0x03]).await;
            }
            Signal::CtrlBreak => {
                // Send Ctrl+Break (0x03 works similarly)
                let _ = session.input_tx.send(vec![0x03]).await;
            }
            Signal::Terminate => {
                use windows_sys::Win32::System::Threading::TerminateProcess;
                unsafe {
                    TerminateProcess(session.process_handle.as_handle(), 1);
                }
            }
        }

        Ok(())
    }

    #[cfg(not(windows))]
    pub async fn send_signal(&self, id: &str, _signal: Signal) -> Result<(), String> {
        if !self.sessions.contains_key(id) {
            return Err("Session not found".to_string());
        }
        Err("Signals not supported on this platform".to_string())
    }

    /// Resize a session's terminal.
    #[cfg(windows)]
    pub async fn resize_session(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let session = self
            .sessions
            .get(id)
            .ok_or_else(|| "Session not found".to_string())?;

        let mut session = session.write().await;
        session.pty.resize(cols, rows)?;
        session.cols = cols;
        session.rows = rows;

        Ok(())
    }

    #[cfg(not(windows))]
    pub async fn resize_session(&self, id: &str, _cols: u16, _rows: u16) -> Result<(), String> {
        if !self.sessions.contains_key(id) {
            return Err("Session not found".to_string());
        }
        Err("Resize not supported on this platform".to_string())
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
