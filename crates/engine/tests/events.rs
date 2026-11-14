#![allow(clippy::unwrap_used)]

use almahrya_api_types::EventDto;
use almahrya_core::SessionKind;
use almahrya_engine::Engine;
use almahrya_store::Store;
use futures_util::StreamExt;

fn engine_with_retention(retention: u64) -> Engine {
    let dir = tempfile::tempdir().unwrap().keep();
    Engine::new(Store::open(&dir.join("e.db")).unwrap(), retention)
}

fn engine() -> Engine {
    engine_with_retention(10_000)
}

#[tokio::test]
async fn subscriber_gets_replay_then_live() {
    let e = engine();
    e.create_session("one".into(), SessionKind::Chat)
        .await
        .unwrap();
    let mut sub = e.subscribe(0).await.unwrap();
    assert!(matches!(
        sub.next().await.unwrap().event,
        EventDto::SessionCreated { .. }
    ));
    e.create_session("two".into(), SessionKind::Chat)
        .await
        .unwrap();
    assert!(matches!(
        sub.next().await.unwrap().event,
        EventDto::SessionCreated { .. }
    ));
}

#[tokio::test]
async fn subscribe_before_retention_reports_gap() {
    let e = engine_with_retention(2);
    for i in 0..5 {
        e.create_session(format!("s{i}"), SessionKind::Chat)
            .await
            .unwrap();
    }
    let mut sub = e.subscribe(0).await.unwrap();
    assert!(matches!(
        sub.next().await.unwrap().event,
        EventDto::ReplayGap {
            oldest_available: 4
        }
    ));
    assert_eq!(sub.next().await.unwrap().seq, 4);
    assert_eq!(sub.next().await.unwrap().seq, 5);
}

#[tokio::test]
async fn slow_subscriber_loses_nothing_and_sees_no_duplicates() {
    let e = engine();
    let mut sub = e.subscribe(0).await.unwrap();
    // More than the broadcast capacity (1024) before the subscriber reads anything.
    for i in 0..1_500 {
        e.create_session(format!("s{i}"), SessionKind::Chat)
            .await
            .unwrap();
    }
    for expected in 1..=1_500u64 {
        assert_eq!(sub.next().await.unwrap().seq, expected);
    }
}

#[tokio::test]
async fn list_sessions_returns_created_sessions() {
    let e = engine();
    let s = e
        .create_session("one".into(), SessionKind::Chat)
        .await
        .unwrap();
    assert_eq!(e.list_sessions().unwrap(), vec![s]);
}
