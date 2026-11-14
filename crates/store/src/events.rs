use crate::{Store, StoreError};
use almahrya_api_types::{EventDto, EventEnvelope};
use rusqlite::OptionalExtension;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

fn to_u64(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

impl Store {
    /// Appends `e`, assigning the next `seq`.
    pub fn append_event(&self, e: &EventDto) -> Result<EventEnvelope, StoreError> {
        // Rfc3339 formatting only fails for years outside 0..=9999.
        let at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default();
        let payload = serde_json::to_string(e)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO events (at, payload) VALUES (?1, ?2)",
            rusqlite::params![at, payload],
        )?;
        Ok(EventEnvelope {
            seq: to_u64(conn.last_insert_rowid()),
            at,
            event: e.clone(),
        })
    }

    /// Up to `limit` events with `seq > seq`, oldest first.
    pub fn events_after(&self, seq: u64, limit: usize) -> Result<Vec<EventEnvelope>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT seq, at, payload FROM events WHERE seq > ?1 ORDER BY seq LIMIT ?2")?;
        let rows = stmt.query_map(rusqlite::params![to_i64(seq), to_i64(limit as u64)], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, at, payload) = row?;
            out.push(EventEnvelope {
                seq: to_u64(seq),
                at,
                event: serde_json::from_str(&payload)?,
            });
        }
        Ok(out)
    }

    pub fn oldest_seq(&self) -> Result<Option<u64>, StoreError> {
        let v: Option<Option<i64>> = self
            .conn()?
            .query_row("SELECT MIN(seq) FROM events", [], |r| r.get(0))
            .optional()?;
        Ok(v.flatten().map(to_u64))
    }

    /// Deletes all but the newest `keep_last` events.
    pub fn prune_events(&self, keep_last: u64) -> Result<(), StoreError> {
        self.conn()?.execute(
            "DELETE FROM events WHERE seq <= (SELECT MAX(seq) FROM events) - ?1",
            [to_i64(keep_last)],
        )?;
        Ok(())
    }
}
