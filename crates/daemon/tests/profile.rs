#![allow(clippy::unwrap_used)]

//! The development and production profiles side by side.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_almahryad");
/// Tests here take two adjacent ports, and macOS hands out ephemeral ports in sequence,
/// so a parallel test could pick the other's `base + 1`. Daemon tests take turns.
static PORTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn port_turn() -> std::sync::MutexGuard<'static, ()> {
    PORTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

const DEV_REFUSAL: &str = "the development profile has no launchd agent; use scripts/dev.sh";

/// A base port whose successor is free as well, so the dev daemon can take `base + 1`.
fn free_port_pair() -> u16 {
    for _ in 0..50 {
        let base = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        if base < u16::MAX && TcpListener::bind(("127.0.0.1", base + 1)).is_ok() {
            return base;
        }
    }
    panic!("no free port pair");
}

/// Drops every inherited `ALMAHRYA_*` variable, so a shell that set one (for example
/// `ALMAHRYA_PROFILE=dev`) cannot change what the test runs.
fn clear_almahrya_env(cmd: &mut Command) {
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("ALMAHRYA_") {
            cmd.env_remove(name);
        }
    }
}

/// One config file shared by both profiles, and a data dir and token file per profile.
struct Env {
    dir: tempfile::TempDir,
    base: u16,
}

impl Env {
    fn new() -> Env {
        let dir = tempfile::Builder::new().prefix("ap").tempdir().unwrap();
        let base = free_port_pair();
        std::fs::write(
            dir.path().join("config.toml"),
            format!("[api]\nport = {base}\n"),
        )
        .unwrap();
        Env { dir, base }
    }

    fn data_dir(&self, profile: &str) -> PathBuf {
        self.dir.path().join(profile)
    }

