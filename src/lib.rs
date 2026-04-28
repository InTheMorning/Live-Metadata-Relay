use std::{
    collections::{HashMap, VecDeque},
    convert::Infallible,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::{RwLock, broadcast};
use tower_http::limit::RequestBodyLimitLayer;

const DEFAULT_BIND: &str = "127.0.0.1:8018";
const DEFAULT_MAX_ACTIVE_EVENTS: usize = 10_000;
const DEFAULT_MAX_SSE_CONNECTIONS: usize = 1_000;
const DEFAULT_TTL_SECS: u64 = 24 * 60 * 60;
const METADATA_BODY_LIMIT_BYTES: usize = 64 * 1024;
const REPLAY_CAPACITY: usize = 100;
const BROADCAST_CAPACITY: usize = 128;
const TOKEN_BYTES: usize = 32;
const EVENT_ID_BYTES: usize = 16;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub bind: SocketAddr,
    pub max_active_events: usize,
    pub max_sse_connections: usize,
    pub ttl: Duration,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            bind: env_parse("BIND", DEFAULT_BIND.parse().expect("default bind is valid"))?,
            max_active_events: env_parse("MAX_ACTIVE_EVENTS", DEFAULT_MAX_ACTIVE_EVENTS)?,
            max_sse_connections: env_parse("MAX_SSE_CONNECTIONS", DEFAULT_MAX_SSE_CONNECTIONS)?,
            ttl: Duration::from_secs(env_parse("EVENT_TTL_SECS", DEFAULT_TTL_SECS)?),
        })
    }

    pub fn for_tests() -> Self {
        Self {
            bind: DEFAULT_BIND.parse().expect("default bind is valid"),
            max_active_events: DEFAULT_MAX_ACTIVE_EVENTS,
            max_sse_connections: DEFAULT_MAX_SSE_CONNECTIONS,
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
        }
    }
}

#[derive(Debug)]
pub struct ConfigError {
    key: &'static str,
    value: String,
    message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid {} value {:?}: {}",
            self.key, self.value, self.message
        )
    }
}

impl std::error::Error for ConfigError {}

fn env_parse<T>(key: &'static str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(value) => value.parse().map_err(|err: T::Err| ConfigError {
            key,
            value,
            message: err.to_string(),
        }),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(std::env::VarError::NotUnicode(value)) => Err(ConfigError {
            key,
            value: format!("{value:?}"),
            message: "not valid unicode".to_string(),
        }),
    }
}

#[derive(Clone)]
pub struct RelayState {
    inner: Arc<RelayStateInner>,
}

struct RelayStateInner {
    config: AppConfig,
    events: RwLock<HashMap<String, Arc<LiveEventState>>>,
    sse_connections: AtomicUsize,
    clock: Clock,
}

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

impl RelayState {
    pub fn new(config: AppConfig) -> Self {
        Self::with_clock(config, Arc::new(epoch_seconds))
    }

    pub fn with_clock(config: AppConfig, clock: Clock) -> Self {
        Self {
            inner: Arc::new(RelayStateInner {
                config,
                events: RwLock::new(HashMap::new()),
                sse_connections: AtomicUsize::new(0),
                clock,
            }),
        }
    }

    fn now(&self) -> u64 {
        (self.inner.clock)()
    }

