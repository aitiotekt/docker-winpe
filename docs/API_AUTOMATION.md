# Automation API (Single-command execution)

## Purpose

The Automation API is designed for **non-interactive** execution of a single command and returning structured results. It is optimized for scripting, CI-like automation, and the `winpe-agent-client exec` mode.

This API must be stable and simple. It is not intended to support full-screen TUIs or interactive prompts; for that use the Terminal API (ConPTY).

## Base

- Base URL: `http://<host>:8080/api/v1`
- Content-Type: `application/json; charset=utf-8`

## Endpoints

### GET /health

Returns basic health and capability information.

Response 200:

```json
{
  "status": "ok",
  "version": "0.1.0",
  "capabilities": {
    "conpty": true,
    "automation": true,
    "terminal": true
  }
}
```

### POST /automation/exec

Execute a single command and return captured stdout/stderr.

Request:

```json
{
  "shell": "cmd",
  "command": "chkdsk",
  "args": ["D:", "/f", "/x"],
  "cwd": "X:\\",
  "env": {"FOO": "bar"},
  "timeout_ms": 600000,
  "encoding": "utf-8"
}
```

Notes:
- `shell` selects how the command is launched:
  - `cmd`: uses `cmd.exe /c <command> <args...>` by default.
  - `powershell`: uses `powershell.exe -NoLogo -NoProfile -Command ...` by default.
- Prefer passing `command` and `args` separately; the server should avoid `cmd.exe /c` string concatenation when possible.
- `timeout_ms` is enforced server-side (kill process on timeout).

Response 200:

```json
{
  "exit_code": 0,
  "stdout": "...",
  "stderr": "...",
  "duration_ms": 12345
}
```

Response 408 (timeout):

```json
{
  "error": {
    "code": "TIMEOUT",
    "message": "Process exceeded timeout",
    "details": {"timeout_ms": 600000}
  }
}
```

Response 400 (invalid request):

```json
{
  "error": {
    "code": "BAD_REQUEST",
    "message": "shell must be cmd|powershell"
  }
}
```

### POST /automation/exec_stream

Same as `/automation/exec` but streams output incrementally.

- Response uses **Server-Sent Events (SSE)** or **WebSocket** (choose one; SSE is simplest).
- Stream includes stdout/stderr chunks and a final exit event.

SSE event examples:

```
event: stdout
data: {"chunk":"...base64 or utf8..."}

event: stderr
data: {"chunk":"..."}

event: exit
data: {"exit_code":0,"duration_ms":12345}
```

This endpoint is optional. If implemented, keep semantics consistent with `/automation/exec`.

## Error model

All non-2xx responses should return:

```json
{
  "error": {
    "code": "STRING_CODE",
    "message": "Human readable message",
    "details": { }
  }
}
```

Recommended `code` values:
- `BAD_REQUEST`
- `NOT_FOUND`
- `TIMEOUT`
- `INTERNAL`
- `NOT_SUPPORTED` (e.g., PowerShell missing)

## Implementation notes

- Use Win32 `CreateProcessW` with redirected pipes for stdout/stderr.
- Avoid shell injection by never concatenating untrusted strings into a single command line where possible.
- Enforce an upper bound on captured output (e.g., 16–64 MiB) to prevent memory blowups.
- Ensure processes are terminated cleanly on timeout; consider Job Objects to kill child trees.
