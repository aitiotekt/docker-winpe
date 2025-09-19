# winpe-agent-client CLI 设计

`winpe-agent-client` 是一个与 `winpe-agent-server` 通信的 Rust CLI。

它支持三种模式：

1. `exec` — 通过 Automation API 执行单个命令并在当前终端中打印结果。
2. `tui` — 通过 Terminal API 打开 ConPTY 会话，并使用本地 TUI 终端渲染器（`tui-term`）渲染它。
3. `web` — 打开用户的浏览器并导航到服务器托管的 xterm.js UI。

## 全局选项

- `--url <URL>`：服务器的基础 URL，例如 `http://127.0.0.1:8080`
- `--token <TOKEN>`：可选的 bearer token
- `--timeout <DURATION>`：exec 模式的请求超时

示例：

```
winpe-agent-client --url http://127.0.0.1:8080 exec --shell cmd -- chkdsk D: /f /x
```

## 模式：exec

### 摘要

```
winpe-agent-client exec [--shell cmd|powershell] [--cwd PATH] [--timeout MS] -- <command> [args...]
```

### 行为

- 调用 `POST /api/v1/automation/exec`。
- 将 stdout 打印到 stdout，将 stderr 打印到 stderr。
- 使用远程退出码退出。

### 输出格式

- 默认：原始输出。
- 可选：`--json` 打印完整的响应文档。

## 模式：tui

### 摘要

```
winpe-agent-client tui [--shell cmd|powershell] [--cols N] [--rows N]
```

### 行为

- 调用 `POST /api/v1/sessions` 创建 ConPTY 会话。
- 附加到 `GET /api/v1/sessions/{id}/ws`。
- 使用本地 TUI 渲染器来近似 xterm.js 行为：
  - 渲染字节流
  - 捕获键盘输入
  - 发送调整大小事件

注意：
- 此模式应被视为尽力而为的渲染器。浏览器中的 xterm.js 是参考 UI。

## 模式：web

### 摘要

```
winpe-agent-client web
```

### 行为

- 将默认浏览器打开到 `http://<server>/ui/`。

实现说明：
- 使用平台适当的浏览器启动（例如，macOS 上的 `open`，Linux 上的 `xdg-open`，Windows 上的 `start`）。
