//! HTTP and WebSocket server: thin, validates, calls the engine and streams events.
//!
//! Request logging is deliberately not enabled: the bearer token travels in the
//! `Authorization` header and, for WebSocket upgrades, in the query string.

mod auth;
mod error;
mod routes;
pub mod serve;
mod ws;

pub use auth::{Token, TokenError};
pub use error::ApiFailure;
pub use routes::router;
