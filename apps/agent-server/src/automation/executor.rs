//! Command execution using Win32 CreateProcessW.
//!
//! Supports both synchronous execution (for /automation/exec)
//! and streaming execution (for /automation/exec_stream).

use std::time::Duration;
use tokio::sync::mpsc;
use winpe_agent_core::{ExecRequest, Shell};

/// Errors that can occur during command execution.
#[derive(Debug)]
pub enum ExecError {
    /// Process exceeded timeout.
    Timeout,
    /// Failed to create process.
    ProcessCreationFailed(String),
    /// Feature not supported on this platform.
    #[allow(dead_code)]
    NotSupported(String),
}

/// Events emitted during streaming execution.
#[derive(Debug)]
pub enum StreamEvent {
    /// Stdout data chunk.
    Stdout(String),
    /// Stderr data chunk.
    Stderr(String),
    /// Process exited with code.
    Exit(i32),
    /// Process was killed due to timeout.
    Timeout,
}

/// Execute a command and return captured output.
pub async fn execute_command(req: &ExecRequest) -> Result<(i32, String, String), ExecError> {
    #[cfg(windows)]
    {
        // Clone the request data for the blocking task
        let command = req.command.clone();
        let args = req.args.clone();
        let shell = req.shell;
        let cwd = req.cwd.clone();
        let env = req.env.clone();
        let timeout_ms = req.timeout_ms;

        // Run all Windows operations in a blocking task
        tokio::task::spawn_blocking(move || {
            execute_command_sync(&command, &args, shell, cwd.as_deref(), &env, timeout_ms)
        })
        .await
        .map_err(|e| ExecError::ProcessCreationFailed(format!("Task join error: {}", e)))?
    }
    #[cfg(not(windows))]
    {
        let _ = req;
        Err(ExecError::NotSupported(
            "Command execution only supported on Windows".to_string(),
        ))
    }
}

/// Execute a command with streaming output.
pub async fn execute_command_stream(
    req: &ExecRequest,
) -> Result<mpsc::Receiver<StreamEvent>, ExecError> {
    #[cfg(windows)]
    {
        execute_command_stream_windows(req).await
    }
    #[cfg(not(windows))]
    {
        let _ = req;
        Err(ExecError::NotSupported(
            "Command execution only supported on Windows".to_string(),
        ))
    }
}

