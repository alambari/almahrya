//! Where the daemon keeps its files; the desktop app reads the same locations. Environment
//! overrides exist for tests and for running a second copy side by side.

use crate::LocalError;
use crate::profile::Profile;
use std::ffi::OsString;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

pub const CONFIG_ENV: &str = "ALMAHRYA_CONFIG";
pub const DATA_DIR_ENV: &str = "ALMAHRYA_DATA_DIR";

/// Reads one environment variable; tests pass their own lookup instead.
pub type EnvLookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

fn process_env(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

fn env_path(env: EnvLookup<'_>, name: &str) -> Option<PathBuf> {
    env(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn home_in(env: EnvLookup<'_>) -> Result<PathBuf, LocalError> {
    env_path(env, "HOME").ok_or(LocalError::NoHome)
}

fn home() -> Result<PathBuf, LocalError> {
    home_in(&process_env)
}

/// `$ALMAHRYA_CONFIG`, else `~/.config/almahrya/config.toml`. Both profiles read this one
/// file; the development profile's differences sit in its `[dev]` table.
pub fn config_file() -> Result<PathBuf, LocalError> {
    config_file_in(&process_env)
}

fn config_file_in(env: EnvLookup<'_>) -> Result<PathBuf, LocalError> {
    match env_path(env, CONFIG_ENV) {
        Some(p) => Ok(p),
        None => Ok(home_in(env)?.join(".config/almahrya/config.toml")),
    }
}

/// `$ALMAHRYA_DATA_DIR`, else `~/Library/Application Support/Almahrya` (or `Almahrya-Dev`
/// for the development profile). The override wins for either profile.
fn data_dir_in(env: EnvLookup<'_>, profile: Profile) -> Result<PathBuf, LocalError> {
    match env_path(env, DATA_DIR_ENV) {
        Some(p) => Ok(p),
        None => Ok(home_in(env)?
            .join("Library/Application Support")
            .join(profile.data_dir_name())),
    }
}

/// The locations one daemon and its clients agree on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub profile: Profile,
}

impl Paths {
    /// The locations of the profile named by `ALMAHRYA_PROFILE`.
    pub fn from_env() -> Result<Paths, LocalError> {
        Paths::for_profile(Profile::from_env())
    }

    /// The default locations of `profile`, or the `ALMAHRYA_DATA_DIR` and
    /// `ALMAHRYA_CONFIG` overrides.
    pub fn for_profile(profile: Profile) -> Result<Paths, LocalError> {
        Paths::for_profile_in(profile, &process_env)
    }

    /// [`Paths::for_profile`] reading variables through `env` instead of the process.
    pub fn for_profile_in(profile: Profile, env: EnvLookup<'_>) -> Result<Paths, LocalError> {
        Ok(Paths {
            data_dir: data_dir_in(env, profile)?,
            config_file: config_file_in(env)?,
            profile,
        })
    }

    /// The daemon's Unix socket.
    pub fn socket(&self) -> PathBuf {
        socket(&self.data_dir)
    }
}

/// `~/Library/LaunchAgents`.
pub fn launch_agents_dir() -> Result<PathBuf, LocalError> {
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
pub fn ensure_dir(dir: &Path) -> Result<(), LocalError> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| LocalError::io(format!("could not create {}", dir.display()), e))
}

/// Removes a socket (or plain file) left behind by a previous run. Call only while
/// holding the instance lock, so the file cannot belong to a live daemon. `unlink`
/// refuses directories and removes a symlink itself, never its target.
pub fn remove_stale_socket(path: &Path) -> Result<(), LocalError> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(LocalError::io(
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
    fn stale_socket_removal_refuses_directories() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.sock");
        std::fs::write(&file, b"x").unwrap();
        remove_stale_socket(&file).unwrap();
        assert!(!file.exists());
        remove_stale_socket(&file).unwrap();
        assert!(matches!(
            remove_stale_socket(dir.path()),
            Err(LocalError::Io { .. })
        ));
    }
}
