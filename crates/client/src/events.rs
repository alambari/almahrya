//! The resumable event stream.

use crate::http::{connect, host_header, transport};
use crate::{ClientError, Inner};
use almahrya_api_types::EventEnvelope;
use futures_util::{Stream, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{AUTHORIZATION, HOST};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

const BACKOFF_START: Duration = Duration::from_millis(250);
const BACKOFF_MAX: Duration = Duration::from_secs(5);

pub(crate) fn next_backoff(current: Duration) -> Duration {
    (current * 2).min(BACKOFF_MAX)
}

enum Connect {
    Fatal(ClientError),
    Retry,
}

async fn open(
    inner: &Inner,
    after: u64,
) -> Result<tokio_tungstenite::WebSocketStream<Box<dyn crate::http::Io>>, Connect> {
    let stream = connect(inner).await.map_err(|_| Connect::Retry)?;
    let uri = format!("ws://{}/v1/events?after={after}", host_header(inner));
    let mut request = uri
        .into_client_request()
        .map_err(|e| Connect::Fatal(transport(e)))?;
    let headers = request.headers_mut();
    let bearer = format!("Bearer {}", inner.token).parse().map_err(|_| {
        Connect::Fatal(ClientError::Transport("token is not a valid header".into()))
    })?;
    headers.insert(AUTHORIZATION, bearer);
    if let Ok(host) = host_header(inner).parse() {
        headers.insert(HOST, host);
    }
    match client_async(request, stream).await {
        Ok((ws, _)) => Ok(ws),
        Err(WsError::Http(response)) if response.status().as_u16() == 401 => {
            Err(Connect::Fatal(ClientError::Unauthorized))
        }
        Err(_) => Err(Connect::Retry),
    }
}

pub(crate) fn stream(
    inner: Arc<Inner>,
    after: u64,
) -> impl Stream<Item = Result<EventEnvelope, ClientError>> + Send + 'static {
    async_stream::stream! {
        // Highest seq delivered so far; replay-gap envelopes are not contiguous, so the
        // maximum (not a counter) is what a resume must start after.
        let mut last = after;
        let mut backoff = BACKOFF_START;
        loop {
            match open(&inner, last).await {
                Err(Connect::Fatal(e)) => {
                    yield Err(e);
                    return;
                }
                Err(Connect::Retry) => {}
                Ok(mut ws) => {
                    while let Some(frame) = ws.next().await {
                        let text = match frame {
                            Ok(Message::Text(t)) => t,
                            Ok(Message::Close(_)) | Err(_) => break,
                            Ok(_) => continue,
                        };
                        backoff = BACKOFF_START;
                        match serde_json::from_str::<EventEnvelope>(text.as_str()) {
                            Ok(envelope) => {
                                if envelope.seq <= last {
                                    continue;
                                }
                                last = envelope.seq;
                                yield Ok(envelope);
                            }
                            Err(e) => yield Err(ClientError::Decode(e.to_string())),
                        }
                    }
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = next_backoff(backoff);
        }
    }
}
