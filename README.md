# MusicIndex Live Relay

`musicindex-live-relay` is a small Rust service for relaying live metadata updates over HTTP, Socket.IO, and server-sent events.

The service is intentionally memory-only for v1. Clients create a live item, receive a one-time broadcaster token, publish metadata with that token, and public listeners can receive `remoteValue` updates over Socket.IO. HTTP snapshot reads and SSE are kept as fallback/debug transports.

## Service Routes

By default the relay binds to `127.0.0.1:8018`. It serves these routes at whatever origin and path layout the host chooses to expose:

```text
POST /v1/liveitems
GET  /v1/liveitems/{event_id}/metadata
GET  /v1/liveitems/{event_id}/remoteValue
GET  /v1/liveitems/{event_id}/events
GET  /socket.io/*
```

The public URLs advertised in RSS should be based on the deployment's external origin. The `api.musicindex.org` examples below show the MusicIndex deployment, not a hosting requirement.

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
  "remote_value_url": "/v1/liveitems/<event_id>/remoteValue",
  "events_url": "/v1/liveitems/<event_id>/events",
  "socket_io_url": "/event?event_id=<event_id>"
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

This wrapped request body is what the v4vmm client sends. The path `event_id` and body `event_id` must match.

For compatibility with the widely used Socket.IO live value implementation, the relay also accepts the live value payload directly:

```json
{
  "title": "This is the title for this block.",
  "image": "https://example.com/art.png",
  "line": ["this is line 1", "this is line 2"],
  "link": {
    "text": "This is the text for the link",
    "url": "https://podcastindex.social"
  },
  "description": "this would be an area for something like show notes",
  "value": {
    "model": {
      "type": "lightning",
      "method": "keysend"
    },
    "destinations": []
  },
  "type": "person",
  "feedGuid": "optional-feed-guid",
  "itemGuid": "optional-item-guid"
}
```

On success, the relay increments the live item's sequence number, stores the latest snapshot, appends the update to the replay buffer, broadcasts a Socket.IO `remoteValue` event, and also broadcasts the same raw payload over SSE for fallback clients.

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
- `413` when the request body exceeds 64 KiB.
- `429` when per-event publish rate limit is exceeded.

The `Bearer` scheme is matched case-insensitively (`bearer`, `BEARER`, etc.).

The relay distinguishes the wrapped form from the direct form by exact key match: a request body is treated as wrapped only when the JSON object has exactly the two keys `event_id` and `metadata`. Any object with additional keys (or different keys) is treated as a direct live value payload.

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

The Socket.IO-compatible raw payload is also available at:

```http
GET /v1/liveitems/{event_id}/remoteValue
```

