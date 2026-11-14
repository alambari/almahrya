//! Finds the running daemon, or starts one.
//!
//! A started daemon outlives the app on purpose: state lives in the daemon, and closing
//! the window must not stop work in progress. It runs in its own process group (so a
//! Ctrl-C in the terminal that launched the app does not reach it), its stdin is
//! `/dev/null` and its output goes to the log files in the data directory, the same files
//! the launchd agent uses. A thread waits on it so it never lingers as a zombie while the
//! app runs; when the app exits, launchd adopts it.

use almahrya_client::{Client, ClientError};
use almahrya_local::LocalError;
use almahrya_local::paths::{self, Paths};
use almahrya_local::token_store;
use std::fs::{File, OpenOptions};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Overrides where the daemon binary is; by default it sits next to the app's executable.
pub const DAEMON_BIN_ENV: &str = "ALMAHRYA_DAEMON_BIN";
const DAEMON_BIN_NAME: &str = "almahryad";
/// How long a started daemon gets to answer `/v1/health`.
pub const START_TIMEOUT: Duration = Duration::from_secs(5);
/// How long one request may take before the daemon counts as not answering.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(50);
/// Exit code of a second instance: another start won the race, keep polling.
const ALREADY_RUNNING: i32 = 3;

/// A daemon that answered `/v1/health`, and a client for it.
#[derive(Debug, Clone)]
pub struct DaemonLink {
    client: Client,
    pid: u32,
    spawned: bool,
}

impl DaemonLink {
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The daemon's process id, from `/v1/health`.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Whether this call started the daemon (false when one was already running).
    pub fn spawned(&self) -> bool {
        self.spawned
    }
}

/// Why the app could not reach a daemon. Messages never contain the token.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error(transparent)]
    Local(#[from] LocalError),
    #[error("could not locate the app executable: {0}")]
    CurrentExe(#[source] std::io::Error),
    #[error("could not start {path}: {source}")]
    Spawn {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("almahryad exited during startup ({status}); see {log}")]
    Exited { status: String, log: PathBuf },
    #[error("almahryad did not answer within {0:?}")]
    Timeout(Duration),
    #[error(
        "almahryad accepts connections but is not answering; see {log}, or stop it so the app can start a new one"
    )]
    NotResponding { log: PathBuf },
}

/// How long the link waits for the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkOptions {
    /// Wall-clock limit for finding or starting the daemon.
    pub start_timeout: Duration,
    /// Limit for one request to the daemon.
    pub request_timeout: Duration,
}

impl Default for LinkOptions {
    fn default() -> Self {
        LinkOptions {
            start_timeout: START_TIMEOUT,
            request_timeout: REQUEST_TIMEOUT,
        }
    }
}

/// `$ALMAHRYA_DAEMON_BIN`, else `almahryad` next to the running executable.
pub fn daemon_binary() -> Result<PathBuf, LinkError> {
    if let Some(path) = std::env::var_os(DAEMON_BIN_ENV).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let exe = std::env::current_exe().map_err(LinkError::CurrentExe)?;
    Ok(exe.with_file_name(DAEMON_BIN_NAME))
}

/// Connects to the daemon at `paths`; if nothing answers on the socket, starts
/// `almahryad`, waits up to [`START_TIMEOUT`] for `/v1/health`, then connects.
pub async fn ensure_daemon(paths: &Paths) -> Result<DaemonLink, LinkError> {
    ensure_daemon_with(paths, &LinkOptions::default()).await
}

/// A request that failed or took too long.
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("almahryad did not answer within {0:?}")]
    TimedOut(Duration),
    #[error(transparent)]
    Client(#[from] ClientError),
}

