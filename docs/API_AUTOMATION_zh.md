# 自动化 API（单命令执行）

## 目的

自动化 API 专为**非交互式**执行单个命令并返回结构化结果而设计。它针对脚本编写、类似 CI 的自动化和 `winpe-agent-client exec` 模式进行了优化。

此 API 必须稳定且简单。它不支持全屏 TUI 或交互式提示；对于这些功能，请使用终端 API（ConPTY）。

## 基础

- 基础 URL：`http://<host>:8080/api/v1`
- Content-Type: `application/json; charset=utf-8`

## 端点

### GET /health

返回基本健康和能力信息。

响应 200：
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

执行单个命令并返回捕获的 stdout/stderr。

请求：
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

注意：
- `shell` 选择如何启动命令：
  - `cmd`：默认使用 `cmd.exe /c <command> <args...>`。
  - `powershell`：默认使用 `powershell.exe -NoLogo -NoProfile -Command ...`。
- 优先分别传递 `command` 和 `args`；服务器应尽可能避免 `cmd.exe /c` 字符串连接。
- `timeout_ms` 在服务器端强制执行（在超时时终止进程）。

响应 200：
```json
{
  "exit_code": 0,
  "stdout": "...",
  "stderr": "...",
  "duration_ms": 12345
}
```

响应 408（超时）：
```json
{
  "error": {
    "code": "TIMEOUT",
    "message": "Process exceeded timeout",
    "details": {"timeout_ms": 600000}
  }
}
```

响应 400（无效请求）：
```json
{
  "error": {
    "code": "BAD_REQUEST",
    "message": "shell must be cmd|powershell"
  }
}
```

### POST /automation/exec_stream

与 `/automation/exec` 相同，但增量流式输出。

- 响应使用**服务器发送事件 (SSE)** 或 **WebSocket**（选择一个；SSE 最简单）。
- 流包括 stdout/stderr 块和最终的退出事件。

SSE 事件示例：
```
event: stdout
data: {"chunk":"...base64 or utf8..."}

event: stderr
data: {"chunk":"..."}

event: exit
data: {"exit_code":0,"duration_ms":12345}
```

此端点是可选的。如果实现，请保持与 `/automation/exec` 的语义一致。

## 错误模型

所有非 2xx 响应应返回：

```json
{
  "error": {
    "code": "STRING_CODE",
    "message": "Human readable message",
    "details": { }
  }
}
```

建议的 `code` 值：
- `BAD_REQUEST`
- `NOT_FOUND`
- `TIMEOUT`
- `INTERNAL`
- `NOT_SUPPORTED`（例如，缺少 PowerShell）

## 实现说明

- 使用 Win32 `CreateProcessW` 并为 stdout/stderr 重定向管道。
- 避免尽可能不要将不受信任的字符串连接到单个命令行中以避免 shell 注入。
- 对捕获的输出强制执行上限（例如 16–64 MiB）以防止内存爆炸。
- 确保在超时时干净地终止进程；考虑使用 Job Objects 来终止子树。
