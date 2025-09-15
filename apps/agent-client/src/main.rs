use std::io::{self, Read, Write};
use std::net::TcpStream;

use serde_json::Value;
use winpe_agent_core::JsonRpcRequest;

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<String>>();
    let url = extract_arg(&mut args, "--url")
        .unwrap_or_else(|| "http://127.0.0.1:8080/jsonrpc".to_string());
    let method = extract_arg(&mut args, "--method").unwrap_or_else(|| "ping".to_string());

    let params = if let Some(param_json) = extract_arg(&mut args, "--params") {
        serde_json::from_str::<Value>(&param_json).unwrap_or(Value::Null)
    } else if !args.is_empty() {
        Value::Array(args.into_iter().map(Value::String).collect())
    } else {
        Value::Array(Vec::new())
    };

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method,
        params: Some(params),
        id: Value::from(1),
    };

    let response = send_request(&url, &request)?;
    println!("{}", response);
    Ok(())
}

fn extract_arg(args: &mut Vec<String>, key: &str) -> Option<String> {
    if let Some(pos) = args.iter().position(|arg| arg == key)
        && pos + 1 < args.len()
    {
        let value = args.remove(pos + 1);
        args.remove(pos);
        return Some(value);
    }
    None
}

fn send_request(url: &str, request: &JsonRpcRequest) -> io::Result<String> {
    let (host, port, path) = parse_url(url)?;
    let mut stream = TcpStream::connect((host.as_str(), port))?;
    let payload = serde_json::to_string(request).unwrap_or_else(|_| "{}".to_string());
    let request_line = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path,
        host,
        payload.len(),
        payload
    );

    stream.write_all(request_line.as_bytes())?;
    stream.flush()?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    let body = response.split("\r\n\r\n").nth(1).unwrap_or("");
    Ok(body.to_string())
}

fn parse_url(url: &str) -> io::Result<(String, u16, String)> {
    let trimmed = url
        .strip_prefix("http://")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Only http:// supported"))?;

    let mut parts = trimmed.splitn(2, '/');
    let host_port = parts.next().unwrap_or("127.0.0.1:8080");
    let path = format!("/{}", parts.next().unwrap_or("jsonrpc"));

    let mut host_parts = host_port.splitn(2, ':');
    let host = host_parts.next().unwrap_or("127.0.0.1").to_string();
    let port = host_parts
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);

    Ok((host, port, path))
}
