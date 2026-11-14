use almahrya_core::{Session, SessionKind};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum SessionKindDto {
    Chat,
    Ticket { key: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SessionDto {
    pub id: String,
    pub title: String,
    pub kind: SessionKindDto,
    /// RFC 3339 timestamp.
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CreateSessionRequest {
    pub title: String,
    pub kind: SessionKindDto,
}

impl From<&SessionKind> for SessionKindDto {
    fn from(kind: &SessionKind) -> Self {
        match kind {
            SessionKind::Chat => Self::Chat,
            SessionKind::Ticket { key } => Self::Ticket { key: key.clone() },
        }
    }
}

impl From<&Session> for SessionDto {
    fn from(s: &Session) -> Self {
        Self {
            id: s.id.to_string(),
            title: s.title.clone(),
            kind: (&s.kind).into(),
            // Rfc3339 formatting only fails for years outside 0..=9999.
            created_at: s.created_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}
