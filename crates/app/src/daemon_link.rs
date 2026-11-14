//! Finds the running daemon, or starts one.
//!
//! A started daemon outlives the app on purpose: state lives in the daemon, and closing
//! the window must not stop work in progress. It runs in its own process group (so a
//! Ctrl-C in the terminal that launched the app does not reach it), its stdin is
//! `/dev/null` and its output goes to the log files in the data directory, the same files
//! the launchd agent uses. A thread waits on it so it never lingers as a zombie while the
//! app runs; when the app exits, launchd adopts it.
//!
//! The app never creates the API token. The daemon creates it on its first start (it
//! holds the instance lock, so only one process ever writes it), and `/v1/health` needs
//! no token, so the app finds or starts the daemon first and reads the token after.
//! A daemon the app started is never killed for being slow: on a clean Mac its first
//! Keychain access can wait on a prompt for longer than the start budget.

use almahrya_client::{Client, ClientError};
use almahrya_local::LocalError;
use almahrya_local::paths::{self, Paths};
use almahrya_local::profile::PROFILE_ENV;
use almahrya_local::token_store::{self, ApiToken, TokenStore};
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
    #[error("almahryad is still starting (it may be waiting for a Keychain prompt); see {log}")]
    StillStarting { log: PathBuf },
    #[error(
        "almahryad is running but no API token is stored yet; it creates one on its first start"
    )]
    NoToken,
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

/// [`ensure_daemon`] with explicit limits. Returns within about `start_timeout`, unless
/// reading the token waits on a Keychain prompt.
pub async fn ensure_daemon_with(
    paths: &Paths,
    options: &LinkOptions,
) -> Result<DaemonLink, LinkError> {
    let start = Instant::now();
    let store = token_store::select(paths.profile)?;
    // Health needs no token, so this client only finds the daemon.
    let probe = Client::unix(paths.socket(), String::new());
    let (pid, spawned) = find_or_start(&probe, paths, options, start).await?;
    let token = wait_for_token(store.as_ref(), start + options.start_timeout).await?;
    Ok(DaemonLink {
        client: Client::unix(paths.socket(), token.expose().to_owned()),
        pid,
        spawned,
    })
}

/// Reads the token, trying again until `deadline` while it does not exist yet. Never
/// creates it: that is the daemon's job on its first start.
pub async fn wait_for_token(
    store: &dyn TokenStore,
    deadline: Instant,
) -> Result<ApiToken, LinkError> {
    loop {
        if let Some(token) = store.get()? {
            return Ok(token);
        }
        if Instant::now() >= deadline {
            return Err(LinkError::NoToken);
        }
        tokio::time::sleep(POLL).await;
    }
}

/// The pid of the daemon that answered `/v1/health`, and whether this call started it.
async fn find_or_start(
    probe: &Client,
    paths: &Paths,
    options: &LinkOptions,
    start: Instant,
) -> Result<(u32, bool), LinkError> {
    match bounded(budget(options, start), probe.health()).await {
        Ok(health) => return Ok((health.pid, false)),
        // Something holds the socket but does not answer. Starting another daemon would
        // only fail on the instance lock, so wait for this one instead.
        Err(RequestError::TimedOut(_)) => {
            return wait_for_silent(probe, paths, options, start).await;
        }
        // Nothing listens (no socket, or a stale one): start a daemon.
        Err(RequestError::Client(_)) => {}
    }

    let mut child = spawn(&daemon_binary()?, paths)?;
    let mut child_running = true;
    loop {
        // Until the timeout, any failure means "not listening yet".
        if let Ok(health) = bounded(budget(options, start), probe.health()).await {
            reap(child);
            return Ok((health.pid, true));
        }
        if child_running && let Ok(Some(status)) = child.try_wait() {
            if status.code() != Some(ALREADY_RUNNING) {
                return Err(LinkError::Exited {
                    status: status.to_string(),
                    log: paths::stderr_log(&paths.data_dir),
                });
            }
            // Another start won the race; keep polling the one that holds the lock.
            child_running = false;
        }
        if start.elapsed() >= options.start_timeout {
            // Ours and still running (it may be waiting on a Keychain prompt), or another
            // daemon holds the instance lock (ours exited 3). Either way a daemon is
            // starting: leave it be, and let the caller try again and find it.
            reap(child);
            return Err(LinkError::StillStarting {
                log: paths::stderr_log(&paths.data_dir),
            });
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Keeps asking a daemon that accepts but does not answer, until the start budget ends.
async fn wait_for_silent(
    probe: &Client,
    paths: &Paths,
    options: &LinkOptions,
    start: Instant,
) -> Result<(u32, bool), LinkError> {
    loop {
        if start.elapsed() >= options.start_timeout {
            return Err(LinkError::NotResponding {
                log: paths::stderr_log(&paths.data_dir),
            });
        }
        tokio::time::sleep(POLL).await;
        if let Ok(health) = bounded(budget(options, start), probe.health()).await {
            return Ok((health.pid, false));
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
        // The daemon must use exactly the locations and profile the app connects to.
        .env(paths::DATA_DIR_ENV, &paths.data_dir)
        .env(paths::CONFIG_ENV, &paths.config_file)
        .env(PROFILE_ENV, paths.profile.env_value())
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