/// Runs one client request, giving up after `limit`. The client itself has no timeout,
/// and a stopped or deadlocked daemon still accepts connections through the kernel
/// backlog, so every request the app makes goes through here.
pub async fn bounded<T>(
    limit: Duration,
    request: impl Future<Output = Result<T, ClientError>>,
) -> Result<T, RequestError> {
    match tokio::time::timeout(limit, request).await {
        Ok(result) => Ok(result?),
        Err(_) => Err(RequestError::TimedOut(limit)),
    }
}

/// Request limit capped by what is left of the start budget.
fn budget(options: &LinkOptions, start: Instant) -> Duration {
    let left = options.start_timeout.saturating_sub(start.elapsed());
    options
        .request_timeout
        .min(left)
        .max(Duration::from_millis(1))
}

/// [`ensure_daemon`] with explicit limits. Returns within about `start_timeout`.
pub async fn ensure_daemon_with(
    paths: &Paths,
    options: &LinkOptions,
) -> Result<DaemonLink, LinkError> {
    let start = Instant::now();
    let token = token_store::select()?.get_or_create()?;
    let client = Client::unix(paths.socket(), token.expose().to_owned());
    match bounded(budget(options, start), client.health()).await {
        Ok(health) => {
            return Ok(DaemonLink {
                client,
                pid: health.pid,
                spawned: false,
            });
        }
        // Something holds the socket but does not answer. Starting another daemon would
        // only fail on the instance lock, so wait for this one instead.
        Err(RequestError::TimedOut(_)) => {
            return wait_for_silent(client, paths, options, start).await;
        }
        // Nothing listens (no socket, or a stale one): start a daemon.
        Err(RequestError::Client(_)) => {}
    }

    let mut child = spawn(&daemon_binary()?, paths)?;
    let mut child_running = true;
    loop {
        // Until the timeout, any failure means "not listening yet".
        if let Ok(health) = bounded(budget(options, start), client.health()).await {
            reap(child);
            return Ok(DaemonLink {
                client,
                pid: health.pid,
                spawned: true,
            });
        }
        if child_running && let Ok(Some(status)) = child.try_wait() {
            if status.code() != Some(ALREADY_RUNNING) {
                return Err(LinkError::Exited {
                    status: status.to_string(),
                    log: paths::stderr_log(&paths.data_dir),
                });
            }
            child_running = false;
        }
        if start.elapsed() >= options.start_timeout {
            // Ours and not answering: do not leave it behind.
            let _ = child.kill();
            reap(child);
            return Err(LinkError::Timeout(options.start_timeout));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Keeps asking a daemon that accepts but does not answer, until the start budget ends.
async fn wait_for_silent(
    client: Client,
    paths: &Paths,
    options: &LinkOptions,
    start: Instant,
) -> Result<DaemonLink, LinkError> {
    loop {
        if start.elapsed() >= options.start_timeout {
            return Err(LinkError::NotResponding {
                log: paths::stderr_log(&paths.data_dir),
            });
        }
        tokio::time::sleep(POLL).await;
        if let Ok(health) = bounded(budget(options, start), client.health()).await {
            return Ok(DaemonLink {
                client,
                pid: health.pid,
                spawned: false,
            });
        }
    }
}

fn log_file(path: &Path) -> Result<File, LinkError> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| LocalError::io(format!("could not open {}", path.display()), e).into())
}

fn spawn(bin: &Path, paths: &Paths) -> Result<Child, LinkError> {
    paths::ensure_dir(&paths.data_dir)?;
    let stdout = log_file(&paths::stdout_log(&paths.data_dir))?;
    let stderr = log_file(&paths::stderr_log(&paths.data_dir))?;
    Command::new(bin)
        // The daemon must use exactly the locations the app connects to.
        .env(paths::DATA_DIR_ENV, &paths.data_dir)
        .env(paths::CONFIG_ENV, &paths.config_file)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0)
        .spawn()
        .map_err(|source| LinkError::Spawn {
            path: bin.to_owned(),
            source,
        })
}

/// Collects the exit status whenever the daemon ends, so it never stays a zombie.
fn reap(mut child: Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}
