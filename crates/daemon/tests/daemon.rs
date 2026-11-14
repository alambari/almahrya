#![allow(clippy::unwrap_used)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_almahryad");

/// Temp dirs for one daemon: short prefix so the socket path stays under sun_path.
struct Env {
    dir: tempfile::TempDir,
    port: u16,
}

impl Env {
    fn new() -> Env {
        let dir = tempfile::Builder::new().prefix("ad").tempdir().unwrap();
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        std::fs::write(
            dir.path().join("config.toml"),
            format!("[api]\nport = {port}\n\n[future]\nunknown = true\n"),
        )
        .unwrap();
        Env { dir, port }
    }

    fn data_dir(&self) -> PathBuf {
        self.dir.path().join("data")
    }

    fn socket(&self) -> PathBuf {
        self.data_dir().join("almahryad.sock")
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(BIN);
        // A shell that set ALMAHRYA_PROFILE or another override must not change the test.
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("ALMAHRYA_") {
                cmd.env_remove(name);
            }
        }
        cmd.env("ALMAHRYA_CONFIG", self.dir.path().join("config.toml"))
            .env("ALMAHRYA_DATA_DIR", self.data_dir())
            .env("ALMAHRYA_TOKEN_FILE", self.dir.path().join("token"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    }

    fn spawn(&self) -> Daemon {
        Daemon(self.command().spawn().unwrap())
    }
}

/// Stops the daemon if a test fails before it does.
struct Daemon(Child);

impl Daemon {
    fn pid(&self) -> u32 {
        self.0.id()
    }

    fn wait_timeout(&mut self, limit: Duration) -> Option<ExitStatus> {
        let start = Instant::now();
        while start.elapsed() < limit {
            if let Some(status) = self.0.try_wait().unwrap() {
                return Some(status);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            sigterm(self.0.id());
            if self.wait_timeout(Duration::from_secs(2)).is_none() {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
}

fn sigterm(pid: u32) {
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .unwrap();
    assert!(status.success());
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

fn is_ok(response: Option<String>) -> bool {
    response.is_some_and(|r| r.starts_with("HTTP/1.1 200"))
}

/// Polls both transports until they answer 200 or ten seconds pass.
fn wait_healthy(env: &Env) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if is_ok(tcp_health(env.port)) && is_ok(unix_health(&env.socket())) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("daemon did not become healthy");
}

#[test]
fn serves_health_on_tcp_and_unix_socket() {
    let env = Env::new();
    let _daemon = env.spawn();
    wait_healthy(&env);
    assert!(tcp_health(env.port).unwrap().contains("\"status\":\"ok\""));
    assert!(
        unix_health(&env.socket())
            .unwrap()
            .contains("\"status\":\"ok\"")
    );
}

#[test]
fn second_instance_refuses() {
    let env = Env::new();
    let _a = env.spawn();
    wait_healthy(&env);

    let b: Output = env.command().output().unwrap();
    assert_eq!(b.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&b.stderr);
    assert!(stderr.contains("already running"), "{stderr}");

    assert!(is_ok(tcp_health(env.port)));
    assert!(is_ok(unix_health(&env.socket())));
}

#[test]
fn stale_socket_is_replaced() {
    let env = Env::new();
    std::fs::create_dir_all(env.data_dir()).unwrap();
    std::fs::write(env.socket(), b"leftover").unwrap();
    let _daemon = env.spawn();
    wait_healthy(&env);
}

#[test]
fn sigterm_exits_cleanly() {
    let env = Env::new();
    let mut daemon = env.spawn();
    wait_healthy(&env);

    sigterm(daemon.pid());
    let status = daemon
        .wait_timeout(Duration::from_secs(2))
        .expect("daemon did not exit within 2 s");
    assert_eq!(status.code(), Some(0));
    assert!(!env.socket().exists());
}

#[test]
fn install_agent_writes_plist_without_running_launchctl() {
    let env = Env::new();
    let plist_dir = env.dir.path().join("agents");
    std::fs::create_dir_all(&plist_dir).unwrap();

    // A fake launchctl first on PATH records any call.
    let bin_dir = env.dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let marker = env.dir.path().join("launchctl-called");
    let fake = bin_dir.join("launchctl");
    std::fs::write(&fake, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap());

    let out = env
        .command()
        .env("PATH", path)
        .arg("install-agent")
        .arg("--plist-dir")
        .arg(&plist_dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let plist_path = plist_dir.join("dev.almahrya.daemon.plist");
    let plist = std::fs::read_to_string(&plist_path).unwrap();
    assert!(plist.contains("<key>Label</key>\n\t<string>dev.almahrya.daemon</string>"));
    assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));
    assert!(plist.contains("<key>KeepAlive</key>\n\t<true/>"));
    let exe = std::fs::canonicalize(BIN).unwrap();
    assert!(plist.contains(&format!("<string>{}</string>", exe.display())));
    let data = env.data_dir();
    assert!(plist.contains(&format!("{}/almahryad.out.log", data.display())));
    assert!(plist.contains(&format!("{}/almahryad.err.log", data.display())));

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(&format!(
            "launchctl bootstrap gui/$(id -u) {}",
            plist_path.display()
        )),
        "{stdout}"
    );
    assert!(!marker.exists(), "install-agent must not run launchctl");
}
