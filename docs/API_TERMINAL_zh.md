# 终端 API（ConPTY 会话）

## 目的

终端 API 提供由 Windows ConPTY 支持的**交互式**终端会话。它旨在支持：

- 使用 xterm.js 的浏览器 UI。
- 使用本地 TUI 渲染器的 `winpe-agent-client tui` 模式。

终端会话是有状态资源。通过 HTTP 创建会话，然后通过 WebSocket 附加。

## 基础

- 基础 URL：`http://<host>:8080/api/v1`

## 概念

### 会话

会话对应于：

- 一个 ConPTY 实例（伪控制台句柄）
- 一个子进程（`cmd.exe` 或 `powershell.exe`）
- 当前终端大小（列/行）
- 附加状态（可选地每次只允许一个活动 WebSocket）

建议的会话 ID 格式：ULID（26 字符）或 UUID。

## 端点

### POST /sessions

创建一个新的 ConPTY 支持的会话。

请求：

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

响应 201：

```json
{
  "id": "01HY...",
  "ws_url": "/api/v1/sessions/01HY.../ws",
  "created_at": "2026-01-16T21:10:00Z"
}
```

注意：

- `force_utf8=true` 应该导致服务器写入初始化序列：
  - cmd: `chcp 65001\r\n`
  - PowerShell 5: `[Console]::InputEncoding=[Text.UTF8Encoding]::UTF8;[Console]::OutputEncoding=[Text.UTF8Encoding]::UTF8\r\n`

### GET /sessions

列出会话。

响应 200：

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

获取会话详细信息。

### DELETE /sessions/{id}

终止子进程并释放 ConPTY 资源。

### POST /sessions/{id}/signal

发送信号。

请求：

```json
{ "signal": "ctrl_c" }
```

支持的信号：

- `ctrl_c`
- `terminate`
- `ctrl_break`

### GET /sessions/{id}/ws (WebSocket)

升级到携带终端 I/O 的 WebSocket。

- 客户端到服务器二进制帧：原始输入字节。
- 服务器到客户端二进制帧：原始输出字节。
- 客户端到服务器文本帧：JSON 控制消息（resize、signal）。

有关确切的帧格式，请参见 `ws_protocol.md`。

## 行为规则

- 如果 ConPTY 不可用，会话创建必须失败（`NOT_SUPPORTED`）。
- 会话可以配置为每次只允许一个附加。
  - 如果第二个 WS 附加，则使用关闭代码拒绝或分离旧的一个。
- 空闲超时应自动终止分离的会话。

## 实现说明（Rust）

- 使用 `windows` crate 进行 ConPTY 绑定。
- 优先使用 Job Objects 以确保整个子进程树被终止。
- 终端 I/O 是字节流；不要规范化换行符或剥离转义序列。
