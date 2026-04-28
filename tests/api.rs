use std::time::Duration;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use musicindex_live_relay::{
    AppConfig, CreateEventResponse, LatestMetadataResponse, PublishMetadataResponse, RelayState,
    app,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::time::timeout;
use tower::ServiceExt;

fn test_app() -> axum::Router {
    app(RelayState::new(AppConfig::for_tests()))
}

async fn read_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("response body is json")
}

async fn create_event(router: axum::Router) -> CreateEventResponse {
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/liveitems")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("create response");

    assert_eq!(response.status(), StatusCode::OK);
    read_json(response).await
}

async fn create_event_at(router: axum::Router, uri: &str) -> CreateEventResponse {
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("create response");

    assert_eq!(response.status(), StatusCode::OK);
    read_json(response).await
}

async fn publish(
    router: axum::Router,
    event_id: &str,
    token: Option<&str>,
    body: Value,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!("/v1/liveitems/{event_id}/metadata"))
        .header(header::CONTENT_TYPE, "application/json");

    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }

    router
        .oneshot(
            builder
                .body(Body::from(body.to_string()))
                .expect("publish request"),
        )
        .await
        .expect("publish response")
}

async fn next_sse_chunk(body: &mut Body) -> String {
    let frame = timeout(Duration::from_secs(2), body.frame())
        .await
        .expect("timed out waiting for sse frame")
        .expect("sse body frame")
        .expect("sse frame result");
    let bytes = frame.into_data().expect("sse data frame");
    String::from_utf8(bytes.to_vec()).expect("sse frame is utf-8")
}

#[tokio::test]
async fn create_publish_and_fetch_latest_snapshot() {
    let router = test_app();
    let created = create_event(router.clone()).await;

    let response = publish(
        router.clone(),
        &created.event_id,
        Some(&created.broadcaster_token),
        json!({
            "event_id": created.event_id,
            "metadata": {"title": "Now Playing", "artist": "Relay Test"}
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let accepted: PublishMetadataResponse = read_json(response).await;
    assert!(accepted.accepted);
    assert_eq!(accepted.seq, 1);

    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/v1/liveitems/{}/metadata", accepted.event_id))
                .body(Body::empty())
                .expect("latest request"),
        )
        .await
        .expect("latest response");

    assert_eq!(response.status(), StatusCode::OK);
    let latest: LatestMetadataResponse = read_json(response).await;
    assert_eq!(latest.seq, 1);
    assert_eq!(
        latest.metadata,
        json!({"title": "Now Playing", "artist": "Relay Test"})
    );
}

#[tokio::test]
async fn create_accepts_trailing_slash() {
    let created = create_event_at(test_app(), "/v1/liveitems/").await;

    assert!(!created.event_id.is_empty());
    assert!(!created.broadcaster_token.is_empty());
}

#[tokio::test]
async fn publish_without_bearer_token_returns_401() {
    let router = test_app();
    let created = create_event(router.clone()).await;

    let response = publish(
        router,
        &created.event_id,
        None,
        json!({
            "event_id": created.event_id,
            "metadata": {}
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn publish_with_wrong_token_returns_403() {
    let router = test_app();
    let created = create_event(router.clone()).await;

    let response = publish(
        router,
        &created.event_id,
        Some("wrong"),
        json!({
            "event_id": created.event_id,
            "metadata": {}
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn unknown_event_id_returns_404() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/v1/liveitems/missing/metadata")
                .body(Body::empty())
                .expect("latest request"),
        )
        .await
        .expect("latest response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn health_returns_ok() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    assert_eq!(&bytes[..], b"ok");
}

#[tokio::test]
async fn sse_stream_receives_live_metadata_event() {
    let router = test_app();
    let created = create_event(router.clone()).await;

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/liveitems/{}/events", created.event_id))
                .body(Body::empty())
                .expect("events request"),
        )
        .await
        .expect("events response");

    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body();

    let response = publish(
        router,
        &created.event_id,
        Some(&created.broadcaster_token),
        json!({
            "event_id": created.event_id,
            "metadata": {"title": "Live"}
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let chunk = next_sse_chunk(&mut body).await;
    assert!(chunk.contains("event: metadata"), "{chunk}");
    assert!(chunk.contains("id: 1"), "{chunk}");
    assert!(chunk.contains("\"title\":\"Live\""), "{chunk}");
}

#[tokio::test]
async fn reconnect_with_last_event_id_replays_only_missed_events() {
    let router = test_app();
    let created = create_event(router.clone()).await;

    for index in 1..=3 {
        let response = publish(
            router.clone(),
            &created.event_id,
            Some(&created.broadcaster_token),
            json!({
                "event_id": created.event_id,
                "metadata": {"index": index}
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/v1/liveitems/{}/events", created.event_id))
                .header("Last-Event-ID", "1")
                .body(Body::empty())
                .expect("events request"),
        )
        .await
        .expect("events response");

    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body();

    let first = next_sse_chunk(&mut body).await;
    assert!(first.contains("id: 2"), "{first}");
    assert!(first.contains("\"index\":2"), "{first}");

    let second = next_sse_chunk(&mut body).await;
    assert!(second.contains("id: 3"), "{second}");
    assert!(second.contains("\"index\":3"), "{second}");
}
