//! Windows **Job Object** binding — the no-orphan guarantee (`03 §6`, DoD #7).
//!
//! A job created here carries `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. While the
//! [`JobObject`] handle is open the job lives; when the *last* handle to the job
//! closes — which the OS does for **every** handle a process owns when that process
//! dies for any reason (clean exit, panic, `TerminateProcess`, power loss after
//! resume) — the kernel terminates every process still assigned to the job. Binding
//! the `llama-server` child to this job therefore makes an orphaned sidecar
//! impossible: if the app goes, the child goes with it.
//!
//! The assignment must happen **before** the child's main thread is resumed (see
//! [`crate::process`]), so there is never a window in which a running child is
//! unbound.

use std::mem::size_of;

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

/// An owned Windows Job Object configured to kill its members when its last handle
/// closes. Dropping the [`JobObject`] closes the handle (and, being the last handle,
/// kills any still-assigned process) — so the lifecycle is RAII-correct.
pub struct JobObject {
    handle: HANDLE,
}

// A job handle is just a kernel handle; sending it across threads is sound (the OS
// owns the object, not the thread). The supervisor holds it behind its own sync.
unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}

impl JobObject {
    /// Creates an unnamed job object and arms `KILL_ON_JOB_CLOSE`.
    pub fn new() -> Result<Self> {
        // SAFETY: a null name + null attributes is a valid call; it returns either a
        // fresh owned handle or an error. We take ownership and close it in `Drop`.
        let handle =
            unsafe { CreateJobObjectW(None, PCWSTR::null()) }.context("CreateJobObjectW failed")?;
        let job = Self { handle };
        job.arm_kill_on_close()?;
        Ok(job)
    }

    /// Sets `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` on the job (the whole point).
    fn arm_kill_on_close(&self) -> Result<()> {
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ptr = std::ptr::addr_of!(info).cast::<core::ffi::c_void>();
        // SAFETY: `info` is a fully-initialized struct of exactly the size we pass,
        // and `JobObjectExtendedLimitInformation` is its matching info class. The
        // pointer is read-only for the duration of the call, and `info` outlives it.
        unsafe {
            SetInformationJobObject(
                self.handle,
                JobObjectExtendedLimitInformation,
                ptr,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .context("SetInformationJobObject(KILL_ON_JOB_CLOSE) failed")
    }

    /// Assigns an already-created process to this job. **Must** be called before the
    /// process's main thread is resumed (`03 §6`).
    pub fn assign(&self, process: HANDLE) -> Result<()> {
        // SAFETY: both handles are valid kernel handles owned by the caller.
        unsafe { AssignProcessToJobObject(self.handle, process) }
            .context("AssignProcessToJobObject failed")
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            // SAFETY: `handle` came from `CreateJobObjectW` and has not been closed.
            // Closing the last handle is what triggers KILL_ON_JOB_CLOSE.
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
