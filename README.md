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

## RSS `podcast:liveValue`

The relay is intended to be advertised from a Podcasting 2.0 live item with an
experimental `podcast:liveValue` tag. This follows the same idea as The Split
Kit's live value work: the RSS feed tells podcast apps which live metadata
transport to use and where to connect, then the live server sends the current
chapter/value block while the stream is playing.

For this relay, the recommended RSS shape is:

```xml
<podcast:liveItem
    status="live"
    start="2026-04-28T00:00:00Z"
    end="2026-04-28T03:00:00Z">

  <title>My Live Music Show</title>
  <guid isPermaLink="false">my-show-live-2026-04-28</guid>

  <enclosure
      url="https://stream.example.com/live.mp3"
      type="audio/mpeg"
      length="1" />

  <podcast:contentLink href="https://example.com/live">
    Listen live
  </podcast:contentLink>

  <podcast:value type="lightning" method="keysend" suggested="0.00000015000">
    <podcast:valueRecipient
        name="Default show split"
        type="node"
        address="030000000000000000000000000000000000000000000000000000000000000000"
        split="100" />
  </podcast:value>

  <podcast:liveValue
      uri="https://api.musicindex.org/v1/liveitems/GCeSvW8qLdzSdXO9rrlKGg"
      protocol="sse" />
</podcast:liveItem>
```

Only the public event URI belongs in RSS. The broadcaster token returned by
`POST /v1/liveitems` is a write credential and must remain private.

### Why `protocol="sse"`

The `protocol` attribute should describe the client transport an app needs, not
the brand or implementation of the relay. The Split Kit-style tag uses a
protocol value such as `socket.io` or `socket-io` to tell apps to open a
Socket.IO client. This relay uses standard Server-Sent Events, so `sse` is the
clean equivalent.

Avoid values such as `musicindex-live-relay` in RSS. They force apps to special
case one service even though the transport is ordinary SSE. A generic
`protocol="sse"` lets other hosts run compatible relays as long as they expose
the same event URI shape.

### Event URI Convention

Use the base live item URI in the tag:

```xml
<podcast:liveValue
    uri="https://api.musicindex.org/v1/liveitems/<event_id>"
    protocol="sse" />
```

Apps derive the two public read endpoints from that base URI:

```text
GET <uri>/metadata  current snapshot
GET <uri>/events    SSE stream
```

The snapshot endpoint matters because podcast apps may start playback in the
middle of a live show, reconnect after losing network, or run in a mobile
background mode where a persistent SSE connection is not reliable. Apps should
read `/metadata` when playback starts, then subscribe to `/events` while they
can keep a live connection open. If the stream disconnects, the app can reconnect
with `Last-Event-ID` or poll `/metadata` as a conservative fallback.

The direct SSE URL form is also possible:

```xml
<podcast:liveValue
    uri="https://api.musicindex.org/v1/liveitems/<event_id>/events"
    protocol="sse" />
```

The base URI form is preferred because it gives apps both the current snapshot
and event stream without inventing extra attributes.

### Metadata Payload

Published metadata should be treated as a live chapter plus a replacement value
block. It has the same role as a `podcast:valueTimeSplit` in a recorded show,
except the active block changes in real time instead of being known ahead of
time.

Recommended payload shape:

```json
{
  "title": "Song Title",
  "author": "Artist Name",
  "podcastName": "Album or Show Name",
  "image": "https://example.com/art.jpg",
  "link": {
    "text": "Open track",
    "url": "https://example.com/track"
  },
  "value": {
    "type": "lightning",
    "method": "keysend",
    "destinations": [
      {
        "name": "Artist",
        "address": "020000000000000000000000000000000000000000000000000000000000000000",
        "customKey": "696969",
        "customValue": "artist-custom-value",
        "split": "95",
        "fee": "false"
      },
      {
        "name": "Show",
        "address": "030000000000000000000000000000000000000000000000000000000000000000",
        "split": "5",
        "fee": "true"
      }
    ]
  },
  "feedGuid": "optional-feed-guid",
  "itemGuid": "optional-item-guid"
}
```

When this payload is active, supporting apps should use `metadata.value` as the
current payment split. The static `<podcast:value>` inside the live item remains
the fallback/default split for apps that do not support `podcast:liveValue`, for
periods before the first live update, and for reconnect failures.

### Health

```http
GET /health
```

Returns:

```text
ok
```

`GET /v1/liveitems/health` returns the same response and is useful for checking that nginx is reaching the live relay specifically. On `api.musicindex.org`, the top-level `/health` route may belong to another API service.

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

If nginx returns `502 Bad Gateway` for `/v1/liveitems`, nginx is matching the route but cannot reach the relay on `127.0.0.1:8018`. Check the service on the host:

```sh
sudo systemctl status musicindex-live-relay --no-pager
sudo journalctl -u musicindex-live-relay -n 100 --no-pager
ss -ltnp | grep 8018
curl -i http://127.0.0.1:8018/health
curl -i -X POST http://127.0.0.1:8018/v1/liveitems
```

After the relay is listening locally, reload nginx and verify the proxied health route:

```sh
sudo nginx -t
sudo systemctl reload nginx
curl -i https://api.musicindex.org/v1/liveitems/health
curl -i -X POST https://api.musicindex.org/v1/liveitems
```
