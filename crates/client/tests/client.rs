#![allow(clippy::unwrap_used)]

use almahrya_api::serve::{serve_tcp, serve_unix};
use almahrya_api::{Token, router};
use almahrya_api_types::{CreateSessionRequest, EventDto, EventEnvelope, SessionKindDto};
use almahrya_client::{Client, ClientError};
use almahrya_core::SessionKind;
use almahrya_engine::{DEFAULT_RETENTION, Engine};
use almahrya_store::Store;
use futures_util::{Stream, StreamExt};
use std::path::Path;
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

const TOKEN: &str = "abc123";

fn engine(dir: &Path, retention: u64) -> Arc<Engine> {
    let store = Store::open(&dir.join("t.db")).unwrap();
    Arc::new(Engine::new(store, retention))
}

fn short_dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("alm")
        .tempdir_in("/private/tmp")
        .unwrap()
}

struct Server {
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl Server {
    fn unix(engine: Arc<Engine>, path: &Path) -> Server {
        let (stop, rx) = oneshot::channel::<()>();
        let app = router(engine, Token::new(TOKEN).unwrap());
        let path = path.to_path_buf();
        let task = tokio::spawn(async move {
            serve_unix(app, &path, async {
                let _ = rx.await;
            })
            .await
            .unwrap();
        });
        Server { stop, task }
    }

    async fn shutdown(self) {
        let _ = self.stop.send(());
        tokio::time::timeout(Duration::from_secs(10), self.task)
            .await
            .unwrap()
            .unwrap();
    }
}

async fn wait_for_socket(path: &Path) {
    for _ in 0..100 {
        if path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("socket never appeared");
}

async fn next<S: Stream<Item = Result<EventEnvelope, ClientError>> + Unpin>(
    s: &mut S,
) -> EventEnvelope {
    tokio::time::timeout(Duration::from_secs(10), s.next())
        .await
        .expect("timed out")
        .expect("stream ended")
        .expect("stream error")
}

fn created_title(e: &EventEnvelope) -> &str {
    match &e.event {
        EventDto::SessionCreated { session } => &session.title,
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn create_and_list_over_unix_socket() {
    let dir = short_dir();
    let sock = dir.path().join("a.sock");
    let server = Server::unix(engine(dir.path(), DEFAULT_RETENTION), &sock);
    wait_for_socket(&sock).await;
    let client = Client::unix(sock.clone(), TOKEN.into());
    let created = client
        .create_session(CreateSessionRequest {
            title: "Hello".into(),
            kind: SessionKindDto::Chat,
        })
        .await
        .unwrap();
    let listed = client.list_sessions().await.unwrap();
    assert_eq!(listed, vec![created]);
    let health = client.health().await.unwrap();
    assert_eq!(health.status, "ok");
    assert_eq!(health.pid, std::process::id());
    server.shutdown().await;
}

/// Serves `engine` on `path` from a private runtime in its own thread, so that
/// shutting the runtime down really drops every open connection (a graceful
/// shutdown leaves upgraded WebSockets attached).
struct IsolatedServer {
    thread: std::thread::JoinHandle<()>,
    stop: std::sync::mpsc::Sender<()>,
}

impl IsolatedServer {
    fn unix(engine: Arc<Engine>, path: &Path) -> IsolatedServer {
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let (ready_tx, ready) = std::sync::mpsc::channel::<()>();
        let path = path.to_path_buf();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            let app = router(engine, Token::new(TOKEN).unwrap());
            let socket = path.clone();
            runtime.spawn(async move {
                let _ = serve_unix(app, &socket, std::future::pending()).await;
            });
            while !path.exists() {
                std::thread::sleep(Duration::from_millis(10));
            }
            ready_tx.send(()).unwrap();
            let _ = stopped.recv();
            runtime.shutdown_timeout(Duration::from_secs(5));
        });
        ready.recv_timeout(Duration::from_secs(10)).unwrap();
        IsolatedServer { thread, stop }
    }

    /// Drops the runtime (and with it the listener and all connections).
    fn kill(self) {
        self.stop.send(()).unwrap();
        self.thread.join().unwrap();
    }
}

#[tokio::test]
async fn reconnect_resumes_without_gap_or_duplicate() {
    let dir = short_dir();
    let sock = dir.path().join("r.sock");
    let engine1 = engine(dir.path(), DEFAULT_RETENTION);
    let server1 = IsolatedServer::unix(engine1.clone(), &sock);
    let client = Client::unix(sock.clone(), TOKEN.into());
    let mut events = pin!(client.events(0));
    engine1
        .create_session("s1".into(), SessionKind::Chat)
        .await
        .unwrap();
    let first = next(&mut events).await;
    assert_eq!(created_title(&first), "s1");

    server1.kill();
    // The runtime died without running the socket cleanup.
    std::fs::remove_file(&sock).unwrap();
    assert!(!sock.exists());

    // A second engine over the same database: only replay from the store can deliver
    // these, a lingering connection to the first engine cannot.
    let engine2 = engine(dir.path(), DEFAULT_RETENTION);
    engine2
        .create_session("s2".into(), SessionKind::Chat)
        .await
        .unwrap();
    engine2
        .create_session("s3".into(), SessionKind::Chat)
        .await
        .unwrap();
    // The old connection is closed and no server listens: nothing can arrive yet.
    assert!(
        tokio::time::timeout(Duration::from_millis(400), events.next())
            .await
            .is_err(),
        "nothing may be delivered while no server is running"
    );
    let server2 = Server::unix(engine2.clone(), &sock);

    let (second, third) = tokio::time::timeout(Duration::from_secs(10), async {
        let second = next(&mut events).await;
        let third = next(&mut events).await;
        (second, third)
    })
    .await
    .expect("stream did not resume after reconnect");
    assert_eq!(created_title(&second), "s2");
    assert_eq!(created_title(&third), "s3");
    assert_eq!(second.seq, first.seq + 1);
    assert_eq!(third.seq, second.seq + 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(600), events.next())
            .await
            .is_err(),
        "no further events expected"
    );
    server2.shutdown().await;
}

#[tokio::test]
async fn tcp_transport_works() {
    let dir = tempfile::tempdir().unwrap();
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = probe.local_addr().unwrap();
    drop(probe);
    let engine = engine(dir.path(), DEFAULT_RETENTION);
    let (stop, rx) = oneshot::channel::<()>();
    let app = router(engine.clone(), Token::new(TOKEN).unwrap());
    let task = tokio::spawn(async move {
        serve_tcp(app, addr, async {
            let _ = rx.await;
        })
        .await
        .unwrap();
    });
    let client = Client::tcp(format!("http://{addr}").parse().unwrap(), TOKEN.into());
    let mut events = pin!(client.events(0));
    // The server may not be listening yet; retry until it is.
    let created = loop {
        match client
            .create_session(CreateSessionRequest {
                title: "T".into(),
                kind: SessionKindDto::Chat,
            })
            .await
        {
            Ok(s) => break s,
            Err(ClientError::Transport(_)) => tokio::time::sleep(Duration::from_millis(50)).await,
            Err(e) => panic!("{e}"),
        }
    };
    assert_eq!(client.list_sessions().await.unwrap(), vec![created]);
    let env = next(&mut events).await;
    assert_eq!(created_title(&env), "T");
    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn unauthorized_maps_to_client_error() {
    let dir = short_dir();
    let sock = dir.path().join("u.sock");
    let server = Server::unix(engine(dir.path(), DEFAULT_RETENTION), &sock);
    wait_for_socket(&sock).await;
    let client = Client::unix(sock.clone(), "wrong".into());
    assert!(matches!(
        client.list_sessions().await,
        Err(ClientError::Unauthorized)
    ));
    let mut events = pin!(client.events(0));
    assert!(matches!(
        events.next().await,
        Some(Err(ClientError::Unauthorized))
    ));
    assert!(events.next().await.is_none());
    assert!(!format!("{client:?}").contains("wrong"));
    server.shutdown().await;
}

#[tokio::test]
async fn replay_gap_is_passed_through_to_the_consumer() {
    let dir = short_dir();
    let sock = dir.path().join("g.sock");
    let engine = engine(dir.path(), 2);
    for i in 0..5 {
        engine
            .create_session(format!("s{i}"), SessionKind::Chat)
            .await
            .unwrap();
    }
    let server = Server::unix(engine.clone(), &sock);
    wait_for_socket(&sock).await;
    let client = Client::unix(sock.clone(), TOKEN.into());
    let mut events = pin!(client.events(0));
    let gap = next(&mut events).await;
    assert!(matches!(gap.event, EventDto::ReplayGap { .. }));
    let a = next(&mut events).await;
    let b = next(&mut events).await;
    assert_eq!(created_title(&a), "s3");
    assert_eq!(created_title(&b), "s4");
    server.shutdown().await;
}
