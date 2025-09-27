# TODO

This document lists known improvements that have not yet been implemented.

## Completed ✅

### ~~Terminal session reconnection issue~~

**Status**: Fixed

Use `tokio::sync::broadcast` instead of `mpsc` for output channel, supporting multiple `subscribe()` calls for reconnection after disconnect.

### ~~Environment variables not applied~~

**Status**: Fixed

Added `build_environment_block()` function to pass `CREATE_UNICODE_ENVIRONMENT` flag in `CreateProcessW`.

### ~~exec_stream timeout event~~

**Status**: Fixed

Added `StreamEvent::Timeout` variant, SSE event type is `timeout`, client can distinguish timeout from normal exit.

### ~~Job Objects for process tree termination~~

**Status**: Fixed

Use Windows Job Objects API with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` flag, terminate entire process tree on timeout.

---

## High Priority

### xterm.js CDN dependency

**Files**: `ui/index.html`, `scripts/install-winpe-deps.ps1`

Web UI relies on CDN to load xterm.js, which won't work in offline WinPE environment.

**Solutions**:

1. Download xterm.js/addons to `ui/vendor/` in `install-winpe-deps.ps1`
2. Modify `index.html` to use local paths
3. Ensure `build-winpe-iso.ps1` includes `ui/` directory (already done)

---

## Medium Priority

### stdout/stderr output limit

**File**: `executor.rs` (line 215)

`execute_command` reads output to memory without limit, long output may cause memory explosion.

**Solutions**:

- Add 16-64 MiB upper bound
- When exceeding limit, stop reading and mark as truncated

---

## Low Priority

### Command line injection security (skipped for now)

**File**: `executor.rs` (line 418)

`build_command_line` simply concatenates command and arguments,虽有 quote handling but not complete.

**Solutions**:

- Implement complete Windows command line escaping logic
- Consider using the inverse operation of `CommandLineToArgvW`
