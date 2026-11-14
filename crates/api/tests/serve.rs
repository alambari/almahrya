#![allow(clippy::unwrap_used)]

use almahrya_api::serve::{serve_tcp, serve_unix};
use almahrya_api::{Token, router};
use almahrya_engine::{DEFAULT_RETENTION, Engine};
use almahrya_store::Store;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;

fn app(dir: &Path) -> axum::Router {
    let store = Store::open(&dir.join("a.db")).unwrap();
    let engine = Arc::new(Engine::new(store, DEFAULT_RETENTION));
    router(engine, Token::new("example-token").unwrap())
}

/// Permission bits as `stat` prints them, e.g. "600".
fn mode_of(path: &Path) -> String {
    let out = std::process::Command::new("stat")
        .args(["-f", "%Lp"])
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

async fn health<S: AsyncRead + AsyncWrite + Unpin>(mut stream: S) -> String {
    stream
        .write_all(b"GET /v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut out = String::new();
    stream.read_to_string(&mut out).await.unwrap();
    out
}

async fn connect_unix(path: &Path) -> tokio::net::UnixStream {
    for _ in 0..100 {
        if let Ok(s) = tokio::net::UnixStream::connect(path).await {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("socket never became ready");
}

async fn connect_tcp(addr: SocketAddr) -> tokio::net::TcpStream {
    for _ in 0..100 {
        if let Ok(s) = tokio::net::TcpStream::connect(addr).await {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("port never became ready");
}

#[tokio::test]
async fn unix_socket_serves_with_mode_0600_and_is_removed_on_shutdown() {
    let dir = tempfile::Builder::new().prefix("as").tempdir().unwrap();
    let path = dir.path().join("t.sock");
    let (tx, rx) = oneshot::channel::<()>();
    let server = tokio::spawn({
        let path = path.clone();
        let app = app(dir.path());
        async move {
            serve_unix(app, &path, async {
                let _ = rx.await;
            })
            .await
        }
    });

    let response = health(connect_unix(&path).await).await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert_eq!(mode_of(&path), "600");

    tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn unix_socket_path_over_the_limit_is_a_clear_error() {
    let long = Path::new("/tmp").join("x".repeat(120));
    let err = serve_unix(app(tempfile::tempdir().unwrap().path()), &long, async {})
        .await
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("too long"), "{err}");
}

#[tokio::test]
async fn tcp_serves_until_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let addr = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    let server = tokio::spawn(serve_tcp(app(dir.path()), addr, async {
        let _ = rx.await;
    }));

    let response = health(connect_tcp(addr).await).await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");

    tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
