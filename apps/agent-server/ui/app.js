// WinPE Agent Web UI - Terminal Application

(function () {
    'use strict';

    // DOM elements
    const terminalContainer = document.getElementById('terminal-container');
    const shellSelect = document.getElementById('shell-select');
    const connectBtn = document.getElementById('connect-btn');
    const disconnectBtn = document.getElementById('disconnect-btn');
    const statusEl = document.getElementById('status');
    const sessionIdEl = document.getElementById('session-id');

    // Terminal state
    let terminal = null;
    let fitAddon = null;
    let ws = null;
    let sessionId = null;

    // Initialize terminal
    function initTerminal() {
        terminal = new Terminal({
            cursorBlink: true,
            fontSize: 14,
            fontFamily: 'Consolas, "Courier New", monospace',
            theme: {
                background: '#000000',
                foreground: '#cdd6f4',
                cursor: '#89b4fa',
                cursorAccent: '#1e1e2e',
                selection: 'rgba(137, 180, 250, 0.3)',
                black: '#45475a',
                red: '#f38ba8',
                green: '#a6e3a1',
                yellow: '#f9e2af',
                blue: '#89b4fa',
                magenta: '#f5c2e7',
                cyan: '#94e2d5',
                white: '#bac2de',
                brightBlack: '#585b70',
                brightRed: '#f38ba8',
                brightGreen: '#a6e3a1',
                brightYellow: '#f9e2af',
                brightBlue: '#89b4fa',
                brightMagenta: '#f5c2e7',
                brightCyan: '#94e2d5',
                brightWhite: '#a6adc8'
            },
            convertEol: false,
            scrollback: 10000
        });

        fitAddon = new FitAddon.FitAddon();
        terminal.loadAddon(fitAddon);
        terminal.open(terminalContainer);
        fitAddon.fit();

        // Handle window resize
        window.addEventListener('resize', () => {
            fitAddon.fit();
            sendResize();
        });

        // Handle terminal input
        terminal.onData(data => {
            if (ws && ws.readyState === WebSocket.OPEN) {
                // Send as binary
                const encoder = new TextEncoder();
                ws.send(encoder.encode(data));
            }
        });

        terminal.onResize(({ cols, rows }) => {
            sendResize();
        });

        terminal.write('Welcome to WinPE Agent Terminal\r\n');
        terminal.write('Select a shell and click Connect to start.\r\n\r\n');
    }

    // Send resize message
    function sendResize() {
        if (ws && ws.readyState === WebSocket.OPEN && terminal) {
            ws.send(JSON.stringify({
                type: 'resize',
                cols: terminal.cols,
                rows: terminal.rows
            }));
        }
    }

    // Update UI state
    function setConnected(connected) {
        connectBtn.disabled = connected;
        disconnectBtn.disabled = !connected;
        shellSelect.disabled = connected;

        if (connected) {
            statusEl.textContent = 'Connected';
            statusEl.className = 'connected';
        } else {
            statusEl.textContent = 'Disconnected';
            statusEl.className = '';
            sessionIdEl.textContent = '';
        }
    }

    // Create session and connect
    async function connect() {
        const shell = shellSelect.value;
        const cols = terminal.cols;
        const rows = terminal.rows;

        setStatus('Connecting...');

        try {
            // Create session
            const response = await fetch('/api/v1/sessions', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    shell,
                    cols,
                    rows,
                    init: { force_utf8: true }
                })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.error?.message || 'Failed to create session');
            }

            const session = await response.json();
            sessionId = session.id;
            sessionIdEl.textContent = `Session: ${sessionId}`;

            // Connect WebSocket
            const protocol = location.protocol === 'https:' ? 'wss' : 'ws';
            const wsUrl = `${protocol}://${location.host}${session.ws_url}`;

            ws = new WebSocket(wsUrl);
            ws.binaryType = 'arraybuffer';

            ws.onopen = () => {
                terminal.clear();
                terminal.focus();
                setConnected(true);
                sendResize();
            };

            ws.onmessage = (event) => {
                if (typeof event.data === 'string') {
                    // Control message
                    try {
                        const msg = JSON.parse(event.data);
                        console.log('Control message:', msg);
                    } catch (e) {
                        // Not JSON, ignore
                    }
                } else {
                    // Binary terminal data
                    terminal.write(new Uint8Array(event.data));
                }
            };

            ws.onclose = (event) => {
                terminal.write('\r\n[Connection closed]\r\n');
                setConnected(false);
                ws = null;
            };

            ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                setStatus('Connection error', true);
            };

        } catch (error) {
            console.error('Connection error:', error);
            setStatus(error.message, true);
        }
    }

    // Disconnect
    async function disconnect() {
        if (ws) {
            ws.close();
            ws = null;
        }

        if (sessionId) {
            try {
                await fetch(`/api/v1/sessions/${sessionId}`, { method: 'DELETE' });
            } catch (e) {
                console.error('Failed to delete session:', e);
            }
            sessionId = null;
        }

        setConnected(false);
        terminal.write('\r\n[Disconnected]\r\n');
    }

    // Set status message
    function setStatus(message, isError = false) {
        statusEl.textContent = message;
        statusEl.className = isError ? 'error' : '';
    }

    // Event listeners
    connectBtn.addEventListener('click', connect);
    disconnectBtn.addEventListener('click', disconnect);

    // Initialize on load
    initTerminal();
})();
