#![allow(clippy::unwrap_used)]

//! `ensure_daemon` reads its locations from the environment, and changing the environment
//! of a running process is `unsafe`. The test therefore runs itself again as a child
//! process with the variables set, and does the real work there.

use almahrya_app::daemon_link::{LinkError, LinkOptions, ensure_daemon, ensure_daemon_with};
use almahrya_local::paths::Paths;
use almahrya_local::profile::Profile;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// The dev test takes two adjacent ports, and macOS hands out ephemeral ports in
/// sequence, so a parallel test could pick its `base + 1`. Tests that start daemons take
/// turns.
static PORTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn port_turn() -> std::sync::MutexGuard<'static, ()> {
    PORTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

const CHILD_ENV: &str = "ALMAHRYA_TEST_ENSURE_DAEMON_CHILD";

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

/// A temp dir with a config on a free port, and the variables `ensure_daemon` reads.
struct ChildEnv {
    dir: tempfile::TempDir,
}

impl ChildEnv {
    fn new() -> ChildEnv {
        // Short path: the socket must stay under the 103-byte sun_path limit.
        let dir = tempfile::Builder::new()
            .prefix("aapp")
            .tempdir_in("/private/tmp")
            .unwrap();
        let config = dir.path().join("config.toml");
        std::fs::write(&config, format!("[api]\nport = {}\n", free_port())).unwrap();
        ChildEnv { dir }
    }

    fn data_dir(&self) -> PathBuf {
        self.dir.path().join("data")
    }

    /// Runs `test` again in a child process with the environment set.
    fn run(&self, test: &str, daemon_bin: &Path) -> Output {
        self.run_with(test, daemon_bin, &[])
    }

    /// [`ChildEnv::run`] with extra variables.
    fn run_with(&self, test: &str, daemon_bin: &Path, extra: &[(&str, String)]) -> Output {
        Command::new(std::env::current_exe().unwrap())
            .envs(extra.iter().map(|(k, v)| (*k, v.as_str())))
            .args(["--exact", test, "--nocapture", "--test-threads=1"])
            .env(CHILD_ENV, "1")
            .env("ALMAHRYA_DAEMON_BIN", daemon_bin)
            .env("ALMAHRYA_DATA_DIR", self.data_dir())
            .env("ALMAHRYA_CONFIG", self.dir.path().join("config.toml"))
            .env("ALMAHRYA_TOKEN_FILE", self.dir.path().join("token"))
            .output()
            .unwrap()
    }
}

fn assert_child_ok(output: &Output) {
    assert!(
        output.status.success(),
        "child failed\n--- stdout\n{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn ensure_daemon_starts_one_when_none_runs_and_reuses_it() {
    if std::env::var_os(CHILD_ENV).is_some() {
        scenario().await;
        return;
    }
    let bin = daemon_bin();
    let _turn = port_turn();
    let env = ChildEnv::new();
    let output = env.run(
        "ensure_daemon_starts_one_when_none_runs_and_reuses_it",
        &bin,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // libtest prints the test name on the same line, so the marker is not at a line start.
    for rest in stdout.split("daemon-pid=").skip(1) {
        let pid: String = rest.chars().take_while(char::is_ascii_digit).collect();
        stop_daemon(&pid, &env.data_dir().join("almahryad.sock"));
    }
    assert_child_ok(&output);
    assert!(stdout.contains("daemon-pid="), "{stdout}");
}

/// A process holds the socket, accepts connections and never replies (a stopped or
/// deadlocked daemon). `ensure_daemon` must give up in time and must not start another.
#[tokio::test]
async fn ensure_daemon_times_out_when_the_daemon_never_answers() {
    if std::env::var_os(CHILD_ENV).is_some() {
        silent_daemon_scenario().await;
        return;
    }
    let env = ChildEnv::new();
    // Stands in for almahryad: if it is ever started, the marker file appears.
    let marker = env.dir.path().join("spawned");
    let fake_bin = env.dir.path().join("fake-almahryad");
    std::fs::write(
        &fake_bin,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    Command::new("chmod")
        .arg("755")
        .arg(&fake_bin)
        .status()
        .unwrap();

    let output = env.run(
        "ensure_daemon_times_out_when_the_daemon_never_answers",
        &fake_bin,
    );
    assert_child_ok(&output);
    assert!(!marker.exists(), "a second daemon was started");
}

async fn silent_daemon_scenario() {
    let paths = Paths::from_env().unwrap();
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    let listener = std::os::unix::net::UnixListener::bind(paths.socket()).unwrap();
    // Accept and hold every connection without reading or writing.
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming() {
            held.push(stream);
        }
    });
    let options = LinkOptions {
        start_timeout: Duration::from_millis(600),
        request_timeout: Duration::from_millis(150),
    };
    let start = Instant::now();
    // The outer limit turns a hang into a failure.
    let result = tokio::time::timeout(Duration::from_secs(3), ensure_daemon_with(&paths, &options))
        .await
        .expect("ensure_daemon_with hung");
    let elapsed = start.elapsed();
    match result {
        Err(LinkError::NotResponding { log }) => {
            assert!(log.starts_with(&paths.data_dir), "{}", log.display());
        }
        other => panic!("expected NotResponding, got {other:?}"),
    }
    assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
}

const DEV_PORT_ENV: &str = "ALMAHRYA_TEST_DEV_PORT";

/// Runs in the child with `ALMAHRYA_PROFILE=dev`: the started daemon must be the dev one,
/// listening on the dev port (base + 1), not the production port.
async fn dev_profile_scenario() {
    let paths = Paths::from_env().unwrap();
    assert_eq!(paths.profile, Profile::Dev);
    let link = ensure_daemon(&paths).await.unwrap();
    println!("daemon-pid={}", link.pid());
    assert!(link.spawned());

    let dev_port: u16 = std::env::var(DEV_PORT_ENV).unwrap().parse().unwrap();
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", dev_port)).unwrap();
    stream
        .write_all(b"GET /v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(
        response.contains(&format!("\"pid\":{}", link.pid())),
        "{response}"
    );
    assert!(std::net::TcpStream::connect(("127.0.0.1", dev_port - 1)).is_err());
}

#[tokio::test]
async fn ensure_daemon_starts_the_dev_daemon_under_the_dev_profile() {
    if std::env::var_os(CHILD_ENV).is_some() {
        dev_profile_scenario().await;
        return;
    }
    let bin = daemon_bin();
    let _turn = port_turn();
    let env = ChildEnv::new();
    let base = loop {
        let port = free_port();
        if port < u16::MAX && std::net::TcpListener::bind(("127.0.0.1", port + 1)).is_ok() {
            break port;
        }
    };
    std::fs::write(
        env.dir.path().join("config.toml"),
        format!("[api]\nport = {base}\n"),
    )
    .unwrap();
    let output = env.run_with(
        "ensure_daemon_starts_the_dev_daemon_under_the_dev_profile",
        &bin,
        &[
            ("ALMAHRYA_PROFILE", "dev".to_owned()),
            (DEV_PORT_ENV, (base + 1).to_string()),
        ],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for rest in stdout.split("daemon-pid=").skip(1) {
        let pid: String = rest.chars().take_while(char::is_ascii_digit).collect();
        stop_daemon(&pid, &env.data_dir().join("almahryad.sock"));
    }
    assert_child_ok(&output);
    assert!(stdout.contains("daemon-pid="), "{stdout}");
}
