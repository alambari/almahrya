#![allow(clippy::unwrap_used)]

//! `ensure_daemon` reads its locations from the environment, and changing the environment
//! of a running process is `unsafe`. The test therefore runs itself again as a child
//! process with the variables set, and does the real work there.

use almahrya_app::daemon_link::ensure_daemon;
use almahrya_local::paths::Paths;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const CHILD_ENV: &str = "ALMAHRYA_TEST_ENSURE_DAEMON_CHILD";
const TEST_NAME: &str = "ensure_daemon_starts_one_when_none_runs_and_reuses_it";

/// Builds `almahryad` (the app must not depend on the daemon crate, so cargo does not
/// build it for this test) and returns its path.
fn daemon_bin() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "--quiet",
            "-p",
            "almahrya-daemon",
            "--bin",
            "almahryad",
        ])
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(status.success(), "building almahryad failed");
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    target.join("debug/almahryad")
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The part that runs in the child, with the environment prepared by the parent.
async fn scenario() {
    let paths = Paths::from_env().unwrap();
    let first = ensure_daemon(&paths).await.unwrap();
    println!("daemon-pid={}", first.pid());
    assert!(first.spawned(), "no daemon ran yet, so one must be started");
    first.client().list_sessions().await.unwrap();

    let second = ensure_daemon(&paths).await.unwrap();
    assert!(!second.spawned(), "the running daemon must be reused");
    assert_eq!(second.pid(), first.pid());
}

fn stop_daemon(pid: &str, socket: &Path) {
    let _ = Command::new("kill").args(["-TERM", pid]).status();
    let start = Instant::now();
    while socket.exists() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[tokio::test]
async fn ensure_daemon_starts_one_when_none_runs_and_reuses_it() {
    if std::env::var_os(CHILD_ENV).is_some() {
        scenario().await;
        return;
    }
    let bin = daemon_bin();
    // Short path: the socket must stay under the 103-byte sun_path limit.
    let dir = tempfile::Builder::new()
        .prefix("aapp")
        .tempdir_in("/private/tmp")
        .unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(&config, format!("[api]\nport = {}\n", free_port())).unwrap();
    let data_dir = dir.path().join("data");

    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env("ALMAHRYA_DAEMON_BIN", &bin)
        .env("ALMAHRYA_DATA_DIR", &data_dir)
        .env("ALMAHRYA_CONFIG", &config)
        .env("ALMAHRYA_TOKEN_FILE", dir.path().join("token"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // libtest prints the test name on the same line, so the marker is not at a line start.
    for rest in stdout.split("daemon-pid=").skip(1) {
        let pid: String = rest.chars().take_while(char::is_ascii_digit).collect();
        stop_daemon(&pid, &data_dir.join("almahryad.sock"));
    }
    assert!(
        output.status.success(),
        "child failed\n--- stdout\n{stdout}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("daemon-pid="), "{stdout}");
}
