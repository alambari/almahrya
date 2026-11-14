//! Rust client for the API (HTTP and WebSocket), used by the desktop app.
//!
//! Two transports share one code path: a Unix socket and plain TCP. Both open a fresh
//! connection per HTTP request; the event stream keeps one WebSocket and reconnects with
//! backoff, resuming after the last delivered `seq`.

mod events;
mod http;

use almahrya_api_types::{ApiError, CreateSessionRequest, EventEnvelope, HealthDto, SessionDto};
use futures_util::Stream;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use url::Url;

/// Everything that can go wrong talking to the daemon.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Connecting, reading or writing failed.
    #[error("transport error: {0}")]
    Transport(String),
    /// The daemon rejected the token (HTTP 401).
    #[error("unauthorized")]
    Unauthorized,
    /// The daemon answered with an error body.
    #[error("api error {status}: {}: {}", .error.code, .error.message)]
    Api { status: u16, error: ApiError },
    /// A response or event could not be decoded.
    #[error("decode error: {0}")]
    Decode(String),
}

#[derive(Clone)]
pub(crate) enum Endpoint {
    Unix(PathBuf),
    Tcp(Url),
}

pub(crate) struct Inner {
    pub(crate) endpoint: Endpoint,
    pub(crate) token: String,
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let endpoint = match &self.endpoint {
            Endpoint::Unix(p) => format!("unix:{}", p.display()),
            Endpoint::Tcp(u) => format!("tcp:{}", u.authority()),
        };
        f.debug_struct("Client")
            .field("endpoint", &endpoint)
            .field("token", &"***")
            .finish()
    }
}

/// Handle to the daemon API. Cheap to clone.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl Client {
    /// Talks to the daemon over its Unix socket.
    pub fn unix(socket: PathBuf, token: String) -> Client {
        Client::new(Endpoint::Unix(socket), token)
    }

    /// Talks to the daemon over plain TCP at `base` (`http://host:port`).
    pub fn tcp(base: Url, token: String) -> Client {
        Client::new(Endpoint::Tcp(base), token)
    }

    fn new(endpoint: Endpoint, token: String) -> Client {
        Client {
            inner: Arc::new(Inner { endpoint, token }),
        }
    }

    /// `/v1/health`: answers without a valid token, so it also tells whether anything
    /// listens at all.
    pub async fn health(&self) -> Result<HealthDto, ClientError> {
        http::get_json(&self.inner, "/v1/health").await
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionDto>, ClientError> {
        http::get_json(&self.inner, "/v1/sessions").await
    }

    pub async fn create_session(
        &self,
        req: CreateSessionRequest,
    ) -> Result<SessionDto, ClientError> {
        http::post_json(&self.inner, "/v1/sessions", &req).await
    }

    /// Events after `after`, reconnecting with backoff and resuming from the last
    /// delivered `seq`. A `seq` is never yielded twice. The stream ends only after an
    /// `Unauthorized` error.
    pub fn events(
        &self,
        after: u64,
    ) -> impl Stream<Item = Result<EventEnvelope, ClientError>> + Send + 'static {
        events::stream(self.inner.clone(), after)
    }
}
