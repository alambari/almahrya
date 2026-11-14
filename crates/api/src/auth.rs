use crate::error::ApiFailure;
use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq;

/// The shared secret clients present. Never printed.
#[derive(Clone)]
pub struct Token(String);

impl Token {
    /// Rejects an empty or whitespace-only value; the stored value is trimmed.
    pub fn new(value: impl AsRef<str>) -> Result<Self, TokenError> {
        let trimmed = value.as_ref().trim();
        if trimmed.is_empty() {
            return Err(TokenError::Empty);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Constant-time comparison against a candidate.
    pub fn matches(&self, candidate: &str) -> bool {
        if candidate.is_empty() {
            return false;
        }
        self.0.as_bytes().ct_eq(candidate.as_bytes()).into()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("token must not be empty")]
    Empty,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(***)")
    }
}

fn bearer(req: &Request) -> Option<&str> {
    req.headers()
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Browsers cannot set headers on a WebSocket upgrade, so that route also accepts `?token=`.
fn query_token(req: &Request) -> Option<&str> {
    if req.uri().path() != "/v1/events" {
        return None;
    }
    req.uri()
        .query()?
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
}

pub(crate) async fn require_token(
    State(token): State<Token>,
    req: Request,
    next: Next,
) -> Result<Response, ApiFailure> {
    let supplied = bearer(&req).or_else(|| query_token(&req));
    match supplied {
        Some(candidate) if token.matches(candidate) => Ok(next.run(req).await),
        _ => Err(ApiFailure::Unauthorized),
    }
}
