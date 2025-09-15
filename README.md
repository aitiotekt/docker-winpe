# Docker Tiny WinPE

[中文说明](README_zh.md)

## Overview

A minimal WinPE image running in Docker, designed for NT-kernel dependent tooling.

Note: This project is in a prototype phase and is not yet stable.

## Requirements (ISO build)

- Windows 10/11 host
- Windows ADK + WinPE Add-on
- PowerShell 7+

Install with winget:

```powershell
just install-winpe-deps
```

## Build WinPE ISO

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-winpe-iso.ps1 -Arch amd64 -AgentServerPath build/winpe-agent-server.exe -OutputIsoPath build/winpe.iso -Clean
```

If `build/winpe-agent-server.exe` is missing, the ISO is still built without it.

## Build Docker image

```bash
docker build -t docker-tiny-winpe .
```

## Run

```bash
docker run -it --rm --privileged --device /dev/mmcblk0p1 -e DEVICE=/dev/mmcblk0p1 docker-tiny-winpe:latest
```

JSON-RPC example:

```bash
docker run -d --privileged --device /dev/mmcblk0p1 -e DEVICE=/dev/mmcblk0p1 docker-tiny-winpe:latest
curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc": "2.0", "method": "cmd", "params": ["/c", "chkdsk", "D:", "/r"], "id": 1}' http://localhost:8080/jsonrpc
```
