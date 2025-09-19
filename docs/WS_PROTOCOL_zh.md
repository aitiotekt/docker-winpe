# 终端会话的 WebSocket 协议

本文档定义了 Terminal API 使用的 WebSocket 帧格式。

## WebSocket URL

`GET /api/v1/sessions/{id}/ws`

## 帧类型

### 1) 二进制帧：终端字节流

- **客户端 -> 服务器**：原始输入字节（按惯例为 UTF-8）。服务器将这些字节写入 ConPTY 输入句柄。
- **服务器 -> 客户端**：原始输出字节。客户端将这些字节逐字写入 xterm.js（或 TUI 渲染器）。

除 WebSocket 消息边界外，不应用额外的帧格式。

### 2) 文本帧：JSON 控制消息

控制消息是带有 `type` 字段的 JSON 对象。

#### resize

当终端大小更改时由客户端发送。

```json
{"type":"resize","cols":120,"rows":30}
```

服务器操作：
- 调用 `ResizePseudoConsole(cols, rows)`。

#### signal

由客户端发送以请求信号。

```json
{"type":"signal","name":"ctrl_c"}
```

服务器操作（建议的初始行为）：
- `ctrl_c`：将字节 `0x03` 写入 ConPTY 输入流。
- `terminate`：终止子进程树。

#### ping（可选）

```json
{"type":"ping","t":1737060000}
```

服务器可能会回复：
```json
{"type":"pong","t":1737060000}
```

## 关闭代码

建议的关闭代码：
- `1000`：正常关闭
- `1008`：策略违规（例如，不允许第二次附加）
- `1011`：内部错误

## 输出分块

- 服务器应该从 ConPTY 输出读取块（例如 4 KiB 到 16 KiB），并将每个块作为二进制帧发送。
- 应用背压：如果 WebSocket 写缓冲区拥塞，则：
  - 丢弃旧的输出块（不理想），或
  - 使用 `1011` 关闭连接并让客户端重新连接。

## UTF-8 和代码页

当启用 `force_utf8` 时，服务器应该在会话启动时尝试将 shell 配置为 UTF-8。协议本身是面向字节的，不强制执行编码。
