# Rust Live Metadata Relay on Port 8018

  ## Summary

  Build a new independent Rust service, separate from Stophammer, that nginx serves under api.musicindex.org/v1/liveitems/* on 127.0.0.1:8018.

  The relay is memory-only for v1. It supports public event creation, returns a secret broadcaster token, accepts metadata updates only with that token, stores the latest
  snapshot, and fans updates out over SSE by server-generated event_id.

  ## Public API

  - POST /v1/liveitems
      - Public endpoint.
      - Creates a new live metadata event.
      - Response:

        {
          "event_id": "<random opaque id>",
          "broadcaster_token": "<random secret token>",
          "metadata_url": "/v1/liveitems/<event_id>/metadata",
          "events_url": "/v1/liveitems/<event_id>/events"
        }
  - POST /v1/liveitems/{event_id}/metadata
      - Requires Authorization: Bearer <broadcaster_token>.
      - Request body matches the v4vmm client contract:

        {
          "event_id": "<same event_id>",
          "metadata": {}
        }
      - Reject if path/body event_id differs.
      - On success, increments a per-event sequence, stores latest metadata, appends to replay buffer, and broadcasts an SSE metadata event.
      - Response:

        {
          "event_id": "<event_id>",
          "accepted": true,
          "seq": 1
        }
  - GET /v1/liveitems/{event_id}/metadata
      - Public latest-snapshot read.
      - Returns 404 if event does not exist or no metadata has been published yet.
      - Response includes event_id, latest seq, updated_at, and metadata.
  - GET /v1/liveitems/{event_id}/events
      - Public SSE stream.
      - Replays buffered events with seq > Last-Event-ID when the header is present and valid.
      - Emits frames:

        event: metadata
        id: <seq>
        data: {"event_id":"...","seq":1,"updated_at":"...","metadata":{...}}
      - Sends periodic keepalive comments.
  - GET /health
      - Returns ok.

  ## Implementation Changes

  - Create a new Rust crate/repo, recommended path: /home/citizen/build/musicindex-live-relay.
  - Use axum 0.8, tokio, serde, serde_json, tower-http, tracing, rand, sha2, and subtle.
  - Bind address comes from BIND, defaulting to 127.0.0.1:8018.
  - Keep relay state in Arc<RelayState>:
      - events: RwLock<HashMap<EventId, LiveEventState>>
      - each event stores token_hash, latest snapshot, monotonic seq, bounded replay buffer, broadcast sender, and last activity timestamp.
  - Generate:
      - event_id: URL-safe random opaque ID, at least 128 bits entropy.
      - broadcaster_token: URL-safe random secret, at least 256 bits entropy.
      - Store only sha256(token) and compare with constant-time equality.
  - Enforce limits:
      - max request body size: 64 KiB for metadata.
      - max metadata JSON depth/size by body limit only for v1.
      - replay buffer: 100 events per live item.
      - max active events: configurable, default 10,000.
      - max SSE connections: configurable, default 1,000.
      - TTL cleanup: background task expires events with no publish/listen activity for 24 hours by default.
  - Add systemd unit template:
      - service name: musicindex-live-relay.service
      - Environment=BIND=127.0.0.1:8018
      - restart on failure
      - no database/state directory required for v1.
  - Keep existing nginx route as-is:
      - /v1/liveitems/ proxies to 127.0.0.1:8018
      - all other API routes continue to Stophammer on 8008.

  ## Test Plan

  - Unit tests:
      - event creation returns unique event_id and token.
      - token hash validation accepts correct token and rejects wrong token.
      - path/body event_id mismatch returns 400.
      - publish without bearer token returns 401.
      - publish with wrong token returns 403.
      - latest snapshot returns the most recent metadata.
      - replay buffer is bounded to 100 events.
      - TTL cleanup removes inactive events.
  - Integration tests with Axum router:
      - create event, publish metadata, fetch latest snapshot.
      - open SSE stream, publish metadata, receive event: metadata with matching id.
      - reconnect with Last-Event-ID and receive only missed events.
      - unknown event_id returns 404.
      - /health returns 200.
  - Verification commands:
      - cargo fmt -- --check
      - cargo check
      - cargo test
      - cargo clippy -- -D warnings

  ## Assumptions

  - v1 is single-instance and memory-only; process restart loses events, tokens, latest snapshots, and replay buffers.
  - Public event creation is acceptable because event_id and broadcaster token are unguessable.
  - Podcast apps can read latest metadata and subscribe over SSE without authentication.
  - Broadcaster tokens are returned only once at creation time.
  - Stophammer is not modified for this v1 relay.
