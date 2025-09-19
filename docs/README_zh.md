# 概述

docker-winpe 是一个在 Docker 中运行的 Tiny WinPE 的镜像

## 核心原则

- 最小化：只包含最基本的组件，不包含任何不必要的软件
- 目标：用于在 SteamOS、MacOS 和其他 Linux 中执行包括 chkdsk 等命令在内高度依赖 NT 内核的工具
- 无头：支持远程 Agent 命令执行
- 简化控制面：构建 WinPE 时注入 NetKVM 驱动，使用成熟的 TCP/IP + HTTP 控制

## 构建 WinPE ISO

基于 Windows ADK 构建一个基于 Windows 11 PE 25H2 的 Tiny WinPE 的镜像，使用 `scripts/build-winpe-iso.ps1` 脚本进行构建，需要注入所需的 `winpe-agent-server` 和 NetKVM 驱动，并在启动脚本中配置网络后后台运行 Agent。

## 构建 Docker 镜像

使用 `Dockerfile` 构建 Docker 镜像。

## 实现计划

这些文档指定 **docker-winpe** 的实现计划：一个在 QEMU 中运行的通用 WinPE 运行时，带有 Rust 代理服务器和 Rust 客户端。

- `ARCHITECTURE.md` — 运行时架构和控制/数据平面。
- `API_AUTOMATION.md` — 自动化 API (单命令执行) 规范。
- `API_TERMINAL.md` — 终端 API (ConPTY 会话) 规范。
- `WS_PROTOCOL.md` — 终端会话的 WebSocket 框架。
- `CLIENT_CLI.md` — `winpe-agent-client` CLI 模式和 UX。
- `SERVER_IMPL_NOTES.md` — 服务器实现说明 (Axum, ConPTY, Windows API, 清理)。
- `UI_NOTES.md` — WinPE-hosted web UI (xterm.js) 集成说明。
