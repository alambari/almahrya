//! Single instance: an exclusive `flock` on `<data_dir>/almahryad.lock`, held until
//! the process exits (the kernel releases it when the file is closed).

use crate::error::DaemonError;
use std::fs::File;
use std::path::Path;

pub struct InstanceLock {
    _lock: fd_lock::RwLock<File>,
}

impl InstanceLock {
    /// Takes the lock without waiting; `AlreadyRunning` if another process holds it.
    pub fn acquire(path: &Path) -> Result<InstanceLock, DaemonError> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(|e| DaemonError::io(format!("could not open {}", path.display()), e))?;
        let mut lock = fd_lock::RwLock::new(file);
        match lock.try_write() {
            // The guard would unlock on drop; forgetting it keeps the flock for as
            // long as the file stays open, which is the lifetime of `InstanceLock`.
            Ok(guard) => std::mem::forget(guard),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(DaemonError::AlreadyRunning);
            }
            Err(e) => {
                return Err(DaemonError::io(
                    format!("could not lock {}", path.display()),
                    e,
                ));
            }
        }
        Ok(InstanceLock { _lock: lock })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn second_lock_is_refused_until_the_first_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.lock");
        let first = InstanceLock::acquire(&path).unwrap();
        assert!(matches!(
            InstanceLock::acquire(&path),
            Err(DaemonError::AlreadyRunning)
        ));
        drop(first);
        InstanceLock::acquire(&path).unwrap();
    }
}