    fn command(&self, profile: &str) -> Command {
        let mut cmd = Command::new(BIN);
        clear_almahrya_env(&mut cmd);
        cmd.env("ALMAHRYA_PROFILE", profile)
            // Nothing may fall back to the real home directory.
            .env("HOME", self.dir.path())
            .env("ALMAHRYA_CONFIG", self.dir.path().join("config.toml"))
            .env("ALMAHRYA_DATA_DIR", self.data_dir(profile))
            .env(
                "ALMAHRYA_TOKEN_FILE",
                self.dir.path().join(format!("{profile}.token")),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    }
}

/// Stops the daemon when the test ends, passed or failed.
struct Daemon(Child);

impl Daemon {
    /// Panics with the daemon's stderr if it already exited (for example, port taken).
    fn assert_running(&mut self) {
        if let Some(status) = self.0.try_wait().unwrap() {
            let mut err = String::new();
            if let Some(mut stderr) = self.0.stderr.take() {
                let _ = stderr.read_to_string(&mut err);
            }
            panic!("daemon exited ({status}): {err}");
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.0.id().to_string()])
            .status();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn health_request<S: Read + Write>(mut stream: S) -> String {
    stream
        .write_all(b"GET /v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut out = String::new();
    stream.read_to_string(&mut out).unwrap();
    out
}

fn tcp_health(port: u16) -> Option<String> {
    TcpStream::connect(("127.0.0.1", port))
        .ok()
        .map(health_request)
}

fn unix_health(path: &Path) -> Option<String> {
    UnixStream::connect(path).ok().map(health_request)
}

fn ok(response: &Option<String>) -> bool {
    response
        .as_deref()
        .is_some_and(|r| r.starts_with("HTTP/1.1 200"))
}

/// Both transports answer 200 within ten seconds; returns the TCP and socket responses.
fn wait_healthy(daemon: &mut Daemon, port: u16, socket: &Path) -> (String, String) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        daemon.assert_running();
        let (tcp, unix) = (tcp_health(port), unix_health(socket));
        if ok(&tcp) && ok(&unix) {
            return (tcp.unwrap(), unix.unwrap());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("daemon on port {port} did not become healthy");
}

fn pid_in(response: &str) -> String {
    let at = response.find("\"pid\":").unwrap() + "\"pid\":".len();
    response[at..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect()
}

#[test]
fn dev_and_production_daemons_run_side_by_side() {
    let _turn = port_turn();
    let env = Env::new();
    let mut prod = Daemon(env.command("production").spawn().unwrap());
    let mut dev = Daemon(env.command("dev").spawn().unwrap());

    let prod_socket = env.data_dir("production").join("almahryad.sock");
    let dev_socket = env.data_dir("dev").join("almahryad.sock");
    let (prod_tcp, prod_unix) = wait_healthy(&mut prod, env.base, &prod_socket);
    let (dev_tcp, dev_unix) = wait_healthy(&mut dev, env.base + 1, &dev_socket);

    let prod_pid = prod.0.id().to_string();
    let dev_pid = dev.0.id().to_string();
    assert_eq!(pid_in(&prod_tcp), prod_pid);
    assert_eq!(pid_in(&prod_unix), prod_pid);
    assert_eq!(pid_in(&dev_tcp), dev_pid);
    assert_eq!(pid_in(&dev_unix), dev_pid);

    // Each daemon keeps its database and socket inside its own directory, and neither
    // directory holds the other's files: the two are separate, not nested.
    let (prod_dir, dev_dir) = (env.data_dir("production"), env.data_dir("dev"));
    assert!(!prod_dir.starts_with(&dev_dir) && !dev_dir.starts_with(&prod_dir));
    for (own, own_pid, other_pid) in [
        (&prod_dir, &prod_pid, &dev_pid),
        (&dev_dir, &dev_pid, &prod_pid),
    ] {
        assert!(own.join("almahrya.db").is_file(), "{}", own.display());
        let socket = own.join("almahryad.sock");
        let reply = unix_health(&socket).unwrap();
        assert_eq!(&pid_in(&reply), own_pid, "{}", socket.display());
        assert_ne!(&pid_in(&reply), other_pid);
        let names = file_names(own);
        assert!(
            names.iter().all(|n| n.starts_with("almahrya")),
            "{} holds unexpected files: {names:?}",
            own.display()
        );
    }
    // Nothing was written next to the two directories except the shared config, the token
    // files and the directories themselves.
    let mut top = file_names(env.dir.path());
    top.sort();
    assert_eq!(
        top,
        [
            "config.toml",
            "dev",
            "dev.token",
            "production",
            "production.token"
        ]
    );
}

fn file_names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn dev_daemon_uses_the_dev_port_override() {
    let _turn = port_turn();
    let env = Env::new();
    let dev_port = loop {
        let port = free_port_pair();
        if port != env.base && port != env.base + 1 {
            break port;
        }
    };
    std::fs::write(
        env.dir.path().join("config.toml"),
        format!(
            "[api]\nport = {}\n\n[dev]\napi_port = {dev_port}\n",
            env.base
        ),
    )
    .unwrap();
    let mut dev = Daemon(env.command("dev").spawn().unwrap());
    let (tcp, _) = wait_healthy(
        &mut dev,
        dev_port,
        &env.data_dir("dev").join("almahryad.sock"),
    );
    let pid = dev.0.id().to_string();
    assert_eq!(pid_in(&tcp), pid);
    // Another test may own base + 1; it must not be this daemon.
    if let Some(other) = tcp_health(env.base + 1).filter(|r| r.contains("\"pid\":")) {
        assert_ne!(pid_in(&other), pid);
    }
}

fn install_agent(env: &Env, profile: &str) -> (Output, PathBuf) {
    let plist_dir = env.dir.path().join(format!("{profile}-agents"));
    let out = env
        .command(profile)
        .arg("install-agent")
        .arg("--plist-dir")
        .arg(&plist_dir)
        .output()
        .unwrap();
    (out, plist_dir.join("dev.almahrya.daemon.plist"))
}

#[test]
fn dev_profile_install_agent_refuses() {
    let env = Env::new();
    let (out, plist) = install_agent(&env, "dev");
    assert!(!out.status.success());
    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains(DEV_REFUSAL), "{stderr}");
    assert!(!plist.exists());
}

#[test]
fn profile_env_other_than_dev_is_production() {
    let env = Env::new();
    let (out, plist) = install_agent(&env, "staging");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(plist.exists());
}
