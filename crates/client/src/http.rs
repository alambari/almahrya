//! Connection setup and plain HTTP requests over either transport.

use crate::{ClientError, Endpoint, Inner};
use almahrya_api_types::ApiError;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{AUTHORIZATION, CONTENT_TYPE, HOST};
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpStream, UnixStream};

pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

pub(crate) fn transport(e: impl std::fmt::Display) -> ClientError {
    ClientError::Transport(e.to_string())
}

pub(crate) async fn connect(inner: &Inner) -> Result<Box<dyn Io>, ClientError> {
    match &inner.endpoint {
        Endpoint::Unix(path) => Ok(Box::new(
            UnixStream::connect(path).await.map_err(transport)?,
        )),
        Endpoint::Tcp(base) => {
            let host = base
                .host_str()
                .ok_or_else(|| ClientError::Transport("base url has no host".into()))?;
            let port = base
                .port_or_known_default()
                .ok_or_else(|| ClientError::Transport("base url has no port".into()))?;
            Ok(Box::new(
                TcpStream::connect((host, port)).await.map_err(transport)?,
            ))
        }
    }
}

/// Value of the `Host` header for this endpoint.
pub(crate) fn host_header(inner: &Inner) -> String {
    match &inner.endpoint {
        Endpoint::Unix(_) => "localhost".to_owned(),
        Endpoint::Tcp(base) => match (base.host_str(), base.port()) {
            (Some(h), Some(p)) => format!("{h}:{p}"),
            (Some(h), None) => h.to_owned(),
            _ => "localhost".to_owned(),
        },
    }
}

async fn send(
    inner: &Inner,
    method: Method,
    path: &str,
    body: Option<Vec<u8>>,
) -> Result<(StatusCode, Bytes), ClientError> {
    let stream = connect(inner).await?;
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(transport)?;
    let driver = tokio::spawn(async move {
        let _ = conn.await;
    });
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(HOST, host_header(inner))
        .header(AUTHORIZATION, format!("Bearer {}", inner.token));
    let payload = match body {
        Some(b) => {
            builder = builder.header(CONTENT_TYPE, "application/json");
            Bytes::from(b)
        }
        None => Bytes::new(),
    };
    let request = builder.body(Full::new(payload)).map_err(transport)?;
    let result = async {
        let response = sender.send_request(request).await.map_err(transport)?;
        let status = response.status();
        let bytes = response.into_body().collect().await.map_err(transport)?;
        Ok((status, bytes.to_bytes()))
    }
    .await;
    driver.abort();
    result
}

fn decode<T: DeserializeOwned>(status: StatusCode, body: &Bytes) -> Result<T, ClientError> {
    if status == StatusCode::UNAUTHORIZED {
        return Err(ClientError::Unauthorized);
    }
    if !status.is_success() {
        return Err(match serde_json::from_slice::<ApiError>(body) {
            Ok(error) => ClientError::Api {
                status: status.as_u16(),
                error,
            },
            Err(_) => ClientError::Decode(format!("unexpected status {status} without api error")),
        });
    }
    serde_json::from_slice(body).map_err(|e| ClientError::Decode(e.to_string()))
}

pub(crate) async fn get_json<T: DeserializeOwned>(
    inner: &Inner,
    path: &str,
) -> Result<T, ClientError> {
    let (status, body) = send(inner, Method::GET, path, None).await?;
    decode(status, &body)
}

pub(crate) async fn post_json<T: DeserializeOwned, B: Serialize>(
    inner: &Inner,
    path: &str,
    body: &B,
) -> Result<T, ClientError> {
    let payload = serde_json::to_vec(body).map_err(|e| ClientError::Decode(e.to_string()))?;
    let (status, body) = send(inner, Method::POST, path, Some(payload)).await?;
    decode(status, &body)
}
