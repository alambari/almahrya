use std::path::PathBuf;

/// Everything that can stop the daemon. Messages never contain the API token.
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("almahryad is already running")]
    AlreadyRunning,
    #[error("{0}\n\nusage: almahryad [install-agent [--plist-dir <dir>]]")]
    Usage(String),
    #[error("invalid config file {path}: {source}")]
    Config {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "socket path is too long ({len} bytes, limit {limit}): {path}; use a shorter ALMAHRYA_DATA_DIR"
    )]
    SocketPathTooLong {
        path: PathBuf,
        len: usize,
        limit: usize,
    },
    #[error(transparent)]
    Local(#[from] almahrya_local::LocalError),
    #[error(transparent)]
    Store(#[from] almahrya_store::StoreError),
    #[error("server task failed: {0}")]
    Task(String),
}

impl DaemonError {
    pub fn io(context: impl Into<String>, source: std::io::Error) -> DaemonError {
        DaemonError::Io {
            context: context.into(),
            source,
        }
    }

    /// Process exit code: 3 for a second instance, 2 for bad arguments, 1 otherwise.
    pub fn exit_code(&self) -> u8 {
        match self {
            DaemonError::AlreadyRunning => 3,
            DaemonError::Usage(_) => 2,
            _ => 1,
        }
    }
}
