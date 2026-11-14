#![allow(clippy::unwrap_used)]

use almahrya_api_types::EventDto;
use almahrya_core::SessionKind;
use almahrya_engine::Engine;
use almahrya_store::Store;
use futures_util::StreamExt;

/// An engine whose database directory is removed when the test ends.
struct TestEngine {
    engine: Engine,
    _dir: tempfile::TempDir,
}

impl std::ops::Deref for TestEngine {
    type Target = Engine;

    fn deref(&self) -> &Engine {
        &self.engine
    }
}

fn engine_with_retention(retention: u64) -> TestEngine {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::new(Store::open(&dir.path().join("e.db")).unwrap(), retention);
    TestEngine { engine, _dir: dir }
}

fn engine() -> TestEngine {
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

#[test]
fn pruning_between_oldest_check_and_batch_read_is_signalled() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("e.db")).unwrap();
    for i in 0..5 {
        store
            .append_event(&EventDto::ReplayGap {
                oldest_available: i,
            })
            .unwrap();
    }
    // The caller saw oldest = 1 (so `after = 0` needs no up-front gap) ...
    let oldest = store.oldest_seq().unwrap();
    assert_eq!(oldest, Some(1));
    // ... then a concurrent publish pruned events 1..=3 before the batch read.
    store.prune_events(2).unwrap();
    let out = almahrya_engine::collect_replay(0, oldest, |c| store.events_after(c, 500)).unwrap();
    assert!(matches!(
        out[0].event,
        EventDto::ReplayGap {
            oldest_available: 4
        }
    ));
    let seqs: Vec<u64> = out.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![3, 4, 5]);
}

#[test]
fn pruning_between_batches_is_signalled() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("e.db")).unwrap();
    for i in 0..6 {
        store
            .append_event(&EventDto::ReplayGap {
                oldest_available: i,
            })
            .unwrap();
    }
    let mut reads = 0;
    let out = almahrya_engine::collect_replay(0, Some(1), |c| {
        reads += 1;
        let batch = store.events_after(c, 2)?;
        if reads == 1 {
            // Pruning lands after the first batch was read.
            store.prune_events(1)?;
        }
        Ok(batch)
    })
    .unwrap();
    let kinds: Vec<(u64, bool)> = out
        .iter()
        .map(|e| {
            (
                e.seq,
                matches!(
                    e.event,
                    EventDto::ReplayGap {
                        oldest_available: 6
                    }
                ),
            )
        })
        .collect();
    assert_eq!(kinds, vec![(1, false), (2, false), (5, true), (6, false)]);
}
