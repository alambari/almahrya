use crate::EngineError;
use almahrya_api_types::{EventDto, EventEnvelope, SessionDto};
use almahrya_core::{Session, SessionId, SessionKind};
use almahrya_store::{Store, StoreError};
use futures_core::Stream;
use std::{pin::Pin, sync::Arc};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::{Mutex, broadcast};

/// Default number of events kept for replay.
pub const DEFAULT_RETENTION: u64 = 10_000;
const BROADCAST_CAPACITY: usize = 1024;
const REPLAY_BATCH: usize = 500;

pub struct Engine {
    store: Arc<Store>,
    retention: u64,
    tx: broadcast::Sender<EventEnvelope>,
    /// Serializes append + broadcast so live order always matches `seq` order.
    append_lock: Mutex<()>,
}

impl Engine {
    pub fn new(store: Store, retention: u64) -> Engine {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Engine {
            store: Arc::new(store),
            retention,
            tx,
            append_lock: Mutex::new(()),
        }
    }

    pub async fn create_session(
        &self,
        title: String,
        kind: SessionKind,
    ) -> Result<Session, EngineError> {
        let session = Session {
            id: SessionId::new(),
            title,
            kind,
            created_at: OffsetDateTime::now_utc(),
        };
        self.store.insert_session(&session)?;
        self.publish(&EventDto::SessionCreated {
            session: SessionDto::from(&session),
        })
        .await?;
        Ok(session)
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>, EngineError> {
        Ok(self.store.list_sessions()?)
    }

    async fn publish(&self, event: &EventDto) -> Result<(), EngineError> {
        let _guard = self.append_lock.lock().await;
        let envelope = self.store.append_event(event)?;
        self.store.prune_events(self.retention)?;
        // No receivers is fine: the event is stored and replayable.
        let _ = self.tx.send(envelope);
        Ok(())
    }

    /// Replays stored events after `after`, then follows live events.
    ///
    /// If `after` is older than the retained history, the first item is a
    /// `ReplayGap` carrying the oldest available `seq`.
    pub async fn subscribe(
        &self,
        after: u64,
    ) -> Result<Pin<Box<dyn Stream<Item = EventEnvelope> + Send>>, EngineError> {
        // Subscribe first so nothing appended during the replay read is missed.
        let mut rx = self.tx.subscribe();
        let store = Arc::clone(&self.store);
        let initial = catch_up(&store, after)?;
        Ok(Box::pin(async_stream::stream! {
            let mut cursor = after;
            for env in initial {
                cursor = cursor.max(env.seq);
                yield env;
            }
            loop {
                match rx.recv().await {
                    Ok(env) => {
                        if env.seq > cursor {
                            cursor = env.seq;
                            yield env;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Re-read what was missed from the store.
                        let Ok(missed) = catch_up(&store, cursor) else { break };
                        for env in missed {
                            cursor = cursor.max(env.seq);
                            yield env;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }))
    }
}

/// Stored events after `after`, preceded by a `ReplayGap` when history was pruned past it.
fn catch_up(store: &Store, after: u64) -> Result<Vec<EventEnvelope>, StoreError> {
    let mut out = Vec::new();
    if let Some(oldest) = store.oldest_seq()?
        && after.saturating_add(1) < oldest
    {
        out.push(EventEnvelope {
            seq: oldest - 1,
            // Rfc3339 formatting only fails for years outside 0..=9999.
            at: OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .unwrap_or_default(),
            event: EventDto::ReplayGap {
                oldest_available: oldest,
            },
        });
    }
    let mut cursor = out.last().map_or(after, |g| g.seq);
    loop {
        let batch = store.events_after(cursor, REPLAY_BATCH)?;
        let Some(last) = batch.last() else { break };
        cursor = last.seq;
        out.extend(batch);
    }
    Ok(out)
}
