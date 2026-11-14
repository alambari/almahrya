//! Thin transport helpers: serve a router on a Unix socket or a TCP address until a
//! shutdown future resolves, then drain in-flight requests (axum graceful shutdown).

use axum::Router;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tokio::net::{TcpListener, UnixListener};

/// macOS `sun_path` holds 104 bytes including the trailing NUL.
pub const MAX_SOCKET_PATH_BYTES: usize = 103;

/// Rejects a socket path the kernel would truncate, with a readable message.
pub fn check_socket_path(path: &Path) -> io::Result<()> {
    let len = path.as_os_str().as_bytes().len();
    if len > MAX_SOCKET_PATH_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "socket path is too long ({len} bytes, limit {MAX_SOCKET_PATH_BYTES}): {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

/// Binds `path` (which must not exist), restricts it to the owner (0600), serves until
/// `shutdown` resolves and removes the socket file afterwards.
pub async fn serve_unix(
    router: Router,
    path: &Path,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    check_socket_path(path)?;
    let listener = UnixListener::bind(path)?;
    let result = async {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await
    }
    .await;
    let removed = match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    };
    result.and(removed)
}

/// Binds `addr`, serves until `shutdown` resolves, then drains.
pub async fn serve_tcp(
    router: Router,
    addr: SocketAddr,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
}
