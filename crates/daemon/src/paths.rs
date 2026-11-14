//! Where the daemon keeps its files. Environment overrides exist for tests and for
//! running a second copy side by side.

use crate::error::DaemonError;
use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

pub const CONFIG_ENV: &str = "ALMAHRYA_CONFIG";
pub const DATA_DIR_ENV: &str = "ALMAHRYA_DATA_DIR";

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn home() -> Result<PathBuf, DaemonError> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|h: OsString| PathBuf::from(h))
        .ok_or(DaemonError::NoHome)
}

/// `$ALMAHRYA_CONFIG`, else `~/.config/almahrya/config.toml`.
pub fn config_file() -> Result<PathBuf, DaemonError> {
    match env_path(CONFIG_ENV) {
        Some(p) => Ok(p),
        None => Ok(home()?.join(".config/almahrya/config.toml")),
    }
}

/// `$ALMAHRYA_DATA_DIR`, else `~/Library/Application Support/Almahrya`.
pub fn data_dir() -> Result<PathBuf, DaemonError> {
    match env_path(DATA_DIR_ENV) {
        Some(p) => Ok(p),
        None => Ok(home()?.join("Library/Application Support/Almahrya")),
    }
}

/// `~/Library/LaunchAgents`.
pub fn launch_agents_dir() -> Result<PathBuf, DaemonError> {
    Ok(home()?.join("Library/LaunchAgents"))
}

pub fn socket(data_dir: &Path) -> PathBuf {
    data_dir.join("almahryad.sock")
}

pub fn lock(data_dir: &Path) -> PathBuf {
    data_dir.join("almahryad.lock")
}

pub fn database(data_dir: &Path) -> PathBuf {
    data_dir.join("almahrya.db")
}

pub fn stdout_log(data_dir: &Path) -> PathBuf {
    data_dir.join("almahryad.out.log")
}

pub fn stderr_log(data_dir: &Path) -> PathBuf {
    data_dir.join("almahryad.err.log")
}

/// Creates the directory (and parents) owner-only if it does not exist yet.
pub fn ensure_dir(dir: &Path) -> Result<(), DaemonError> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| DaemonError::io(format!("could not create {}", dir.display()), e))
}

/// Fails early, with a readable message, when the socket path exceeds `sun_path`.
pub fn check_socket_path(path: &Path) -> Result<(), DaemonError> {
    let len = path.as_os_str().as_bytes().len();
    let limit = almahrya_api::serve::MAX_SOCKET_PATH_BYTES;
    if len > limit {
        return Err(DaemonError::SocketPathTooLong {
            path: path.to_owned(),
            len,
            limit,
        });
    }
    Ok(())
}

/// Removes a socket (or plain file) left behind by a previous run. Call only while
/// holding the instance lock, so the file cannot belong to a live daemon. `unlink`
/// refuses directories and removes a symlink itself, never its target.
pub fn remove_stale_socket(path: &Path) -> Result<(), DaemonError> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(DaemonError::io(
            format!("could not remove stale socket {}", path.display()),
            e,
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn long_socket_path_is_a_clear_error() {
        let path = PathBuf::from("/tmp").join("x".repeat(110));
        let err = check_socket_path(&path).unwrap_err();
        assert!(err.to_string().contains("too long"), "{err}");
        assert!(check_socket_path(Path::new("/tmp/a.sock")).is_ok());
    }

    #[test]
    fn stale_socket_removal_refuses_directories() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.sock");
        std::fs::write(&file, b"x").unwrap();
        remove_stale_socket(&file).unwrap();
        assert!(!file.exists());
        remove_stale_socket(&file).unwrap();
        assert!(matches!(
            remove_stale_socket(dir.path()),
            Err(DaemonError::Io { .. })
        ));
    }
}
