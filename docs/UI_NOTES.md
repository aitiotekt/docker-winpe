# WinPE-hosted web UI (xterm.js)

## Goals

- Provide a minimal `/ui/` web app hosted by `winpe-agent-server`.
- Allow users to open either `cmd` or `powershell` in an xterm.js terminal.
- Keep the UI static (no server-side templates).

## Pages

- `/ui/index.html`: UI shell, buttons, terminal container.

## Client workflow

1. `POST /api/v1/sessions` with desired shell and initial cols/rows.
2. Create WebSocket: `ws(s)://.../api/v1/sessions/{id}/ws`.
3. Bind xterm.js:
   - onData: send binary frames (use `TextEncoder`).
   - onResize: send JSON text frame {type:'resize', cols, rows}.
   - onClose: show disconnect banner.

## Minimal JS skeleton (reference)

```js
const term = new Terminal({ convertEol: false, cursorBlink: true });
term.open(document.getElementById('terminal'));

async function openSession(shell) {
  const cols = term.cols;
  const rows = term.rows;
  const resp = await fetch('/api/v1/sessions', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ shell, cols, rows, init: { force_utf8: true } })
  });
  const { ws_url } = await resp.json();

  const ws = new WebSocket((location.protocol === 'https:' ? 'wss' : 'ws') + '://' + location.host + ws_url);
  ws.binaryType = 'arraybuffer';

  const enc = new TextEncoder();
  term.onData(d => ws.readyState === 1 && ws.send(enc.encode(d)));
  term.onResize(({ cols, rows }) => ws.readyState === 1 && ws.send(JSON.stringify({ type: 'resize', cols, rows })));

  ws.onmessage = (ev) => {
    if (typeof ev.data === 'string') {
      // optional control messages
      return;
    }
    term.write(new Uint8Array(ev.data));
  };

  ws.onclose = () => {
    term.write('\r\n[disconnected]\r\n');
  };
}
```

## Notes

- Prefer bundling xterm.js assets into `ui/` so WinPE can serve them without external internet.
- Do not assume fonts; keep CSS simple.
- Consider a top bar with:
  - shell selector
  - reconnect
  - copy/paste hints
