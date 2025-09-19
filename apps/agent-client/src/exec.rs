//! exec mode: Execute a single command via Automation API.

use std::collections::HashMap;
use std::io::{self, Write};
use winpe_agent_core::{ExecRequest, ExecResponse, Shell};

pub async fn run(
    base_url: &str,
    token: Option<&str>,
    shell: &str,
    cwd: Option<&str>,
    timeout: u64,
    json_output: bool,
    command: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    if command.is_empty() {
        return Err("No command specified".into());
    }

    let shell_enum = match shell.to_lowercase().as_str() {
        "cmd" => Shell::Cmd,
        "powershell" | "pwsh" => Shell::Powershell,
        _ => return Err(format!("Unknown shell: {}", shell).into()),
    };

    let (cmd, args) = command.split_first().unwrap();

    let req = ExecRequest {
        shell: shell_enum,
        command: cmd.clone(),
        args: args.to_vec(),
        cwd: cwd.map(String::from),
        env: HashMap::new(),
        timeout_ms: timeout,
        encoding: "utf-8".to_string(),
    };

    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/automation/exec", base_url);

    let mut request = client.post(&url).json(&req);
    if let Some(t) = token {
        request = request.bearer_auth(t);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        return Err(format!("Request failed ({}): {}", status, body).into());
    }

    let result: ExecResponse = response.json().await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        // Print stdout to stdout
        if !result.stdout.is_empty() {
            io::stdout().write_all(result.stdout.as_bytes())?;
        }
        // Print stderr to stderr
        if !result.stderr.is_empty() {
            io::stderr().write_all(result.stderr.as_bytes())?;
        }
    }

    // Exit with the remote exit code
    std::process::exit(result.exit_code);
}
