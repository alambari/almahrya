//! Runs both listeners until SIGTERM or SIGINT, then shuts them down gracefully.

use crate::error::DaemonError;
use almahrya_api::serve::{serve_tcp, serve_unix};
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// How long in-flight requests get to finish after a stop signal.
const GRACE: Duration = Duration::from_millis(1500);

type Server = JoinHandle<std::io::Result<()>>;

fn joined(
    what: &str,
    result: Result<std::io::Result<()>, tokio::task::JoinError>,
) -> Result<(), DaemonError> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(DaemonError::io(format!("{what} listener failed"), e)),
        Err(e) => Err(DaemonError::Task(e.to_string())),
    }
}

/// Serves `router` on `socket` (must not exist) and `addr` until a stop signal.
pub async fn run(router: Router, socket: PathBuf, addr: SocketAddr) -> Result<(), DaemonError> {
    let mut term = signal(SignalKind::terminate())
        .map_err(|e| DaemonError::io("could not install the SIGTERM handler", e))?;
    let mut int = signal(SignalKind::interrupt())
        .map_err(|e| DaemonError::io("could not install the SIGINT handler", e))?;

    let (stop, stopped) = watch::channel(false);
    let on_stop = |mut rx: watch::Receiver<bool>| async move {
        let _ = rx.wait_for(|stop| *stop).await;
    };

    let mut unix: Server = tokio::spawn({
        let router = router.clone();
        let socket = socket.clone();
        let shutdown = on_stop(stopped.clone());
        async move { serve_unix(router, &socket, shutdown).await }
    });
    let mut tcp: Server = tokio::spawn(serve_tcp(router, addr, on_stop(stopped)));
    eprintln!(
        "almahryad serving on {} and http://{addr}",
        socket.display()
    );

    // Either a signal, or a listener ending early (e.g. the port is taken).
    let mut first = Ok(());
    let (mut unix_done, mut tcp_done) = (false, false);
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
        r = &mut unix => { unix_done = true; first = joined("unix socket", r); }
        r = &mut tcp => { tcp_done = true; first = joined("tcp", r); }
    }
    let _ = stop.send(true);

    let rest = async {
        let mut result = Ok(());
        if !unix_done {
            result = result.and(joined("unix socket", (&mut unix).await));
        }
        if !tcp_done {
            result = result.and(joined("tcp", (&mut tcp).await));
        }
        result
    };
    let rest = match tokio::time::timeout(GRACE, rest).await {
        Ok(result) => result,
        Err(_) => {
            // Stragglers are dropped with the runtime; make sure the socket goes too.
            unix.abort();
            tcp.abort();
            Ok(())
        }
    };
    crate::paths::remove_stale_socket(&socket)?;
    first.and(rest)
}
