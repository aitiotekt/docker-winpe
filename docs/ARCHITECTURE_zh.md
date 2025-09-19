# 架构

## 概述

`docker-winpe` 在 QEMU 中引导自定义 WinPE ISO，并在 WinPE 内运行 Rust 代理。代理公开两个 API 表面：

1. **自动化 API**：运行单个命令（cmd/powershell）并返回结构化结果。
2. **终端 API**：支持 xterm.js 和其他终端客户端的 ConPTY 交互式终端会话，通过 WebSocket 字节流。

服务器还托管一个小型 Web UI（静态文件），其中嵌入了 xterm.js 并与终端 API 通信。

## 运行时拓扑

```
Host OS
  └─ Docker container (Arch + qemu-system-x86_64)
       └─ QEMU VM (WinPE)
            └─ winpe-agent-server (Rust + Axum)
                  ├─ HTTP: Automation API + Session mgmt
                  ├─ WS: Terminal sessions
                  └─ Static UI: /ui (xterm.js)
```

### 网络

- QEMU 使用用户模式网络（`-netdev user`）和 `hostfwd`。
- Docker 从容器到主机发布单个 TCP 端口（默认 `8080`）。

路径：

```
Browser / CLI (host)
  -> host:8080
  -> docker port mapping
  -> QEMU hostfwd
  -> WinPE:8080 (winpe-agent-server)
```

## 设备直通（Linux 主机）

物理磁盘或分区从主机 -> Docker -> QEMU -> WinPE 传递。

- Compose 将主机块设备（例如 `/dev/nvme0n1p4`）映射到容器作为 `/disk2`、`/disk3` 等。
- 容器入口点自动将 `/diskN` 作为原始驱动器附加到 QEMU。
- WinPE 将这些视为额外的磁盘并分配驱动器号。

**要求：** 主机操作系统在 WinPE 中修复目标分区时不得挂载它。

## 服务器职责

### 自动化 API

- 执行单个命令并返回：
  - 退出代码
  - stdout/stderr（已捕获）
  - 可选的计时/元数据

设计意图：

- 最适合自动化管道和 CLI 使用。
- 不适用于完全交互式程序。

### 终端 API (ConPTY)

- 创建由 ConPTY 支持的交互式终端会话。
- 公开流式原始终端字节的 WebSocket 端点。
- 支持调整大小和基本信号。

设计意图：

- 最适合人类；支持 cmd/powershell 交互行为。

### 静态 UI

- 提供 `/ui/*`（xterm.js、JS 粘合代码、最小 HTML）。
- UI 执行：
  - 创建会话
  - 打开 WebSocket
  - 处理调整大小
  - 将字节渲染到 xterm.js

## 客户端职责 (winpe-agent-client)

三种模式：

1. `exec`：通过 Automation API 运行一个命令，并将输出打印到当前终端。
2. `tui`：打开 ConPTY 会话，并使用本地 Rust TUI（tui-term）渲染伪终端。
3. `web`：打开浏览器到服务器托管的 UI。

## 约束和设计决策

- 优先在单个端口上使用 HTTP+WS 以简化。
- 避免实现自定义链路层协议。
- 保持 TLS 可选；默认将主机端口绑定到 `127.0.0.1`。
- 在运行时使用能力探测来确认 ConPTY 可用性。

## 预期项目结构

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
