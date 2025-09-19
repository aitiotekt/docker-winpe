# WebSocket protocol for Terminal sessions

This document defines the WebSocket framing used by the Terminal API.

## WebSocket URL

`GET /api/v1/sessions/{id}/ws`

## Frame types

### 1) Binary frames: terminal byte stream

- **Client -> Server**: raw input bytes (UTF-8 by convention). The server writes these bytes into the ConPTY input handle.
- **Server -> Client**: raw output bytes. The client writes these bytes into xterm.js (or a TUI renderer) verbatim.

No additional framing is applied beyond WebSocket message boundaries.

### 2) Text frames: JSON control messages

Control messages are JSON objects with `type` fields.

#### resize

Sent by client when terminal size changes.

```json
{"type":"resize","cols":120,"rows":30}
```

Server action:
- Call `ResizePseudoConsole(cols, rows)`.

#### signal

Sent by client to request signals.

```json
{"type":"signal","name":"ctrl_c"}
```

Server action (recommended initial behavior):
- `ctrl_c`: write byte `0x03` to the ConPTY input stream.
- `terminate`: terminate the child process tree.

#### ping (optional)

```json
{"type":"ping","t":1737060000}
```

Server may reply with:
```json
{"type":"pong","t":1737060000}
```

## Close codes

Recommended close codes:
- `1000`: normal close
- `1008`: policy violation (e.g., second attach not allowed)
- `1011`: internal error

## Output chunking

- Server should read from ConPTY output in chunks (e.g., 4 KiB to 16 KiB) and send each chunk as a binary frame.
- Apply backpressure: if the WebSocket write buffer is congested, either:
  - drop old output chunks (not ideal), or
  - close the connection with `1011` and let the client reconnect.

## UTF-8 and code pages

The server should attempt to configure the shell to UTF-8 at session start when `force_utf8` is enabled. The protocol itself is byte-oriented and does not enforce encoding.