#[cfg(windows)]
fn execute_command_sync(
    command: &str,
    args: &[String],
    shell: Shell,
    cwd: Option<&str>,
    env: &std::collections::HashMap<String, String>,
    timeout_ms: u64,
) -> Result<(i32, String, String), ExecError> {
    use std::ffi::OsStr;
    use std::io::Read;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use std::ptr;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Security::*;
    use windows_sys::Win32::System::Pipes::*;
    use windows_sys::Win32::System::Threading::*;

    // Job Objects FFI definitions (not always available in windows-sys)
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x00002000;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct IO_COUNTERS {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        basic_limit_information: JOBOBJECT_BASIC_LIMIT_INFORMATION,
        io_info: IO_COUNTERS,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(
            lpJobAttributes: *mut SECURITY_ATTRIBUTES,
            lpName: *const u16,
        ) -> HANDLE;
        fn SetInformationJobObject(
            hJob: HANDLE,
            JobObjectInformationClass: i32,
            lpJobObjectInformation: *const std::ffi::c_void,
            cbJobObjectInformationLength: u32,
        ) -> i32;
        fn AssignProcessToJobObject(hJob: HANDLE, hProcess: HANDLE) -> i32;
        fn TerminateJobObject(hJob: HANDLE, uExitCode: u32) -> i32;
    }

    // Build command line
    let command_line = build_command_line(command, args, shell);
    let mut command_line_wide: Vec<u16> = OsStr::new(&command_line)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Set up security attributes for inheritable handles
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: TRUE,
    };

    // Create pipes for stdout and stderr
    let mut stdout_read: HANDLE = INVALID_HANDLE_VALUE;
    let mut stdout_write: HANDLE = INVALID_HANDLE_VALUE;
    let mut stderr_read: HANDLE = INVALID_HANDLE_VALUE;
    let mut stderr_write: HANDLE = INVALID_HANDLE_VALUE;

    unsafe {
        if CreatePipe(&mut stdout_read, &mut stdout_write, &sa, 0) == 0 {
            return Err(ExecError::ProcessCreationFailed(
                "Failed to create stdout pipe".to_string(),
            ));
        }
        SetHandleInformation(stdout_read, HANDLE_FLAG_INHERIT, 0);

        if CreatePipe(&mut stderr_read, &mut stderr_write, &sa, 0) == 0 {
            CloseHandle(stdout_read);
            CloseHandle(stdout_write);
            return Err(ExecError::ProcessCreationFailed(
                "Failed to create stderr pipe".to_string(),
            ));
        }
        SetHandleInformation(stderr_read, HANDLE_FLAG_INHERIT, 0);
    }

    // Set up STARTUPINFOW
    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = stdout_write;
    si.hStdError = stderr_write;
    si.hStdInput = INVALID_HANDLE_VALUE;

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    // Working directory
    let cwd_wide: Option<Vec<u16>> = cwd.map(|c| {
        OsStr::new(c)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    });
    let cwd_ptr = cwd_wide.as_ref().map_or(ptr::null(), |v| v.as_ptr());

    // Build environment block
    let env_block = build_environment_block(env);
    let env_ptr = env_block.as_ptr();

    // Create process with environment (CREATE_UNICODE_ENVIRONMENT for wide chars)
    let result = unsafe {
        CreateProcessW(
            ptr::null(),
            command_line_wide.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            TRUE,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            env_ptr as *const std::ffi::c_void,
            cwd_ptr,
            &si,
            &mut pi,
        )
    };

    // Close write ends of pipes (child owns them now)
    unsafe {
        CloseHandle(stdout_write);
        CloseHandle(stderr_write);
    }

    if result == 0 {
        unsafe {
            CloseHandle(stdout_read);
            CloseHandle(stderr_read);
        }
        return Err(ExecError::ProcessCreationFailed(format!(
            "CreateProcessW failed with error {}",
            unsafe { GetLastError() }
        )));
    }

    unsafe { CloseHandle(pi.hThread) };

    // Create Job Object for process tree termination
    let job = unsafe { CreateJobObjectW(ptr::null_mut(), ptr::null()) };
    if job.is_null() || job == INVALID_HANDLE_VALUE {
        // Job creation failed, fallback to simple process termination
        tracing::warn!("Failed to create Job Object, process tree termination disabled");
    } else {
        // Configure job to terminate child processes when job is closed
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let set_result = unsafe {
            SetInformationJobObject(
                job,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };

        if set_result == 0 {
            tracing::warn!("Failed to configure Job Object limits");
        }

        // Assign process to job
        let assign_result = unsafe { AssignProcessToJobObject(job, pi.hProcess) };
        if assign_result == 0 {
            tracing::warn!("Failed to assign process to Job Object");
        }
    }

    // Wait for process with timeout (blocking)
    let timeout = Duration::from_millis(timeout_ms);
    let start = Instant::now();

    loop {
        let wait_result = unsafe { WaitForSingleObject(pi.hProcess, 100) };
        if wait_result == WAIT_OBJECT_0 {
            break;
        }
        if start.elapsed() > timeout {
            // Terminate job (kills entire process tree) or fallback to process
            if !job.is_null() && job != INVALID_HANDLE_VALUE {
                unsafe {
                    TerminateJobObject(job, 1);
                    CloseHandle(job);
                }
            } else {
                unsafe {
                    TerminateProcess(pi.hProcess, 1);
                }
            }
            unsafe {
                CloseHandle(pi.hProcess);
                CloseHandle(stdout_read);
                CloseHandle(stderr_read);
            }
            return Err(ExecError::Timeout);
        }
    }

    // Clean up job object
    if !job.is_null() && job != INVALID_HANDLE_VALUE {
        unsafe { CloseHandle(job) };
    }

    // Get exit code
    let mut exit_code: u32 = 0;
    unsafe {
        GetExitCodeProcess(pi.hProcess, &mut exit_code);
        CloseHandle(pi.hProcess);
    }

    // Read stdout
    let mut stdout_file = unsafe { std::fs::File::from_raw_handle(stdout_read) };
    let mut stdout_buf = Vec::new();
    let _ = stdout_file.read_to_end(&mut stdout_buf);
    let stdout = String::from_utf8_lossy(&stdout_buf).to_string();

    // Read stderr
    let mut stderr_file = unsafe { std::fs::File::from_raw_handle(stderr_read) };
    let mut stderr_buf = Vec::new();
    let _ = stderr_file.read_to_end(&mut stderr_buf);
    let stderr = String::from_utf8_lossy(&stderr_buf).to_string();

    Ok((exit_code as i32, stdout, stderr))
}

