use crate::{Store, StoreError};
use almahrya_core::{Session, SessionId, SessionKind};
use time::OffsetDateTime;

impl Store {
    pub fn insert_session(&self, s: &Session) -> Result<(), StoreError> {
        let (kind, key) = match &s.kind {
            SessionKind::Chat => ("chat", None),
            SessionKind::Ticket { key } => ("ticket", Some(key.as_str())),
        };
        self.conn()?.execute(
            "INSERT INTO sessions (id, title, kind, ticket_key, created_at_ns) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                s.id.to_string(),
                s.title,
                kind,
                key,
                s.created_at.unix_timestamp_nanos().to_string()
            ],
        )?;
        Ok(())
    }

    /// Newest first.
    pub fn list_sessions(&self) -> Result<Vec<Session>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, kind, ticket_key, created_at_ns FROM sessions
             ORDER BY CAST(created_at_ns AS INTEGER) DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, title, kind, key, ns) = row?;
            let kind = match (kind.as_str(), key) {
                ("chat", _) => SessionKind::Chat,
                ("ticket", Some(key)) => SessionKind::Ticket { key },
                (other, _) => return Err(StoreError::Corrupt(format!("session kind {other}"))),
            };
            let id = id
                .parse::<SessionId>()
                .map_err(|e| StoreError::Corrupt(format!("session id: {e}")))?;
            let created_at = ns
                .parse::<i128>()
                .ok()
                .and_then(|n| OffsetDateTime::from_unix_timestamp_nanos(n).ok())
                .ok_or_else(|| StoreError::Corrupt(format!("session timestamp {ns}")))?;
            out.push(Session {
                id,
                title,
                kind,
                created_at,
            });
        }
        Ok(out)
    }
}
