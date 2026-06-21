//! Test helper for the no-orphan integration test (`tests/no_orphan.rs`).
//!
//! It plays the role of "the app": it creates a `KILL_ON_JOB_CLOSE` job, spawns a
//! long-lived grandchild bound to that job, prints the grandchild's PID on stdout,
//! then blocks forever. When the test forcibly kills *this* process, the OS closes
//! its handles — including the only handle to the job — and `KILL_ON_JOB_CLOSE`
//! terminates the grandchild. That is the real cross-process no-orphan proof a single
//! in-process handle-close test can't fully demonstrate.

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use std::io::Write;
    use std::path::PathBuf;

    use inference::job_object::JobObject;
    use inference::process::spawn_suspended;

    let job = JobObject::new()?;

    // A stable, always-present long-lived child: `ping -n 600 127.0.0.1` ≈ 10 min,
    // far longer than the test needs.
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let ping = PathBuf::from(format!(r"{system_root}\System32\ping.exe"));
    let child = spawn_suspended(
        &ping,
        &["-n".to_string(), "600".to_string(), "127.0.0.1".to_string()],
    )?;

    // Assign BEFORE resume (03 §6): no window in which the child runs unbound.
    job.assign(child.process_handle())?;
    child.resume()?;

    println!("GRANDCHILD_PID={}", child.pid());
    std::io::stdout().flush()?;

    // Hold the job + child handles open and block. The test kills us; the OS closing
    // our handles is what fires KILL_ON_JOB_CLOSE on the grandchild.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("jobhelper is Windows-only");
    std::process::exit(1);
}
