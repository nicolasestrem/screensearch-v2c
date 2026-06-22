//! The no-orphan guarantee (`03 §6`, DoD #7): when the app process dies for **any**
//! reason, the OS terminates the Job-Object-bound sidecar. This proves the real
//! cross-process behavior — a `jobhelper` child holds a `KILL_ON_JOB_CLOSE` job plus
//! a grandchild; forcibly killing the helper (standing in for an app crash) must take
//! the grandchild down with it. No orphaned process, ever.
//!
//! This is the gating test for P4: it must pass before any real inference is shipped.

#![cfg(windows)]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn killing_parent_terminates_job_bound_child() {
    // The helper binary, built by Cargo alongside this integration test.
    let exe = env!("CARGO_BIN_EXE_jobhelper");

    let mut helper = Command::new(exe)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn jobhelper");

    // The helper prints the grandchild PID once it is bound to the job and resumed.
    let stdout = helper.stdout.take().expect("helper stdout piped");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read pid line");
    let pid: u32 = line
        .trim()
        .strip_prefix("GRANDCHILD_PID=")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("unexpected jobhelper output: {line:?}"));

    assert!(
        inference::process::pid_alive(pid),
        "grandchild {pid} should be running before the parent is killed"
    );

    // Simulate the app dying (crash / forced kill): terminate the helper. The OS
    // closes its handles, the last job handle closes, KILL_ON_JOB_CLOSE fires.
    helper.kill().expect("kill jobhelper");
    let _ = helper.wait();

    // The grandchild must die promptly. Poll up to 10 s for the kernel to reap it.
    let deadline = Instant::now() + Duration::from_secs(10);
    while inference::process::pid_alive(pid) {
        assert!(
            Instant::now() < deadline,
            "grandchild {pid} orphaned: it was still alive 10s after the parent was killed"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
