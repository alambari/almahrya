use crate::SessionDto;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct EventEnvelope {
    #[ts(type = "number")]
    pub seq: u64,
    /// RFC 3339 timestamp.
    pub at: String,
    pub event: EventDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum EventDto {
    SessionCreated {
        session: SessionDto,
    },
    ReplayGap {
        #[ts(type = "number")]
        oldest_available: u64,
    },
}
