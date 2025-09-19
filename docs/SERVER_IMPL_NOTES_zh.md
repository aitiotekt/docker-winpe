# 服务器实现说明（Rust + Axum + ConPTY）

本文档提供 `winpe-agent-server` 的实现者级说明。

## Crate 布局（建议）

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

## Axum 路由

- `/api/v1/health` -> health 处理器
- `/api/v1/automation/exec` -> 单个命令
- `/api/v1/sessions` -> 会话管理
- `/api/v1/sessions/:id/ws` -> WebSocket
- `/ui/*path` -> 静态文件服务

使用 `TowerHttp`：
- `tower_http::services::ServeDir` 来提供 `ui/`。
- `TraceLayer` 用于请求日志记录。

## ConPTY 模块

### 能力探测

启动时，探测 ConPTY 函数：
- `CreatePseudoConsole`
- `ResizePseudoConsole`
- `ClosePseudoConsole`

如果不可用，终端端点必须返回 `NOT_SUPPORTED`。

### 进程创建策略

使用 `STARTUPINFOEXW` 和 `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`：

1. 为 ConPTY 输入/输出创建管道。
2. 使用初始大小 `CreatePseudoConsole`。
3. 初始化属性列表并设置伪控制台属性。
4. `CreateProcessW` 用于：
   - cmd: `X:\Windows\System32\cmd.exe`
   - powershell: `X:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe` (PowerShell 5)

### 终止进程树

优先使用 Job Objects：
- 为每个会话创建一个作业。
- 将进程分配给作业。
- 终止时，关闭作业句柄或调用 `TerminateJobObject`。

### I/O 循环

- 从 ConPTY 输出读取是阻塞的；使用 `tokio::task::spawn_blocking`。
- 写入输入是阻塞的；使用 spawn_blocking 或专用线程。
- 维护每个会话的 `mpsc` 通道：
  - `ws_in_tx` (bytes -> writer)
  - `ws_out_tx` (bytes -> websocket)

### UTF-8 初始化

如果启用 `force_utf8`：
- cmd: 写入 `chcp 65001\r\n`
- PowerShell: 写入 `[Console]::InputEncoding=[Text.UTF8Encoding]::UTF8;[Console]::OutputEncoding=[Text.UTF8Encoding]::UTF8\r\n`

在会话启动后立即发送这些字节。

## 自动化执行

- 使用 `CreateProcessW` 并重定向管道。
- 使用有界缓冲区捕获 stdout/stderr。
- 强制执行超时。
- 优先使用 Job Objects 以确保子树不泄漏。

## 会话管理器

- 使用 `DashMap<SessionId, SessionHandle>`。
- 会话状态：`Running`、`Exited`。
- 附加状态：布尔值或 `Option<AttachedClientId>`。
- 清理：
  - 如果分离且空闲 > 超时：终止
  - 如果已退出：删除

## 错误处理

实现一个统一的错误类型，映射到：

```
{ "error": { "code": "...", "message": "...", "details": {...} } }
```

## 静态 UI 托管

从 WinPE 镜像内提供 `ui/`。UI 应该调用相对路径：
- `POST /api/v1/sessions`
- `WS /api/v1/sessions/{id}/ws`

不使用绝对 URL，这样 hostfwd/端口更改不会破坏 UI。