    pub async fn create_event(&self) -> Result<CreateEventResponse, ApiError> {
        let mut events = self.inner.events.write().await;
        if events.len() >= self.inner.config.max_active_events {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "max_active_events_reached",
            ));
        }

        let event_id = unique_event_id(&events);
        let token = random_urlsafe(TOKEN_BYTES);
        let event = Arc::new(LiveEventState::new(hash_token(&token), self.now()));

        events.insert(event_id.clone(), event);

        Ok(CreateEventResponse {
            event_id: event_id.clone(),
            broadcaster_token: token,
            metadata_url: format!("/v1/liveitems/{event_id}/metadata"),
            events_url: format!("/v1/liveitems/{event_id}/events"),
        })
    }

    async fn get_event(&self, event_id: &str) -> Result<Arc<LiveEventState>, ApiError> {
        self.inner
            .events
            .read()
            .await
            .get(event_id)
            .cloned()
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "event_not_found"))
    }

    pub async fn publish_metadata(
        &self,
        event_id: &str,
        token: &str,
        body: PublishMetadataRequest,
    ) -> Result<PublishMetadataResponse, ApiError> {
        if body.event_id != event_id {
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "event_id_mismatch"));
        }

        let event = self.get_event(event_id).await?;
        if !event.token_matches(token) {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "invalid_token"));
        }

        let mut inner = event.inner.write().await;
        inner.seq += 1;
        inner.last_activity = self.now();

        let snapshot = MetadataSnapshot {
            event_id: event_id.to_string(),
            seq: inner.seq,
            updated_at: format_timestamp(inner.last_activity),
            metadata: body.metadata,
        };

        inner.latest = Some(snapshot.clone());
        inner.replay.push_back(snapshot.clone());
        while inner.replay.len() > REPLAY_CAPACITY {
            inner.replay.pop_front();
        }

        drop(inner);
        let _ = event.sender.send(snapshot);

        Ok(PublishMetadataResponse {
            event_id: event_id.to_string(),
            accepted: true,
            seq: event.current_seq().await,
        })
    }

    pub async fn latest_metadata(
        &self,
        event_id: &str,
    ) -> Result<LatestMetadataResponse, ApiError> {
        let event = self.get_event(event_id).await?;
        let inner = event.inner.read().await;
        let snapshot = inner
            .latest
            .clone()
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "metadata_not_found"))?;

        Ok(LatestMetadataResponse {
            event_id: snapshot.event_id,
            seq: snapshot.seq,
            updated_at: snapshot.updated_at,
            metadata: snapshot.metadata,
        })
    }

    async fn subscribe(
        &self,
        event_id: &str,
        last_event_id: Option<u64>,
    ) -> Result<EventSubscription, ApiError> {
        let previous = self.inner.sse_connections.fetch_add(1, Ordering::AcqRel);
        if previous >= self.inner.config.max_sse_connections {
            self.inner.sse_connections.fetch_sub(1, Ordering::AcqRel);
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "max_sse_connections_reached",
            ));
        }

        let event = match self.get_event(event_id).await {
            Ok(event) => event,
            Err(err) => {
                self.inner.sse_connections.fetch_sub(1, Ordering::AcqRel);
                return Err(err);
            }
        };

        let replay = {
            let mut inner = event.inner.write().await;
            inner.last_activity = self.now();
            match last_event_id {
                Some(last_event_id) => inner
                    .replay
                    .iter()
                    .filter(|snapshot| snapshot.seq > last_event_id)
                    .cloned()
                    .collect(),
                None => Vec::new(),
            }
        };
        let receiver = event.sender.subscribe();

        Ok(EventSubscription {
            state: self.clone(),
            event,
            replay,
            receiver,
        })
    }

    pub async fn cleanup_expired(&self) -> usize {
        let cutoff = self.now().saturating_sub(self.inner.config.ttl.as_secs());
        let mut events = self.inner.events.write().await;
        let before = events.len();
        events.retain(|_, event| {
            event
                .inner
                .try_read()
                .map(|inner| inner.last_activity >= cutoff)
                .unwrap_or(true)
        });
        before - events.len()
    }
}

fn unique_event_id(events: &HashMap<String, Arc<LiveEventState>>) -> String {
    loop {
        let event_id = random_urlsafe(EVENT_ID_BYTES);
        if !events.contains_key(&event_id) {
            return event_id;
        }
    }
}

struct LiveEventState {
    token_hash: [u8; 32],
    inner: RwLock<LiveEventInner>,
    sender: broadcast::Sender<MetadataSnapshot>,
}

struct LiveEventInner {
    seq: u64,
    latest: Option<MetadataSnapshot>,
    replay: VecDeque<MetadataSnapshot>,
    last_activity: u64,
}

impl LiveEventState {
    fn new(token_hash: [u8; 32], now: u64) -> Self {
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            token_hash,
            inner: RwLock::new(LiveEventInner {
                seq: 0,
                latest: None,
                replay: VecDeque::with_capacity(REPLAY_CAPACITY),
                last_activity: now,
            }),
            sender,
        }
    }

    fn token_matches(&self, token: &str) -> bool {
        let candidate = hash_token(token);
        self.token_hash
            .as_slice()
            .ct_eq(candidate.as_slice())
            .into()
    }

    async fn current_seq(&self) -> u64 {
        self.inner.read().await.seq
    }
}

