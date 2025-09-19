# Architecture

## Overview

`docker-winpe` boots a custom WinPE ISO in QEMU and runs a Rust agent inside WinPE. The agent exposes two API surfaces:

1. **Automation API**: run a single command (cmd/powershell) and return structured results.
2. **Terminal API**: ConPTY-backed interactive terminal sessions for xterm.js and other terminal clients via WebSocket byte streaming.

The server also hosts a small web UI (static files) that embeds xterm.js and talks to the Terminal API.

## Runtime topology

```
Host OS
  └─ Docker container (Arch + qemu-system-x86_64)
       └─ QEMU VM (WinPE)
            └─ winpe-agent-server (Rust + Axum)
                  ├─ HTTP: Automation API + Session mgmt
                  ├─ WS: Terminal sessions
                  └─ Static UI: /ui (xterm.js)
```

### Networking

- QEMU uses user-mode networking (`-netdev user`) with `hostfwd`.
- Docker publishes a single TCP port (default `8080`) from container to host.

Path:

```
Browser / CLI (host)
  -> host:8080
  -> docker port mapping
  -> QEMU hostfwd
  -> WinPE:8080 (winpe-agent-server)
```

## Device passthrough (Linux hosts)

Physical disks or partitions are passed from host -> Docker -> QEMU -> WinPE.

- Compose maps host block devices (e.g., `/dev/nvme0n1p4`) into the container as `/disk2`, `/disk3`, ...
- The container entrypoint auto-attaches `/diskN` to QEMU as raw drives.
- WinPE sees these as additional disks and assigns drive letters.

**Requirement:** the host OS must not mount the target partition while it is being repaired in WinPE.

## Server responsibilities

### Automation API

- Execute a single command and return:
  - exit code
  - stdout/stderr (captured)
  - optional timing/metadata

Design intent:
- Best for automation pipelines and CLI use.
- Not intended for fully interactive programs.

### Terminal API (ConPTY)

- Create interactive terminal sessions backed by ConPTY.
- Expose a WebSocket endpoint that streams raw terminal bytes.
- Support resize and basic signals.

Design intent:
- Best for humans; supports cmd/powershell interactive behavior.

### Static UI

- Serves `/ui/*` (xterm.js, JS glue, minimal HTML).
- UI performs:
  - create session
  - open WebSocket
  - handle resize
  - render bytes into xterm.js

## Client responsibilities (winpe-agent-client)

Three modes:

1. `exec`: run one command via Automation API and print output to current terminal.
2. `tui`: open a ConPTY session and render a pseudo terminal using a Rust TUI (tui-term) locally.
3. `web`: open the browser to the server-hosted UI.

## Constraints and design decisions

- Prefer HTTP+WS on a single port for simplicity.
- Avoid implementing custom link-layer protocols.
- Keep TLS optional; default to binding host port to `127.0.0.1`.
- Use capability probing at runtime to confirm ConPTY availability.

## Suggested repository layout

```text
.
├─ apps/
│  ├─ agent-client/
│  └─ agent-server/
├─ packages/
│  └─ agent-core/
├─ scripts/
│  ├─ install-winpe-deps.ps1
│  ├─ entrypoint.sh
│  ├─ build-winpe-iso.ps1
│  └─ startup.ps1
├─ Dockerfile
├─ docker-compose.yml
└─ docs/
```
