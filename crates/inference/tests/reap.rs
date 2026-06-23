//! Startup reap (`03 §6`): on launch the supervisor must kill a stray `llama-server`
//! a prior run left behind — but **only** that, never an unrelated process that
//! recycled the pid. We prove both halves with a live stand-in process (`ping`),
//! identified by its real image path, without needing a real `llama-server`.

#![cfg(windows)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use inference::supervisor::{reap_stray, reap_stray_any};

fn spawn_ping() -> std::process::Child {
    Command::new("ping")
        .args(["-n", "60", "127.0.0.1"])
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn ping stand-in")
}

fn wait_dead(pid: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while inference::process::pid_alive(pid) {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

fn tmp_pidfile(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ssv2c-pid-{tag}-{}.pid", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn reaps_a_matching_stray() {
    let mut child = spawn_ping();
    let pid = child.id();
    // The sentinel: the live process's own image path stands in for "our installed
    // llama-server.exe", so the match is exact.
    let exe = inference::process::image_path(pid).expect("ping image path");
    let pidfile = tmp_pidfile("match");
    std::fs::write(&pidfile, pid.to_string()).unwrap();

    assert!(
        reap_stray(&pidfile, &exe),
        "a matching stray should be reaped"
    );
    assert!(wait_dead(pid), "the reaped process should terminate");
    assert!(!pidfile.exists(), "the pidfile should be cleaned up");
    let _ = child.wait();
}

#[test]
fn reaps_a_matching_stray_from_any_owned_install_path() {
    let mut child = spawn_ping();
    let pid = child.id();
    let exe = inference::process::image_path(pid).expect("ping image path");
    let pidfile = tmp_pidfile("multi-match");
    std::fs::write(&pidfile, pid.to_string()).unwrap();

    let old_or_current_installs = vec![
        PathBuf::from(r"C:\definitely\not\our\llama-server.exe"),
        exe,
    ];
    assert!(
        reap_stray_any(&pidfile, &old_or_current_installs),
        "a pid matching any owned install path should be reaped"
    );
    assert!(wait_dead(pid), "the reaped process should terminate");
    assert!(!pidfile.exists(), "the pidfile should be cleaned up");
    let _ = child.wait();
}

#[test]
fn never_reaps_a_foreign_pid() {
    let mut child = spawn_ping();
    let pid = child.id();
    let pidfile = tmp_pidfile("foreign");
    std::fs::write(&pidfile, pid.to_string()).unwrap();

    // The recorded pid is alive, but its image path is NOT our expected exe → the
    // pid was recycled to an unrelated process; it must be left untouched.
    let foreign = PathBuf::from(r"C:\definitely\not\our\llama-server.exe");
    assert!(
        !reap_stray(&pidfile, &foreign),
        "a non-matching pid must never be killed"
    );
    assert!(
        inference::process::pid_alive(pid),
        "the unrelated process must survive the reap"
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&pidfile);
}