struct EventSubscription {
    state: RelayState,
    event: Arc<LiveEventState>,
    replay: Vec<MetadataSnapshot>,
    receiver: broadcast::Receiver<MetadataSnapshot>,
}

impl Drop for EventSubscription {
    fn drop(&mut self) {
        self.state
            .inner
            .sse_connections
            .fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug, Deserialize)]
pub struct PublishMetadataRequest {
    pub event_id: String,
    pub metadata: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventResponse {
    pub event_id: String,
    pub broadcaster_token: String,
    pub metadata_url: String,
    pub events_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublishMetadataResponse {
    pub event_id: String,
    pub accepted: bool,
    pub seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestMetadataResponse {
    pub event_id: String,
    pub seq: u64,
    pub updated_at: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetadataSnapshot {
    event_id: String,
    seq: u64,
    updated_at: String,
    metadata: Value,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str) -> Self {
        Self { status, code }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": self.code,
            })),
        )
            .into_response()
    }
}

pub fn app(state: RelayState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/liveitems/health", get(health))
        .route("/v1/liveitems", post(create_event))
        .route("/v1/liveitems/", post(create_event))
        .route(
            "/v1/liveitems/{event_id}/metadata",
            get(latest_metadata).post(publish_metadata),
        )
        .route("/v1/liveitems/{event_id}/events", get(events))
        .layer(RequestBodyLimitLayer::new(METADATA_BODY_LIMIT_BYTES))
        .with_state(state)
}

pub fn spawn_cleanup_task(state: RelayState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval = std::cmp::min(state.inner.config.ttl / 4, Duration::from_secs(60 * 60));
        let interval = std::cmp::max(interval, Duration::from_secs(60));
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;
            let expired = state.cleanup_expired().await;
            if expired > 0 {
                tracing::info!(expired, "expired inactive live items");
            }
        }
    })
}

async fn health() -> &'static str {
    "ok"
}

async fn create_event(
    State(state): State<RelayState>,
) -> Result<Json<CreateEventResponse>, ApiError> {
    Ok(Json(state.create_event().await?))
}

async fn publish_metadata(
    State(state): State<RelayState>,
    Path(event_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<PublishMetadataRequest>,
) -> Result<Json<PublishMetadataResponse>, ApiError> {
    let token = bearer_token(&headers)?;
    Ok(Json(state.publish_metadata(&event_id, token, body).await?))
}

async fn latest_metadata(
    State(state): State<RelayState>,
    Path(event_id): Path<String>,
) -> Result<Json<LatestMetadataResponse>, ApiError> {
    Ok(Json(state.latest_metadata(&event_id).await?))
}

async fn events(
    State(state): State<RelayState>,
    Path(event_id): Path<String>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    let subscription = state.subscribe(&event_id, last_event_id).await?;

    let stream = async_stream::stream! {
        let mut subscription = subscription;

        for snapshot in subscription.replay.drain(..) {
            yield Ok(snapshot_to_sse(snapshot));
        }

        loop {
            match subscription.receiver.recv().await {
                Ok(snapshot) => {
                    if let Ok(mut inner) = subscription.event.inner.try_write() {
                        inner.last_activity = subscription.state.now();
                    }
                    yield Ok(snapshot_to_sse(snapshot));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

fn snapshot_to_sse(snapshot: MetadataSnapshot) -> Event {
    Event::default()
        .event("metadata")
        .id(snapshot.seq.to_string())
        .json_data(snapshot)
        .expect("metadata snapshots are serializable")
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "missing_bearer_token"))?
        .to_str()
        .map_err(|_| ApiError::new(StatusCode::UNAUTHORIZED, "invalid_authorization_header"))?;

    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "invalid_bearer_token"))
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn random_urlsafe(byte_count: usize) -> String {
    let mut bytes = vec![0; byte_count];
    rand::rng().fill_bytes(&mut bytes);
    base64_urlsafe_no_pad(&bytes)
}

fn base64_urlsafe_no_pad(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        }
    }
    out
}

fn epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before the Unix epoch")
        .as_secs()
}

