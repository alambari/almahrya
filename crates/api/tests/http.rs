#![allow(clippy::unwrap_used)]

use almahrya_api::{Token, router};
use almahrya_api_types::{HealthDto, SessionDto, SessionKindDto};
use almahrya_engine::{DEFAULT_RETENTION, Engine};
use almahrya_store::Store;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, header};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

const TOKEN: &str = "example-token";

fn app() -> (Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("a.db")).unwrap();
    let engine = Arc::new(Engine::new(store, DEFAULT_RETENTION));
    (router(engine, Token::new(TOKEN)), dir)
}

async fn get(app: &Router, uri: &str, token: Option<&str>) -> Response<Body> {
    let mut req = Request::get(uri);
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    app.clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn post(app: &Router, uri: &str, token: &str, body: &str) -> Response<Body> {
    let req = Request::post(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap();
    app.clone().oneshot(req).await.unwrap()
}

async fn body_string(res: Response<Body>) -> String {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn health_needs_no_token() {
    let (app, _dir) = app();
    let res = get(&app, "/v1/health", None).await;
    assert_eq!(res.status(), 200);
    let dto: HealthDto = serde_json::from_str(&body_string(res).await).unwrap();
    assert_eq!(dto.status, "ok");
    assert_eq!(dto.pid, std::process::id());
}

#[tokio::test]
async fn wrong_token_is_rejected_without_echo() {
    let (app, _dir) = app();
    let res = get(&app, "/v1/sessions", Some("nope-secret")).await;
    assert_eq!(res.status(), 401);
    let body = body_string(res).await;
    assert!(body.contains(r#""code":"unauthorized""#));
    assert!(!body.contains("nope-secret"));
}

#[tokio::test]
async fn missing_token_is_rejected() {
    let (app, _dir) = app();
    assert_eq!(get(&app, "/v1/sessions", None).await.status(), 401);
}

#[tokio::test]
async fn token_debug_is_masked() {
    assert_eq!(format!("{:?}", Token::new(TOKEN)), "Token(***)");
}

#[tokio::test]
async fn create_then_list_session() {
    let (app, _dir) = app();
    let res = post(
        &app,
        "/v1/sessions",
        TOKEN,
        r#"{"title":"First","kind":{"type":"chat"}}"#,
    )
    .await;
    assert_eq!(res.status(), 201);
    let created: SessionDto = serde_json::from_str(&body_string(res).await).unwrap();
    assert_eq!(created.title, "First");

    let res = get(&app, "/v1/sessions", Some(TOKEN)).await;
    assert_eq!(res.status(), 200);
    let list: Vec<SessionDto> = serde_json::from_str(&body_string(res).await).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].title, "First");
    assert_eq!(list[0].kind, SessionKindDto::Chat);
}

#[tokio::test]
async fn malformed_body_is_422_invalid_request() {
    let (app, _dir) = app();
    let res = post(&app, "/v1/sessions", TOKEN, r#"{"title":1}"#).await;
    assert_eq!(res.status(), 422);
    assert!(
        body_string(res)
            .await
            .contains(r#""code":"invalid_request""#)
    );
}
