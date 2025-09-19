# winpe-agent-client CLI design

`winpe-agent-client` is a Rust CLI that talks to `winpe-agent-server`.

It supports three modes:

1. `exec` — execute a single command via the Automation API and print results in the current terminal.
2. `tui` — open a ConPTY session via the Terminal API and render it locally using a TUI terminal renderer (`tui-term`).
3. `web` — open the user's browser and navigate to the server-hosted xterm.js UI.

## Global options

- `--url <URL>`: base URL of the server, e.g. `http://127.0.0.1:8080`
- `--token <TOKEN>`: optional bearer token
- `--timeout <DURATION>`: request timeout for exec mode

Example:

```
winpe-agent-client --url http://127.0.0.1:8080 exec --shell cmd -- chkdsk D: /f /x
```

## Mode: exec

### Synopsis

```
winpe-agent-client exec [--shell cmd|powershell] [--cwd PATH] [--timeout MS] -- <command> [args...]
```

### Behavior

- Calls `POST /api/v1/automation/exec`.
- Prints stdout to stdout and stderr to stderr.
- Exits with the remote exit code.

### Output formatting

- Default: raw output.
- Optional: `--json` to print the full response document.

## Mode: tui

### Synopsis

```
winpe-agent-client tui [--shell cmd|powershell] [--cols N] [--rows N]
```

### Behavior

- Calls `POST /api/v1/sessions` to create a ConPTY session.
- Attaches to `GET /api/v1/sessions/{id}/ws`.
- Uses a local TUI renderer to approximate xterm.js behavior:
  - render byte stream
  - capture keyboard input
  - send resize events

Notes:
- This mode should be treated as a best-effort renderer. xterm.js in the browser is the reference UI.

## Mode: web

### Synopsis

```
winpe-agent-client web
```

### Behavior

- Opens the default browser to `http://<server>/ui/`.

Implementation notes:
- Use platform-appropriate browser launch (e.g., `open` on macOS, `xdg-open` on Linux, `start` on Windows).
