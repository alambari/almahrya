use crate::error::ApiFailure;
use almahrya_api_types::EventEnvelope;
use almahrya_engine::Engine;
use axum::extract::rejection::QueryRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Deserialize)]
pub(crate) struct EventsQuery {
    #[serde(default)]
    after: u64,
    // Consumed by the auth layer; accepted here so the query still parses.
    #[serde(default, rename = "token")]
    _token: Option<String>,
}

pub(crate) async fn events(
    State(engine): State<Arc<Engine>>,
    query: Result<Query<EventsQuery>, QueryRejection>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ApiFailure> {
    let Query(query) = query.map_err(|e| ApiFailure::InvalidRequest(e.body_text()))?;
    let stream = engine.subscribe(query.after).await?;
    Ok(upgrade.on_upgrade(move |socket| pump(socket, stream)))
}

type EventStream = Pin<Box<dyn Stream<Item = EventEnvelope> + Send>>;

async fn pump(mut socket: WebSocket, mut stream: EventStream) {
    loop {
        tokio::select! {
            next = stream.next() => {
                let Some(envelope) = next else { break };
                let Ok(text) = serde_json::to_string(&envelope) else { break };
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    None | Some(Err(_) | Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}
