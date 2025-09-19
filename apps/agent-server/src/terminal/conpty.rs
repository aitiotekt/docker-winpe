//! ConPTY (Windows Pseudo Console) bindings.
//!
//! Provides safe wrappers around Win32 ConPTY APIs.

#[cfg(windows)]
use std::ptr;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, FALSE, GetLastError, HANDLE};
#[cfg(windows)]
use windows_sys::Win32::System::Console::HPCON;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::*;

/// Spawn a process attached to a pseudo console.
#[cfg(windows)]
pub fn spawn_process(
    hpc: HPCON,
    command_line: &str,
    cwd: Option<&str>,
) -> Result<(HANDLE, u32), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    unsafe {
        // Initialize the attribute list
        let mut attr_list_size: usize = 0;
        InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut attr_list_size);

        let mut attr_list_buffer = vec![0u8; attr_list_size];
        let attr_list = attr_list_buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;

        if InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_list_size) == 0 {
            return Err(format!(
                "InitializeProcThreadAttributeList failed: {}",
                GetLastError()
            ));
        }

        // Set the pseudo console attribute
        // Note: HPCON is already a pointer type
        let hpc_ptr = hpc as *mut std::ffi::c_void;
        if UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            hpc_ptr,
            std::mem::size_of::<HPCON>(),
            ptr::null_mut(),
            ptr::null_mut(),
        ) == 0
        {
            DeleteProcThreadAttributeList(attr_list);
            return Err(format!(
                "UpdateProcThreadAttribute failed: {}",
                GetLastError()
            ));
        }

        // Set up STARTUPINFOEXW
        let mut si: STARTUPINFOEXW = std::mem::zeroed();
        si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        si.lpAttributeList = attr_list;

        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();

        // Convert command line to wide string
        let mut cmd_wide: Vec<u16> = OsStr::new(command_line)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // Working directory
        let cwd_wide: Option<Vec<u16>> = cwd.map(|c| {
            OsStr::new(c)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        });
        let cwd_ptr = cwd_wide.as_ref().map_or(ptr::null(), |v| v.as_ptr());

        let result = CreateProcessW(
            ptr::null(),
            cmd_wide.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            FALSE,
            EXTENDED_STARTUPINFO_PRESENT,
            ptr::null(),
            cwd_ptr,
            &si.StartupInfo,
            &mut pi,
        );

        DeleteProcThreadAttributeList(attr_list);

        if result == 0 {
            return Err(format!("CreateProcessW failed: {}", GetLastError()));
        }

        CloseHandle(pi.hThread);
        Ok((pi.hProcess, pi.dwProcessId))
    }
}

#[cfg(not(windows))]
pub fn spawn_process(
    _hpc: (),
    _command_line: &str,
    _cwd: Option<&str>,
) -> Result<((), u32), String> {
    Err("spawn_process is only available on Windows".to_string())
}
