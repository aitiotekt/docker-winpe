# Server implementation notes (Rust + Axum + ConPTY)

This document provides implementer-level notes for `winpe-agent-server`.

## Crate layout (suggested)

```
winpe-agent-server/
  src/
    main.rs
    api/
      mod.rs
      health.rs
      automation.rs
      terminal.rs
    terminal/
      mod.rs
      conpty.rs
      session.rs
      ws.rs
    static_ui/
      mod.rs
  ui/
    index.html
    app.js
    xterm.js (bundled)
    ...
```

## Axum routing

- `/api/v1/health` -> health handler
- `/api/v1/automation/exec` -> single command
- `/api/v1/sessions` -> session mgmt
- `/api/v1/sessions/:id/ws` -> WebSocket
- `/ui/*path` -> static file service

Use `TowerHttp`:
- `tower_http::services::ServeDir` to serve `ui/`.
- `TraceLayer` for request logging.

## ConPTY module

### Capability probe

At startup, probe ConPTY functions:
- `CreatePseudoConsole`
- `ResizePseudoConsole`
- `ClosePseudoConsole`

If not available, terminal endpoints must return `NOT_SUPPORTED`.

### Process creation strategy

Use `STARTUPINFOEXW` and `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`:

1. Create pipes for ConPTY input/output.
2. `CreatePseudoConsole` with initial size.
3. Initialize attribute list and set pseudo console attribute.
4. `CreateProcessW` for:
   - cmd: `X:\Windows\System32\cmd.exe`
   - powershell: `X:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe` (PowerShell 5)

### Terminating process trees

Prefer Job Objects:
- Create a job per session.
- Assign process to job.
- On terminate, close job handle or call `TerminateJobObject`.

### I/O loop

- Reading from ConPTY output is blocking; use `tokio::task::spawn_blocking`.
- Writing input is blocking; either use spawn_blocking or a dedicated thread.
- Maintain per-session `mpsc` channels:
  - `ws_in_tx` (bytes -> writer)
  - `ws_out_tx` (bytes -> websocket)

### UTF-8 init

If `force_utf8` is enabled:
- cmd: write `chcp 65001\r\n`
- PowerShell: write `[Console]::InputEncoding=[Text.UTF8Encoding]::UTF8;[Console]::OutputEncoding=[Text.UTF8Encoding]::UTF8\r\n`

Send these bytes immediately after session start.

## Automation execution

- Use `CreateProcessW` with redirected pipes.
- Capture stdout/stderr with bounded buffers.
- Enforce timeout.
- Prefer Job Objects to ensure child trees do not leak.

## Session manager

- Use `DashMap<SessionId, SessionHandle>`.
- Session states: `Running`, `Exited`.
- Attachment: boolean or `Option<AttachedClientId>`.
- Cleanup:
  - if detached and idle > timeout: terminate
  - if exited: remove

## Error handling

Implement a unified error type that maps to:

```
{ "error": { "code": "...", "message": "...", "details": {...} } }
```

## Static UI hosting

Serve `ui/` from within the WinPE image. The UI should call relative paths:
- `POST /api/v1/sessions`
- `WS /api/v1/sessions/{id}/ws`

No absolute URLs so that hostfwd/port changes do not break the UI.
