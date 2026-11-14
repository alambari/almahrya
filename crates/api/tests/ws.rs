#![allow(clippy::unwrap_used)]

use almahrya_api::{Token, router};
use almahrya_api_types::{EventDto, EventEnvelope};
use almahrya_core::SessionKind;
use almahrya_engine::{DEFAULT_RETENTION, Engine};
use almahrya_store::Store;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;

const TOKEN: &str = "example-token";

async fn serve() -> (std::net::SocketAddr, Arc<Engine>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("w.db")).unwrap();
    let engine = Arc::new(Engine::new(store, DEFAULT_RETENTION));
    let app = router(engine.clone(), Token::new(TOKEN).unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (addr, engine, dir)
}

async fn next_event<S>(ws: &mut S) -> EventEnvelope
where
    S: futures_util::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + Unpin,
{
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next())
        .await
        .expect("timed out")
        .expect("stream ended")
        .expect("ws error");
    serde_json::from_str(msg.to_text().unwrap()).unwrap()
}

#[tokio::test]
async fn websocket_streams_replay_then_live() {
    let (addr, engine, _dir) = serve().await;
    engine
        .create_session("Old".into(), SessionKind::Chat)
        .await
        .unwrap();

    let url = format!("ws://{addr}/v1/events?after=0&token={TOKEN}");
    let (mut ws, _) = connect_async(url).await.unwrap();

    let first = next_event(&mut ws).await;
    match first.event {
        EventDto::SessionCreated { session } => assert_eq!(session.title, "Old"),
        other => panic!("unexpected {other:?}"),
    }

    engine
        .create_session("Live".into(), SessionKind::Chat)
        .await
        .unwrap();
    let second = next_event(&mut ws).await;
    assert!(second.seq > first.seq);
    match second.event {
        EventDto::SessionCreated { session } => assert_eq!(session.title, "Live"),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn websocket_without_token_is_refused_before_upgrade() {
    let (addr, _engine, _dir) = serve().await;
    let err = connect_async(format!("ws://{addr}/v1/events"))
        .await
        .unwrap_err();
    match err {
        WsError::Http(res) => assert_eq!(res.status(), 401),
        other => panic!("expected an HTTP refusal, got {other:?}"),
    }
}
