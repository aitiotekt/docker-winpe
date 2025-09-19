# WinPE 托管的 Web UI (xterm.js)

## 目标

- 提供由 `winpe-agent-server` 托管的最小化 `/ui/` Web 应用。
- 允许用户在 xterm.js 终端中打开 `cmd` 或 `powershell`。
- 保持 UI 静态（无服务器端模板）。

## 页面

- `/ui/index.html`：UI 外壳、按钮、终端容器。

## 客户端工作流程

1. 使用所需的 shell 和初始列/行数 `POST /api/v1/sessions`。
2. 创建 WebSocket：`ws(s)://.../api/v1/sessions/{id}/ws`。
3. 绑定 xterm.js：
   - onData：发送二进制帧（使用 `TextEncoder`）。
   - onResize：发送 JSON 文本帧 `{type:'resize', cols, rows}`。
   - onClose：显示断开连接横幅。

## 最小 JS 骨架（参考）

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
      // 可选控制消息
      return;
    }
    term.write(new Uint8Array(ev.data));
  };

  ws.onclose = () => {
    term.write('\r\n[disconnected]\r\n');
  };
}
```

## 注意事项

- 优先将 xterm.js 资源打包到 `ui/` 中，以便 WinPE 可以在没有外部互联网的情况下提供它们。
- 不要假设字体；保持 CSS 简单。
- 考虑一个顶部栏，包含：
  - shell 选择器
  - 重新连接
  - 复制/粘贴提示
