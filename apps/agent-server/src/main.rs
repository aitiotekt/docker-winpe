use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::process::Command;
use std::thread;
use std::time::Duration;

use serde_json::Value;
use winpe_agent_core::{JsonRpcRequest, JsonRpcResponse};

fn main() -> io::Result<()> {
    let port_name = std::env::var("AGENT_COM_PORT").unwrap_or_else(|_| "COM1".to_string());
    let device_path = format!(r"\\.\{}", port_name);

    println!("WinPE Agent Server Started. Waiting for COM port...");

    let mut file = loop {
        match open_com_handle(&device_path) {
            Ok(handle) => break handle,
            Err(err) => {
                eprintln!("Failed to open {}: {}. Retrying...", device_path, err);
                thread::sleep(Duration::from_millis(500));
            }
        }
    };

    println!("Connected to host via {}.", device_path);

    let mut pending = Vec::new();
    let mut buffer = [0u8; 1024];

    loop {
        let bytes_read = match file.read(&mut buffer) {
            Ok(count) => count,
            Err(err) => {
                eprintln!("Read failed: {}. Retrying...", err);
                thread::sleep(Duration::from_millis(200));
                continue;
            }
        };

        if bytes_read == 0 {
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        pending.extend_from_slice(&buffer[..bytes_read]);
        while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
            let line = pending.drain(..=pos).collect::<Vec<u8>>();
            let trimmed = line
                .iter()
                .copied()
                .filter(|b| *b != b'\n' && *b != b'\r')
                .collect::<Vec<u8>>();
            if trimmed.is_empty() {
                continue;
            }

            let response = match serde_json::from_slice::<JsonRpcRequest>(&trimmed) {
                Ok(request) => handle_request(request),
                Err(err) => JsonRpcResponse::error(Value::Null, -32600, err.to_string()),
            };

            let payload = serde_json::to_vec(&response).unwrap_or_else(|_| b"{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-32000,\"message\":\"serialize failed\"},\"id\":null}".to_vec());
            if let Err(err) = write_line(&mut file, &payload) {
                eprintln!("Write failed: {}", err);
            }
        }
    }
}

fn open_com_handle(path: &str) -> io::Result<std::fs::File> {
    OpenOptions::new().read(true).write(true).open(path)
}

fn write_line(file: &mut std::fs::File, payload: &[u8]) -> io::Result<()> {
    let mut buffer = payload.to_vec();
    buffer.push(b'\n');

    file.write_all(&buffer)?;
    file.flush()?;
    Ok(())
}

fn handle_request(request: JsonRpcRequest) -> JsonRpcResponse {
    if request.jsonrpc != "2.0" {
        return JsonRpcResponse::error(request.id, -32600, "Invalid jsonrpc version");
    }

    match request.method.as_str() {
        "ping" => JsonRpcResponse::success(request.id, Value::String("pong".to_string())),
        "cmd" => handle_cmd(request.id, request.params),
        _ => JsonRpcResponse::error(request.id, -32601, "Method not found"),
    }
}

fn handle_cmd(id: Value, params: Option<Value>) -> JsonRpcResponse {
    let args = match params {
        Some(Value::Array(values)) => {
            let mut out = Vec::new();
            for value in values {
                if let Value::String(text) = value {
                    out.push(text);
                } else {
                    return JsonRpcResponse::error(id, -32602, "Params must be string array");
                }
            }
            out
        }
        _ => return JsonRpcResponse::error(id, -32602, "Params must be string array"),
    };

    let output = Command::new("cmd").args(args).output();
    match output {
        Ok(output) => {
            let result = serde_json::json!({
                "status": output.status.code().unwrap_or(-1),
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr)
            });
            JsonRpcResponse::success(id, result)
        }
        Err(err) => JsonRpcResponse::error(id, -32000, err.to_string()),
    }
}
