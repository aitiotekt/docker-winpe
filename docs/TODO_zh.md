# 待办事项 (TODO)

本文档记录了已知但尚未实现的改进项目。

## 高优先级

### 终端会话二次连接问题

**文件**: `ws.rs`, `session.rs`

`output_rx` 在首次 WebSocket 连接时被 `take()` 且不会恢复。当客户端断开后，`attached = false`，但 `output_rx` 仍为 `None`，导致后续连接失败（收到 1011 关闭码）。

**解决方案**:

- 使用 `tokio::sync::broadcast` 替代 `mpsc` 用于输出
- 或在断开时重新创建输出通道
- 或使用 `Arc<Mutex<Option<Receiver>>>` 模式允许恢复

### xterm.js CDN 依赖

**文件**: `ui/index.html`, `scripts/install-winpe-deps.ps1`

Web UI 依赖 CDN 加载 xterm.js，在离线 WinPE 环境中无法工作。

**解决方案**:

1. 在 `install-winpe-deps.ps1` 中下载 xterm.js/addons 到 `ui/vendor/`
2. 修改 `index.html` 使用本地路径
3. 确保 `build-winpe-iso.ps1` 包含 `ui/` 目录（已完成）

## 中优先级

### 环境变量未应用

**文件**: `executor.rs` (line 154, 303), `session.rs` (line 264)

API 请求中的 `env` 字段完全被忽略，`CreateProcessW` 的环境参数传 `NULL`。

**解决方案**:

- 构建环境块（以 null 分隔的 "KEY=VALUE" 字符串数组）
- 传递给 `CreateProcessW` 的 `lpEnvironment` 参数

### stdout/stderr 输出上限

**文件**: `executor.rs` (line 215)

`execute_command` 无限制读取输出到内存，长输出可能导致内存暴涨。

**解决方案**:

- 添加 16-64 MiB 上限
- 超过上限时截断并标记

### exec_stream 超时事件

**文件**: `executor.rs` (line 354)

流式执行超时时只发送 `Exit` 事件，客户端无法区分正常退出和超时。

**解决方案**:

- 添加 `Timeout` 事件类型
- 超时时发送 `Timeout` 事件而非 `Exit`

## 低优先级

### 命令行注入安全(先跳过)

**文件**: `executor.rs` (line 418)

`build_command_line` 简单拼接命令和参数，虽有引号处理但不完整。

**解决方案**:

- 实现完整的 Windows 命令行转义逻辑
- 考虑使用 `CommandLineToArgvW` 的逆操作

### Job Objects 进程树终止

**文件**: `executor.rs`

当前使用 `TerminateProcess` 终止进程，但不会终止子进程。

**解决方案**:

- 创建 Job Object
- 将进程添加到 Job
- 终止时终止整个 Job
