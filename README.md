# MusicIndex Live Relay

`musicindex-live-relay` is a small Rust service for relaying live metadata updates over HTTP and server-sent events.

The service is intentionally memory-only for v1. Clients create a live item, receive a one-time broadcaster token, publish metadata with that token, and public listeners can read the latest snapshot or subscribe to updates over SSE.

## Service

By default the relay binds to `127.0.0.1:8018` and is intended to sit behind nginx at:

```text
/v1/liveitems
/v1/liveitems/*
```

All state is process-local. Restarting the process drops live items, broadcaster tokens, latest snapshots, and replay buffers.

## API

### Create Live Item

```http
POST /v1/liveitems
```

Creates a new live metadata event. This endpoint is public. `POST /v1/liveitems/` is also accepted for trailing-slash tolerant clients and proxies.

Response:

```json
{
  "event_id": "<random opaque id>",
  "broadcaster_token": "<random secret token>",
  "metadata_url": "/v1/liveitems/<event_id>/metadata",
  "events_url": "/v1/liveitems/<event_id>/events"
}
```

The broadcaster token is returned only once. The service stores only a SHA-256 hash of the token and validates it with constant-time comparison.

### Publish Metadata

```http
POST /v1/liveitems/{event_id}/metadata
Authorization: Bearer <broadcaster_token>
Content-Type: application/json
```

Request body:

```json
{
  "event_id": "<same event_id>",
  "metadata": {}
}
```

The path `event_id` and body `event_id` must match. On success, the relay increments the live item's sequence number, stores the latest snapshot, appends the update to the replay buffer, and broadcasts an SSE `metadata` event.

Response:

```json
{
  "event_id": "<event_id>",
  "accepted": true,
  "seq": 1
}
```

Status codes:

- `400` when the path and body event IDs differ.
- `401` when the bearer token is missing or malformed.
- `403` when the bearer token is wrong.
- `404` when the event does not exist.

### Read Latest Metadata

```http
GET /v1/liveitems/{event_id}/metadata
```

Returns the latest published metadata snapshot.

Response:

```json
{
  "event_id": "<event_id>",
  "seq": 1,
  "updated_at": "2026-04-27T12:34:56Z",
  "metadata": {}
}
```

Returns `404` when the event does not exist or no metadata has been published yet.

### Subscribe To Metadata Events

```http
GET /v1/liveitems/{event_id}/events
Accept: text/event-stream
```

The SSE stream is public. Each metadata update is emitted as:

```text
event: metadata
id: <seq>
data: {"event_id":"...","seq":1,"updated_at":"...","metadata":{}}
```

When `Last-Event-ID` is present and valid, the relay replays buffered events with `seq > Last-Event-ID` before streaming live updates. The replay buffer keeps the last 100 metadata updates per live item.

The stream also sends periodic keepalive comments.

### Health

```http
GET /health
```

Returns:

```text
ok
```

## Configuration

Configuration is read from environment variables.

| Variable | Default | Description |
| --- | --- | --- |
| `BIND` | `127.0.0.1:8018` | Socket address to listen on. |
| `MAX_ACTIVE_EVENTS` | `10000` | Maximum number of live items kept in memory. |
| `MAX_SSE_CONNECTIONS` | `1000` | Maximum concurrent SSE streams. |
| `EVENT_TTL_SECS` | `86400` | Time before inactive events expire. |

Metadata request bodies are limited to 64 KiB.

## Run

```sh
cargo run
```

With explicit configuration:

```sh
BIND=127.0.0.1:8018 MAX_ACTIVE_EVENTS=10000 cargo run
```

## Test

```sh
cargo fmt -- --check
cargo check
cargo test
cargo clippy -- -D warnings
```

## Deployment

A systemd unit template is included at:

```text
systemd/musicindex-live-relay.service
```

The unit runs `/usr/local/bin/musicindex-live-relay`, sets `BIND=127.0.0.1:8018`, and restarts on failure.

The intended nginx routing is:

- `/v1/liveitems` proxies to `127.0.0.1:8018`.
- `/v1/liveitems/` proxies to `127.0.0.1:8018`.
- Other API routes continue to the existing Stophammer service.

Example nginx locations:

```nginx
location = /v1/liveitems {
    proxy_pass http://127.0.0.1:8018;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}

location ^~ /v1/liveitems/ {
    proxy_pass http://127.0.0.1:8018;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;

    proxy_buffering off;
    proxy_cache off;
    proxy_read_timeout 1h;
}
```

The `proxy_pass` target intentionally has no trailing slash so nginx preserves `/v1/liveitems/...` when forwarding to the relay.
