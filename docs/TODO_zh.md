# 待办事项 (TODO)

本文档记录了已知但尚未实现的改进项目。

## 已完成 ✅

### ~~终端会话二次连接问题~~

**状态**: 已修复

使用 `tokio::sync::broadcast` 替代 `mpsc` 用于输出通道，支持多次 `subscribe()` 实现断开重连。

### ~~环境变量未应用~~

**状态**: 已修复

添加了 `build_environment_block()` 函数，在 `CreateProcessW` 中传递 `CREATE_UNICODE_ENVIRONMENT` 标志。

### ~~exec_stream 超时事件~~

**状态**: 已修复

添加了 `StreamEvent::Timeout` 变体，SSE 事件类型为 `timeout`，客户端可区分超时和正常退出。

### ~~Job Objects 进程树终止~~

**状态**: 已修复

使用 Windows Job Objects API，配置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 标志，超时时终止整个进程树。

---

## 高优先级

### xterm.js CDN 依赖

**文件**: `ui/index.html`, `scripts/install-winpe-deps.ps1`

Web UI 依赖 CDN 加载 xterm.js，在离线 WinPE 环境中无法工作。

**解决方案**:

1. 在 `install-winpe-deps.ps1` 中下载 xterm.js/addons 到 `ui/vendor/`
2. 修改 `index.html` 使用本地路径
3. 确保 `build-winpe-iso.ps1` 包含 `ui/` 目录（已完成）

---

## 中优先级

### stdout/stderr 输出上限

**文件**: `executor.rs` (line 215)

`execute_command` 无限制读取输出到内存，长输出可能导致内存暴涨。

**解决方案**:

- 添加 16-64 MiB 上限
- 超过上限时截断并标记

---

## 低优先级

### 命令行注入安全 (先跳过)

**文件**: `executor.rs` (line 418)

`build_command_line` 简单拼接命令和参数，虽有引号处理但不完整。

**解决方案**:

- 实现完整的 Windows 命令行转义逻辑
- 考虑使用 `CommandLineToArgvW` 的逆操作