fn format_timestamp(timestamp: u64) -> String {
    OffsetDateTime::from_unix_timestamp(timestamp.try_into().expect("timestamp fits in i64"))
        .expect("timestamp is in range")
        .format(&Rfc3339)
        .expect("timestamp can be formatted as RFC 3339")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    fn test_state(now: u64) -> (RelayState, Arc<AtomicU64>) {
        let clock = Arc::new(AtomicU64::new(now));
        let state = RelayState::with_clock(
            AppConfig {
                bind: DEFAULT_BIND.parse().expect("default bind is valid"),
                max_active_events: DEFAULT_MAX_ACTIVE_EVENTS,
                max_sse_connections: DEFAULT_MAX_SSE_CONNECTIONS,
                ttl: Duration::from_secs(10),
            },
            {
                let clock = clock.clone();
                Arc::new(move || clock.load(Ordering::SeqCst))
            },
        );
        (state, clock)
    }

    #[tokio::test]
    async fn event_creation_returns_unique_event_id_and_token() {
        let (state, _) = test_state(1);
        let first = state.create_event().await.expect("create first event");
        let second = state.create_event().await.expect("create second event");

        assert_ne!(first.event_id, second.event_id);
        assert_ne!(first.broadcaster_token, second.broadcaster_token);
        assert!(first.metadata_url.ends_with("/metadata"));
        assert!(first.events_url.ends_with("/events"));
    }

    #[tokio::test]
    async fn token_hash_validation_accepts_correct_token_and_rejects_wrong_token() {
        let (state, _) = test_state(1);
        let created = state.create_event().await.expect("create event");

        state
            .publish_metadata(
                &created.event_id,
                &created.broadcaster_token,
                PublishMetadataRequest {
                    event_id: created.event_id.clone(),
                    metadata: json!({"title": "First"}),
                },
            )
            .await
            .expect("valid token publishes");

        let err = state
            .publish_metadata(
                &created.event_id,
                "wrong",
                PublishMetadataRequest {
                    event_id: created.event_id.clone(),
                    metadata: json!({"title": "Second"}),
                },
            )
            .await
            .expect_err("wrong token rejected");
        assert_eq!(err.status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn path_body_event_id_mismatch_returns_400() {
        let (state, _) = test_state(1);
        let created = state.create_event().await.expect("create event");
        let err = state
            .publish_metadata(
                &created.event_id,
                &created.broadcaster_token,
                PublishMetadataRequest {
                    event_id: "different".to_string(),
                    metadata: json!({}),
                },
            )
            .await
            .expect_err("mismatch rejected");

        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn latest_snapshot_returns_most_recent_metadata() {
        let (state, _) = test_state(1);
        let created = state.create_event().await.expect("create event");

        for title in ["First", "Second"] {
            state
                .publish_metadata(
                    &created.event_id,
                    &created.broadcaster_token,
                    PublishMetadataRequest {
                        event_id: created.event_id.clone(),
                        metadata: json!({"title": title}),
                    },
                )
                .await
                .expect("publish");
        }

        let latest = state
            .latest_metadata(&created.event_id)
            .await
            .expect("latest metadata");
        assert_eq!(latest.seq, 2);
        assert_eq!(latest.metadata, json!({"title": "Second"}));
    }

    #[tokio::test]
    async fn replay_buffer_is_bounded_to_100_events() {
        let (state, _) = test_state(1);
        let created = state.create_event().await.expect("create event");

        for index in 0..105 {
            state
                .publish_metadata(
                    &created.event_id,
                    &created.broadcaster_token,
                    PublishMetadataRequest {
                        event_id: created.event_id.clone(),
                        metadata: json!({"index": index}),
                    },
                )
                .await
                .expect("publish");
        }

        let event = state.get_event(&created.event_id).await.expect("event");
        let inner = event.inner.read().await;
        assert_eq!(inner.replay.len(), REPLAY_CAPACITY);
        assert_eq!(inner.replay.front().expect("front").seq, 6);
        assert_eq!(inner.replay.back().expect("back").seq, 105);
    }

    #[tokio::test]
    async fn ttl_cleanup_removes_inactive_events() {
        let (state, clock) = test_state(100);
        let created = state.create_event().await.expect("create event");

        clock.store(111, Ordering::SeqCst);

        assert_eq!(state.cleanup_expired().await, 1);
        let err = match state.get_event(&created.event_id).await {
            Ok(_) => panic!("event expired"),
            Err(err) => err,
        };
        assert_eq!(err.status, StatusCode::NOT_FOUND);
    }
}