#[cfg(windows)]
async fn execute_command_stream_windows(
    req: &ExecRequest,
) -> Result<mpsc::Receiver<StreamEvent>, ExecError> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Security::*;
    use windows_sys::Win32::System::Pipes::*;
    use windows_sys::Win32::System::Threading::*;

    let (tx, rx) = mpsc::channel(100);

    // Build command line
    let command_line = build_command_line(&req.command, &req.args, req.shell);
    let mut command_line_wide: Vec<u16> = OsStr::new(&command_line)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Set up security attributes for inheritable handles
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: TRUE,
    };

    // Create pipes for stdout and stderr
    let mut stdout_read: HANDLE = INVALID_HANDLE_VALUE;
    let mut stdout_write: HANDLE = INVALID_HANDLE_VALUE;
    let mut stderr_read: HANDLE = INVALID_HANDLE_VALUE;
    let mut stderr_write: HANDLE = INVALID_HANDLE_VALUE;

    unsafe {
        if CreatePipe(&mut stdout_read, &mut stdout_write, &sa, 0) == 0 {
            return Err(ExecError::ProcessCreationFailed(
                "Failed to create stdout pipe".to_string(),
            ));
        }
        SetHandleInformation(stdout_read, HANDLE_FLAG_INHERIT, 0);

        if CreatePipe(&mut stderr_read, &mut stderr_write, &sa, 0) == 0 {
            CloseHandle(stdout_read);
            CloseHandle(stdout_write);
            return Err(ExecError::ProcessCreationFailed(
                "Failed to create stderr pipe".to_string(),
            ));
        }
        SetHandleInformation(stderr_read, HANDLE_FLAG_INHERIT, 0);
    }

    // Set up STARTUPINFOW
    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = stdout_write;
    si.hStdError = stderr_write;
    si.hStdInput = INVALID_HANDLE_VALUE;

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    // Working directory
    let cwd = req.cwd.clone();
    let cwd_wide: Option<Vec<u16>> = cwd.as_ref().map(|c| {
        OsStr::new(c)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    });
    let cwd_ptr = cwd_wide.as_ref().map_or(ptr::null(), |v| v.as_ptr());

    // Build environment block
    let env_block = build_environment_block(&req.env);
    let env_ptr = env_block.as_ptr();

    // Create process with environment
    let result = unsafe {
        CreateProcessW(
            ptr::null(),
            command_line_wide.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            TRUE,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            env_ptr as *const std::ffi::c_void,
            cwd_ptr,
            &si,
            &mut pi,
        )
    };

    // Close write ends of pipes
    unsafe {
        CloseHandle(stdout_write);
        CloseHandle(stderr_write);
    }

    if result == 0 {
        unsafe {
            CloseHandle(stdout_read);
            CloseHandle(stderr_read);
        }
        return Err(ExecError::ProcessCreationFailed(format!(
            "CreateProcessW failed with error {}",
            unsafe { GetLastError() }
        )));
    }

    unsafe { CloseHandle(pi.hThread) };

    // Store handle values as usize for Send safety
    let process_handle = pi.hProcess as usize;
    let stdout_read_val = stdout_read as usize;
    let stderr_read_val = stderr_read as usize;
    let timeout_ms = req.timeout_ms;

    // Spawn task to read and stream output using std::thread
    let tx_stdout = tx.clone();
    std::thread::spawn(move || {
        stream_pipe(stdout_read_val as HANDLE, tx_stdout, true);
    });

    let tx_stderr = tx.clone();
    std::thread::spawn(move || {
        stream_pipe(stderr_read_val as HANDLE, tx_stderr, false);
    });

    // Spawn a thread to wait for process and send exit event
    std::thread::spawn(move || {
        let timeout = Duration::from_millis(timeout_ms);
        let start = std::time::Instant::now();
        let handle = process_handle as HANDLE;
        let mut timed_out = false;

        loop {
            let result = unsafe { WaitForSingleObject(handle, 100) };
            if result == WAIT_OBJECT_0 {
                break;
            }
            if start.elapsed() > timeout {
                unsafe {
                    TerminateProcess(handle, 1);
                }
                timed_out = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        unsafe {
            CloseHandle(handle);
        }

        // Send appropriate event based on whether we timed out
        if timed_out {
            let _ = tx.blocking_send(StreamEvent::Timeout);
        } else {
            let mut exit_code: u32 = 0;
            unsafe {
                GetExitCodeProcess(handle, &mut exit_code);
            }
            let _ = tx.blocking_send(StreamEvent::Exit(exit_code as i32));
        }
    });

    Ok(rx)
}

#[cfg(windows)]
fn stream_pipe(
    handle: windows_sys::Win32::Foundation::HANDLE,
    tx: mpsc::Sender<StreamEvent>,
    is_stdout: bool,
) {
    use std::io::Read;
    use std::os::windows::io::FromRawHandle;

    let mut file = unsafe { std::fs::File::from_raw_handle(handle) };
    let mut buffer = [0u8; 4096];

    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
                let event = if is_stdout {
                    StreamEvent::Stdout(chunk)
                } else {
                    StreamEvent::Stderr(chunk)
                };
                if tx.blocking_send(event).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

/// Build command line string from request.
fn build_command_line(command: &str, args: &[String], shell: Shell) -> String {
    match shell {
        Shell::Cmd => {
            let mut cmd = format!("cmd.exe /c {}", command);
            for arg in args {
                cmd.push(' ');
                if arg.contains(' ') {
                    cmd.push('"');
                    cmd.push_str(arg);
                    cmd.push('"');
                } else {
                    cmd.push_str(arg);
                }
            }
            cmd
        }
        Shell::Powershell => {
            let mut cmd = String::from("powershell.exe -NoLogo -NoProfile -Command ");
            cmd.push_str(command);
            for arg in args {
                cmd.push(' ');
                if arg.contains(' ') {
                    cmd.push('"');
                    cmd.push_str(arg);
                    cmd.push('"');
                } else {
                    cmd.push_str(arg);
                }
            }
            cmd
        }
    }
}

/// Build environment block for CreateProcessW.
/// The block is a null-terminated sequence of null-terminated "KEY=VALUE" strings.
#[cfg(windows)]
fn build_environment_block(extra_env: &std::collections::HashMap<String, String>) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let mut env_strings: Vec<String> = Vec::new();

    // Inherit current environment
    for (key, value) in std::env::vars() {
        env_strings.push(format!("{}={}", key, value));
    }

    // Add/override with request environment
    for (key, value) in extra_env {
        // Remove any existing entry with this key
        env_strings.retain(|s| !s.starts_with(&format!("{}=", key)));
        env_strings.push(format!("{}={}", key, value));
    }

    // Sort for consistency (optional but nice)
    env_strings.sort();

    // Build the block: each string null-terminated, then double null at end
    let mut block: Vec<u16> = Vec::new();
    for s in env_strings {
        block.extend(OsStr::new(&s).encode_wide());
        block.push(0); // null terminator for this string
    }
    block.push(0); // final null terminator

    block
}