When the event exists but no metadata has been published yet, this endpoint returns `200 OK` with `{}` (mirrors Socket.IO's initial-emit behavior). It returns `404` only when the event itself does not exist.

### Subscribe With Socket.IO

Socket.IO is the primary app-facing transport because it matches the existing live value implementation used by The Split Kit.

Connect to namespace `/event` with `event_id` in the query string. Replace the origin with the relay host's public origin:

```js
import { io } from "socket.io-client";

const socket = io("https://api.musicindex.org/event", {
  query: { event_id: "<event_id>" }
});

socket.on("remoteValue", payload => {
  applyRemoteValue(payload);
});
```

The relay immediately emits the current `remoteValue` payload after a successful connection. If no metadata has been published yet, it emits `{}`. Future publishes emit the raw live value payload:

```json
{
  "title": "...",
  "image": "...",
  "line": ["..."],
  "value": {
    "model": {
      "type": "lightning",
      "method": "keysend"
    },
    "destinations": []
  },
  "type": "person"
}
```

The Socket.IO server uses namespace `/event` and the standard Engine.IO request path `/socket.io`.

### Subscribe With SSE

```http
GET /v1/liveitems/{event_id}/events
Accept: text/event-stream
```

The SSE stream is public and is intended as a fallback/debug transport. Each metadata update is emitted with the same event name and data shape used by the Socket.IO live value implementation:

```text
event: remoteValue
id: <seq>
data: {"title":"...","image":"...","line":["..."],"value":{...},"type":"person"}
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
      uri="https://api.musicindex.org/event?event_id=GCeSvW8qLdzSdXO9rrlKGg"
      protocol="socket.io" />
</podcast:liveItem>
```

Only the public event URI belongs in RSS. The broadcaster token returned by
`POST /v1/liveitems` is a write credential and must remain private. The relay
uses Socket.IO as the compatibility transport and emits the current
`remoteValue` payload on the `/event` namespace.

### Event URI Convention

Use the Socket.IO namespace URI from the create response in the tag:

```xml
<podcast:liveValue
    uri="https://api.musicindex.org/event?event_id=<event_id>"
    protocol="socket.io" />
```

Apps connect with the standard Socket.IO client:

```js
const socket = io("https://api.musicindex.org/event", {
  query: { event_id: "<event_id>" }
});

socket.on("remoteValue", applyRemoteValue);
```

The HTTP fallback/debug endpoints are:

```text
GET /v1/liveitems/<event_id>/metadata     wrapped current snapshot
GET /v1/liveitems/<event_id>/remoteValue  raw current remoteValue payload
GET /v1/liveitems/<event_id>/events       SSE stream of remoteValue events
```

The relay sends the current payload immediately on Socket.IO connect. The HTTP
snapshot endpoints are still useful for debugging, startup checks, and mobile
background modes where the app may choose to poll instead of keeping a live
connection open. If Socket.IO disconnects, the Socket.IO client handles
reconnection. Apps can also poll `/remoteValue` or `/metadata` as a conservative
fallback.

The direct SSE URL form is also possible:

```xml
<podcast:liveValue
    uri="https://api.musicindex.org/v1/liveitems/<event_id>/events"
    protocol="sse" />
```

The Socket.IO form is preferred for app compatibility. The SSE form remains
available for clients that deliberately choose the simpler one-way transport.

### `remoteValue` Payload

Published metadata should use the same shape as the Socket.IO `remoteValue`
payload. It acts as a live chapter plus a replacement value block, similar to a
`podcast:valueTimeSplit` in a recorded show, except the active block changes in
real time instead of being known ahead of time.

Recommended payload shape:

```json
{
  "title": "This is the title for this block.",
  "image": "https://example.com/art.png",
  "line": [
    "this is line 1",
    "this is line 2",
    "this is line 3",
    "this is line 4"
  ],
  "link": {
    "text": "This is the text for the link",
    "url": "https://podcastindex.social"
  },
  "description": "this would be an area for something like show notes",
  "value": {
    "model": {
      "type": "lightning",
      "method": "keysend"
    },
    "destinations": [
      {
        "name": "The Split Kit",
        "address": "030a58b8653d32b99200a2334cfe913e51dc7d155aa0116c176657a4f1722677a3",
        "customKey": "696969",
        "customValue": "boPNspwDdt7axih5DfKs",
        "split": "5",
        "fee": "false"
      }
    ]
  },
  "type": "person",
  "feedGuid": "optional-feed-guid",
  "itemGuid": "optional-item-guid"
}
```

When this payload is active, supporting apps should use `value` as the
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
| `MAX_CREATES_PER_SEC` | `50` | Global cap on `POST /v1/liveitems` per second. Returns 429 when exceeded. |
| `MAX_PUBLISHES_PER_EVENT_PER_SEC` | `20` | Per-event cap on metadata publishes per second. Returns 429 when exceeded. |

Metadata request bodies are limited to 64 KiB. The body limit applies only to `POST /v1/liveitems/{event_id}/metadata`; other routes are unconstrained.

Per-IP rate limiting is the responsibility of the front-end proxy (e.g. nginx). The relay's `MAX_CREATES_PER_SEC` is a global safety bound, not a per-client limit.

CORS is enabled with a permissive policy (any origin, method, and header) so browser-side podcast apps can consume the SSE and HTTP fallback endpoints directly. Lock this down at the proxy if you need stricter origin policy.

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

The relay can be exposed directly or behind any reverse proxy that preserves the service routes. MusicIndex currently deploys it behind nginx at `api.musicindex.org`; that deployment uses:

- `/v1/liveitems` proxies to `127.0.0.1:8018`.
- `/v1/liveitems/` proxies to `127.0.0.1:8018`.
- `/socket.io/` proxies to `127.0.0.1:8018`.
- Other API routes continue to the existing Stophammer service.

Example nginx locations for that split deployment:

```nginx
location = /v1/liveitems {
    proxy_pass http://127.0.0.1:8018;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}

location ^~ /socket.io/ {
    proxy_pass http://127.0.0.1:8018;
    proxy_http_version 1.1;
    proxy_buffering off;
    proxy_cache off;
    proxy_read_timeout 1h;

    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
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

    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
}
```

The `proxy_pass` target intentionally has no trailing slash so nginx preserves the relay paths when forwarding to the service.

If nginx returns `502 Bad Gateway` for `/v1/liveitems`, nginx is matching the route but cannot reach the relay on `127.0.0.1:8018`. Check the service on the host:

```sh
sudo systemctl status musicindex-live-relay --no-pager
sudo journalctl -u musicindex-live-relay -n 100 --no-pager
ss -ltnp | grep 8018
curl -i http://127.0.0.1:8018/health
curl -i -X POST http://127.0.0.1:8018/v1/liveitems
curl -i 'http://127.0.0.1:8018/socket.io/?EIO=4&transport=polling'
```

After the relay is listening locally, reload nginx and verify the proxied health route:

```sh
sudo nginx -t
sudo systemctl reload nginx
curl -i https://api.musicindex.org/v1/liveitems/health
curl -i -X POST https://api.musicindex.org/v1/liveitems
```
