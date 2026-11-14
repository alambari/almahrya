//! The running system: drives the sources, applies the rules, starts agent runs and
//! the Lark bot, and persists state.
//!
//! It runs on its own async runtime, separate from the GPUI executor, and talks to
//! the UI only through channels.

mod events;

pub use events::{DEFAULT_RETENTION, Engine, collect_replay};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Store(#[from] almahrya_store::StoreError),
    #[error("engine lock poisoned")]
    Poisoned,
}
