//! Paths, token storage and the local daemon address shared by the daemon and the desktop app.

mod error;
pub mod paths;
pub mod profile;
pub mod token_store;

pub use error::LocalError;
