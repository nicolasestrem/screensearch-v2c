//! Suspended child-process spawning for the Job-Object lifecycle (`03 §6`).
//!
//! A `llama-server` child is created **suspended** (`CREATE_SUSPENDED`) so the caller
//! can assign it to a [`crate::job_object::JobObject`] *before* its main thread runs
//! — closing the race in which a briefly-running child could orphan if the app died
//! between spawn and assign. We use raw `CreateProcessW` (not `std::process::Command`)
//! precisely because the standard library gives no way to spawn suspended and recover
//! the main-thread handle needed to resume.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW, ResumeThread,
    TerminateProcess, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED, PROCESS_INFORMATION,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, STARTUPINFOW,
};

/// `GetExitCodeProcess` reports this sentinel exit code while a process is running.
const STILL_ACTIVE: u32 = 259;

/// A child process created suspended: its main thread has not begun executing.
/// Assign [`process_handle`](Self::process_handle) to a job, then [`resume`](Self::resume).
/// Dropping closes the owned process/thread handles.
pub struct SuspendedChild {
    process: HANDLE,
    thread: HANDLE,
    pid: u32,
}

// The handles are kernel handles, safe to move between threads.
unsafe impl Send for SuspendedChild {}
unsafe impl Sync for SuspendedChild {}

impl SuspendedChild {
    /// The child's process handle (for `JobObject::assign` and waiting).
    pub fn process_handle(&self) -> HANDLE {
        self.process
    }

    /// The child's process id.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Resumes the suspended main thread. Call **only** after the process is assigned
    /// to its job (`03 §6`).
    pub fn resume(&self) -> Result<()> {
        // ResumeThread returns the previous suspend count, or u32::MAX on failure.
        // SAFETY: `thread` is a valid handle owned until `Drop`.
        let prev = unsafe { ResumeThread(self.thread) };
        if prev == u32::MAX {
            bail!("ResumeThread failed (child {})", self.pid);
        }
        Ok(())
    }

    /// Forcibly terminates this child (used to stop the sidecar on model-switch or
    /// idle-evict). The child is also in the Job Object, so this is a clean direct
    /// kill rather than relying on job teardown (which only happens on app exit).
    pub fn kill(&self) -> bool {
        // SAFETY: `process` is a valid handle with TERMINATE rights (we created it).
        unsafe { TerminateProcess(self.process, 1) }.is_ok()
    }
}

impl Drop for SuspendedChild {
    fn drop(&mut self) {
        // SAFETY: both handles were produced by CreateProcessW and not yet closed.
        unsafe {
            if !self.thread.is_invalid() {
                let _ = CloseHandle(self.thread);
            }
            if !self.process.is_invalid() {
                let _ = CloseHandle(self.process);
            }
        }
    }
}

/// Spawns `program args…` **suspended**, with no console window. The caller must
/// assign the returned child to a job before calling [`SuspendedChild::resume`].
pub fn spawn_suspended(program: &Path, args: &[String]) -> Result<SuspendedChild> {
    // CreateProcessW may mutate the command-line buffer in place, so it must be a
    // writable, NUL-terminated UTF-16 vector that outlives the call.
    let mut cmdline = build_command_line(program, args);

    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();

    // SAFETY: `cmdline` is writable + NUL-terminated and lives past the call;
    // `startup`/`info` point to valid stack storage. On success `info` is filled with
    // process/thread handles we take ownership of (closed in `SuspendedChild::drop`).
    unsafe {
        CreateProcessW(
            PCWSTR::null(),
            Some(PWSTR(cmdline.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_SUSPENDED | CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &startup,
            &mut info,
        )
    }
    .with_context(|| format!("CreateProcessW failed for {}", program.display()))?;

    Ok(SuspendedChild {
        process: info.hProcess,
        thread: info.hThread,
        pid: info.dwProcessId,
    })
}

/// Whether a process id is currently running. Used by the startup reap (`03 §6`).
pub fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: returns an owned handle or an error; nothing is dereferenced unsafely.
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => h,
        Err(_) => return false, // gone, or no access — treat as not-ours/not-alive
    };
    let mut code = 0u32;
    // SAFETY: `handle` is valid; `code` is a valid out-param.
    let alive = unsafe { GetExitCodeProcess(handle, &mut code) }.is_ok() && code == STILL_ACTIVE;
    // SAFETY: close the handle we opened.
    unsafe {
        let _ = CloseHandle(handle);
    }
    alive
}

