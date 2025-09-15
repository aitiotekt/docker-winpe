use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tiny_http::{Method, Response, Server};
use winpe_agent_core::{JsonRpcRequest, JsonRpcResponse};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let http_addr = std::env::var("AGENT_HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let serial = connect_serial()?;
    let serial = Arc::new(Mutex::new(serial));

    println!("WinPE Agent Bridge listening on http://{}", http_addr);
    println!("Serial: {}", serial_label());

    let server = Server::http(http_addr)?;
    for mut request in server.incoming_requests() {
        if request.method() != &Method::Post || request.url() != "/jsonrpc" {
            let response = Response::from_string("Not Found").with_status_code(404);
            let _ = request.respond(response);
            continue;
        }

        let mut body = Vec::new();
        if let Err(err) = request.as_reader().read_to_end(&mut body) {
            let response = Response::from_string(err.to_string()).with_status_code(400);
            let _ = request.respond(response);
            continue;
        }

        let response = match serde_json::from_slice::<JsonRpcRequest>(&body) {
            Ok(rpc) => forward_request(&serial, rpc),
            Err(err) => JsonRpcResponse::error(Value::Null, -32600, err.to_string()),
        };

        let payload = serde_json::to_string(&response).unwrap_or_else(|_| {
            "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-32000,\"message\":\"serialize failed\"},\"id\":null}".to_string()
        });
        let response = Response::from_string(payload)
            .with_status_code(200)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .unwrap(),
            );
        let _ = request.respond(response);
    }

    Ok(())
}

fn forward_request(
    serial: &Arc<Mutex<Box<dyn ReadWrite + Send>>>,
    rpc: JsonRpcRequest,
) -> JsonRpcResponse {
    if rpc.jsonrpc != "2.0" {
        return JsonRpcResponse::error(rpc.id, -32600, "Invalid jsonrpc version");
    }

    let payload = match serde_json::to_vec(&rpc) {
        Ok(data) => data,
        Err(err) => return JsonRpcResponse::error(rpc.id, -32603, err.to_string()),
    };

    let mut guard = match serial.lock() {
        Ok(lock) => lock,
        Err(_) => return JsonRpcResponse::error(rpc.id, -32000, "Serial lock poisoned"),
    };

    let stream: &mut dyn ReadWrite = &mut **guard;
    if let Err(err) = write_line(stream, &payload) {
        return JsonRpcResponse::error(rpc.id, -32000, err.to_string());
    }

    match read_line(stream) {
        Ok(line) => match serde_json::from_slice::<JsonRpcResponse>(&line) {
            Ok(response) => response,
            Err(err) => JsonRpcResponse::error(rpc.id, -32603, err.to_string()),
        },
        Err(err) => JsonRpcResponse::error(rpc.id, -32000, err.to_string()),
    }
}

fn write_line(stream: &mut dyn ReadWrite, payload: &[u8]) -> io::Result<()> {
    stream.write_all(payload)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

fn read_line(stream: &mut dyn ReadWrite) -> io::Result<Vec<u8>> {
    let mut pending = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "serial closed",
            ));
        }
        pending.extend_from_slice(&buffer[..bytes_read]);
        if let Some(pos) = pending.iter().position(|b| *b == b'\n') {
            let line = pending.drain(..=pos).collect::<Vec<u8>>();
            let trimmed = line
                .iter()
                .copied()
                .filter(|b| *b != b'\n' && *b != b'\r')
                .collect::<Vec<u8>>();
            return Ok(trimmed);
        }
    }
}

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

#[cfg(unix)]
fn connect_serial() -> io::Result<Box<dyn ReadWrite + Send>> {
    use std::os::unix::net::UnixStream;

    if let Ok(path) = std::env::var("AGENT_SERIAL_SOCKET") {
        let stream = UnixStream::connect(&path)?;
        stream.set_nonblocking(false)?;
        return Ok(Box::new(stream));
    }

    if let Ok(addr) = std::env::var("AGENT_SERIAL_TCP") {
        let stream = TcpStream::connect(addr)?;
        return Ok(Box::new(stream));
    }

    let stream = UnixStream::connect("/tmp/qemu-agent.sock")?;
    stream.set_nonblocking(false)?;
    Ok(Box::new(stream))
}

#[cfg(windows)]
fn connect_serial() -> io::Result<Box<dyn ReadWrite + Send>> {
    let addr = std::env::var("AGENT_SERIAL_TCP").unwrap_or_else(|_| "127.0.0.1:5555".to_string());
    let stream = TcpStream::connect(addr)?;
    Ok(Box::new(stream))
}

#[cfg(unix)]
fn serial_label() -> String {
    if let Ok(path) = std::env::var("AGENT_SERIAL_SOCKET") {
        return format!("unix:{}", path);
    }
    if let Ok(addr) = std::env::var("AGENT_SERIAL_TCP") {
        return format!("tcp:{}", addr);
    }
    "unix:/tmp/qemu-agent.sock".to_string()
}

#[cfg(windows)]
fn serial_label() -> String {
    let addr = std::env::var("AGENT_SERIAL_TCP").unwrap_or_else(|_| "127.0.0.1:5555".to_string());
    format!("tcp:{}", addr)
}
