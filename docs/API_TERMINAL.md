# Terminal API (ConPTY sessions)

## Purpose

The Terminal API provides **interactive** terminal sessions backed by Windows ConPTY. It is designed to support:

- A browser UI using xterm.js.
- The `winpe-agent-client tui` mode using a local TUI renderer.

Terminal sessions are stateful resources. A session is created via HTTP and then attached via WebSocket.

## Base

- Base URL: `http://<host>:8080/api/v1`

## Concepts

### Session

A session corresponds to:
- A ConPTY instance (pseudo console handle)
- A child process (`cmd.exe` or `powershell.exe`)
- Current terminal size (cols/rows)
- Attachment state (optionally allow only one active WebSocket at a time)

Recommended session id format: ULID (26 char) or UUID.

## Endpoints

### POST /sessions

Create a new ConPTY-backed session.

Request:

```json
{
  "shell": "cmd",
  "cwd": "X:\\",
  "env": { "FOO": "bar" },
  "cols": 120,
  "rows": 30,
  "idle_timeout_sec": 600,
  "init": {
    "force_utf8": true
  }
}
```

Response 201:

```json
{
  "id": "01HY...",
  "ws_url": "/api/v1/sessions/01HY.../ws",
  "created_at": "2026-01-16T21:10:00Z"
}
```

Note:

- `force_utf8=true` should cause the server to write an initialization sequence:
  - cmd: `chcp 65001\r\n`
  - PowerShell 5: `[Console]::InputEncoding=[Text.UTF8Encoding]::UTF8;[Console]::OutputEncoding=[Text.UTF8Encoding]::UTF8\r\n`

### GET /sessions

List sessions.

Response 200:

```json
[
  {
    "id": "01HY...",
    "shell": "cmd",
    "pid": 1234,
    "state": "running",
    "attached": false,
    "cols": 120,
    "rows": 30,
    "created_at": "2026-01-16T21:10:00Z",
    "last_activity_at": "2026-01-16T21:10:05Z"
  }
]
```

### GET /sessions/{id}

Get session detail.

### DELETE /sessions/{id}

Terminate the child process and release ConPTY resources.

### POST /sessions/{id}/signal

Send a signal.

Request:

```json
{ "signal": "ctrl_c" }
```

Supported signals:

- `ctrl_c`
- `terminate`
- `ctrl_break`

### GET /sessions/{id}/ws (WebSocket)

Upgrade to a WebSocket that carries terminal I/O.

- Client-to-server binary frames: raw input bytes.
- Server-to-client binary frames: raw output bytes.
- Client-to-server text frames: JSON control messages (resize, signal).

See `WS_PROTOCOL.md` for exact framing.

## Behavioral rules

- Session creation must fail if ConPTY is unavailable (`NOT_SUPPORTED`).
- A session may be configured to allow only one attachment at a time.
  - If a second WS attaches, either reject with close code or detach the old one.
- Idle timeout should terminate detached sessions automatically.

## Implementation notes (Rust)

- Use the `windows` crate for ConPTY bindings.
- Prefer Job Objects to ensure the entire child process tree is terminated.
- Terminal I/O is a byte stream; do not normalize newlines or strip escape sequences.
