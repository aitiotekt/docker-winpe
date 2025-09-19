# TODO

This document lists known improvements that have not yet been implemented.

## High Priority

### Terminal session reconnection issue

**Files**: `ws.rs`, `session.rs`

`output_rx` is taken with `take()` on the first WebSocket connection and not restored. When the client disconnects, `attached = false` but `output_rx` remains `None`, causing subsequent connections to fail (receiving close code 1011).

**Solutions**:

- Use `tokio::sync::broadcast` instead of `mpsc` for output
- Or recreate the output channel on disconnect
- Or use `Arc<Mutex<Option<Receiver>>>` pattern to allow recovery

### xterm.js CDN dependency

**Files**: `ui/index.html`, `scripts/install-winpe-deps.ps1`

Web UI relies on CDN to load xterm.js, which won't work in offline WinPE environment.

**Solutions**:

1. Download xterm.js/addons to `ui/vendor/` in `install-winpe-deps.ps1`
2. Modify `index.html` to use local paths
3. Ensure `build-winpe-iso.ps1` includes `ui/` directory (already done)

## Medium Priority

### Environment variables not applied

**Files**: `executor.rs` (lines 154, 303), `session.rs` (line 264)

The `env` field in API requests is completely ignored, `NULL` is passed for the environment parameter of `CreateProcessW`.

**Solutions**:

- Build environment block (array of "KEY=VALUE" strings separated by null)
- Pass to the `lpEnvironment` parameter of `CreateProcessW`

### stdout/stderr output limit

**File**: `executor.rs` (line 215)

`execute_command` reads output to memory without limit, long output may cause memory explosion.

**Solutions**:

- Add 16-64 MiB upper bound
- When exceeding the limit, stop reading and mark it as truncated

### exec_stream timeout event

**File**: `executor.rs` (line 354)

When stream execution times out, only `Exit` event is sent, client cannot distinguish normal exit from timeout.

**Solutions**:

- Add `Timeout` event type
- Send `Timeout` event instead of `Exit` on timeout

## Low Priority

### Command line injection security

**File**: `executor.rs` (line 418)

`build_command_line` simply concatenates command and arguments,虽有 quote handling but not complete.

**Solutions**:

- Implement complete Windows command line escaping logic
- Consider using the inverse operation of `CommandLineToArgvW`

### Job Objects for process tree termination

**File**: `executor.rs`

Currently uses `TerminateProcess` to terminate processes, which does not terminate child processes.

**Solutions**:

- Create a Job Object
- Add process to the Job
- Terminate the entire Job on exit