/// Forcibly terminates a process id (best-effort; used by the startup reap to clear a
/// stray sidecar a prior run left behind). Returns `false` if the process could not
/// be opened or terminated.
pub fn terminate(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: returns an owned handle or an error.
    let handle = match unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) } {
        Ok(h) => h,
        Err(_) => return false,
    };
    // SAFETY: `handle` is a valid handle with TERMINATE access.
    let ok = unsafe { TerminateProcess(handle, 1) }.is_ok();
    unsafe {
        let _ = CloseHandle(handle);
    }
    ok
}

/// Blocks until `process` exits or `timeout_ms` elapses; `true` if it exited.
pub fn wait_for_exit(process: HANDLE, timeout_ms: u32) -> bool {
    // SAFETY: `process` is a valid process handle.
    unsafe { WaitForSingleObject(process, timeout_ms) == WAIT_OBJECT_0 }
}

/// The full image path of a running process id, or `None` if it can't be opened /
/// queried. The startup reap uses this as the sentinel: a stray pid is only ours — and
/// only then killed — if its image path is the `llama-server.exe` we installed under
/// app-data (`03 §6`; never kill an unrelated process).
pub fn image_path(pid: u32) -> Option<PathBuf> {
    if pid == 0 {
        return None;
    }
    // SAFETY: returns an owned handle or an error.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = vec![0u16; 1024];
    let mut size = buf.len() as u32;
    // SAFETY: `handle` is valid; `buf`/`size` are valid in/out params. On success
    // `size` is the number of UTF-16 code units written (excluding the NUL).
    let res = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
    };
    // SAFETY: close the handle we opened.
    unsafe {
        let _ = CloseHandle(handle);
    }
    res.ok()?;
    Some(PathBuf::from(String::from_utf16_lossy(
        &buf[..size as usize],
    )))
}

/// Builds a Windows command line (`program` + `args`) with correct argv quoting,
/// returned as a NUL-terminated, writable UTF-16 buffer for `CreateProcessW`.
fn build_command_line(program: &Path, args: &[String]) -> Vec<u16> {
    let mut line = quote_arg(&program.to_string_lossy());
    for a in args {
        line.push(' ');
        line.push_str(&quote_arg(a));
    }
    line.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Quotes one argument per the `CommandLineToArgvW` rules: wrap in quotes when it
/// contains whitespace or a quote, doubling the run of backslashes that precedes a
/// literal quote (and the closing quote).
fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                for _ in 0..backslashes * 2 + 1 {
                    out.push('\\');
                }
                backslashes = 0;
                out.push('"');
            }
            _ => {
                for _ in 0..backslashes {
                    out.push('\\');
                }
                backslashes = 0;
                out.push(c);
            }
        }
    }
    for _ in 0..backslashes * 2 {
        out.push('\\');
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::quote_arg;

    #[test]
    fn quotes_only_when_needed() {
        assert_eq!(quote_arg("simple"), "simple");
        assert_eq!(quote_arg("--ngl"), "--ngl");
        assert_eq!(quote_arg(""), "\"\"");
    }

    #[test]
    fn quotes_paths_with_spaces() {
        assert_eq!(
            quote_arg(r"C:\Program Files\llama\llama-server.exe"),
            r#""C:\Program Files\llama\llama-server.exe""#
        );
    }

    #[test]
    fn escapes_embedded_quotes_and_trailing_backslashes() {
        assert_eq!(quote_arg(r#"a"b"#), r#""a\"b""#);
        // A path ending in a backslash, wrapped in quotes, must double the backslash
        // so the closing quote isn't escaped.
        assert_eq!(quote_arg(r"C:\dir with space\"), r#""C:\dir with space\\""#);
    }
}
