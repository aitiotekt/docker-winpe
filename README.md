# Docker WinPE

[中文说明](README_zh.md)

## Overview

A minimal WinPE image running in Docker, designed for NT-kernel dependent tooling.

Note: This project is in a prototype phase and is not yet stable.

## Requirements (ISO build)

- Windows 10/11/Server2025 host
- Windows ADK + WinPE Add-on
- PowerShell 7+

Install with winget:

```powershell
just install-winpe-deps
```

## Build WinPE ISO

```powershell
just build-winpe-iso
```

## Build Docker image

```bash
docker build -t docker-winpe .
```
