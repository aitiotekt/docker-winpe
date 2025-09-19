# Overview

docker-winpe is a Tiny WinPE image running in Docker

## Core principles

- Minimalism: Only includes the most basic components, no unnecessary software
- Goal: Execute highly NT-kernel dependent tools like chkdsk in SteamOS, MacOS and other Linux
- Headless: Supports remote Agent command execution
- Simplified control plane: Inject NetKVM driver when building WinPE, use mature TCP/IP + HTTP for control

## Building WinPE ISO

Build a Tiny WinPE image based on Windows 11 PE 25H2 using Windows ADK with the `scripts/build-winpe-iso.ps1` script. Need to inject the required `winpe-agent-server` and NetKVM driver, and run the Agent in the background after configuring the network in the startup script.

## Building Docker image

Use `Dockerfile` to build the Docker image.

## Implementation plan

These documents specify the implementation plan for **docker-winpe**: a general-purpose WinPE runtime running in QEMU with a Rust agent server and Rust client.

- `ARCHITECTURE.md` — Runtime architecture and control/data plane.
- `API_AUTOMATION.md` — Automation API (single-command execution) specification.
- `API_TERMINAL.md` — Terminal API (ConPTY sessions) specification.
- `WS_PROTOCOL.md` — WebSocket framing for terminal sessions.
- `CLIENT_CLI.md` — `winpe-agent-client` CLI modes and UX.
- `SERVER_IMPL_NOTES.md` — Server implementation notes (Axum, ConPTY, Windows APIs, cleanup).
- `UI_NOTES.md` — WinPE-hosted web UI (xterm.js) integration notes.
