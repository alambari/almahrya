#![allow(clippy::unwrap_used)]

use almahrya_api_types::{EventDto, SessionDto, SessionKindDto};
use almahrya_core::{Session, SessionId, SessionKind};
use almahrya_store::Store;
use tempfile::TempDir;
use time::OffsetDateTime;

/// Removed when the test ends, so no directory is left in `$TMPDIR`.
fn tmp() -> TempDir {
    tempfile::tempdir().unwrap()
}

fn created(title: &str) -> EventDto {
    EventDto::SessionCreated {
        session: SessionDto {
            id: "id".into(),
            title: title.into(),
            kind: SessionKindDto::Chat,
            created_at: "2026-01-01T00:00:00Z".into(),
        },
    }
}

#[test]
fn events_get_increasing_seq_and_replay_after() {
    let dir = tmp();
    let s = Store::open(&dir.path().join("a.db")).unwrap();
    let a = s.append_event(&created("one")).unwrap();
    let b = s.append_event(&created("two")).unwrap();
    assert_eq!(b.seq, a.seq + 1);
    assert_eq!(s.events_after(a.seq, 10).unwrap().len(), 1);
}

#[test]
fn prune_keeps_last_and_reports_oldest() {
    let dir = tmp();
    let s = Store::open(&dir.path().join("a.db")).unwrap();
    assert_eq!(s.oldest_seq().unwrap(), None);
    for i in 0..5 {
        s.append_event(&created(&format!("s{i}"))).unwrap();
    }
    s.prune_events(2).unwrap();
    assert_eq!(s.oldest_seq().unwrap(), Some(4));
    assert_eq!(s.events_after(0, 10).unwrap().len(), 2);
}

#[test]
fn sessions_round_trip_newest_first_and_survive_reopen() {
    let dir = tmp();
    let path = dir.path().join("a.db");
    let mk = |title: &str, kind: SessionKind, secs: i64| Session {
        id: SessionId::new(),
        title: title.into(),
        kind,
        created_at: OffsetDateTime::from_unix_timestamp(secs).unwrap(),
    };
    let old = mk("old", SessionKind::Chat, 1_000);
    let new = mk(
        "new",
        SessionKind::Ticket {
            key: "PROJ-123".into(),
        },
        2_000,
    );
    {
        let s = Store::open(&path).unwrap();
        s.insert_session(&old).unwrap();
        s.insert_session(&new).unwrap();
    }
    let s = Store::open(&path).unwrap();
    assert_eq!(s.list_sessions().unwrap(), vec![new, old]);
}
