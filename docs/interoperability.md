# Interoperability

This relay is one part of a chain that four repositories build separately. This
document says which behavior of this service other repositories depend on, and
which limits they must work around.

The full chain map lives in `v4vmm`:
`docs/architecture/broadcast-chain.md`.

## The Chain

```text
v4vmm ── writes MusicIndex tags into the audio file
             │
             ▼
a player plays the file (Mixxx, mpv, or liquidsoap)
             │
             ▼
a producer writes a musicindex.nowplaying/1 drop file
             │
             ▼
musicindex-live-publisher sends the direct live value payload
             │
             ▼
THIS SERVICE holds the payload and sends remoteValue
             │
             ├──▶ listener apps
             └──▶ v4vmm, to show what listeners receive
```

`musicindex-live-publisher` is the only sender. `v4vmm` creates live items and
reads snapshots, and sends no payloads.

## What Consumers Depend On

### Two body forms, separated by exact key match

A body with exactly the keys `event_id` and `metadata` is the wrapped form. Any
other object is a direct live value payload, and the relay passes it through
without interpretation.

Listener apps read the direct form and find payment splits in
`value.destinations`. A broadcaster that sends the wrapped form reaches no
listener with its splits, and this service reports no error for that.

`musicindex-live-publisher` has a unit test that asserts its payload never has
exactly the keys `event_id` and `metadata`. Do not change the separation rule
without a change in that repository.

### The create response fields

`POST /v1/liveitems` returns `event_id`, `broadcaster_token`, `metadata_url`,
`remote_value_url`, `events_url`, and `socket_io_url`. `v4vmm` stores these
fields in its event registry, because this service cannot list them later.

### A `404` means a dead event

`v4vmm` tests a stored event with `GET /v1/liveitems/{event_id}/metadata`. A
`404` tells the operator that the event no longer exists. Keep that status code
stable.

## Limits That Consumers Work Around

### State is in memory

A restart of this process discards live items, broadcaster tokens, latest
snapshots, and replay buffers. Every event dies, every stored token becomes
invalid, and listeners must tune to a new event identifier.

`v4vmm` therefore keeps its own registry of the events that it created, and
reports a dead event to the operator instead of a silent replacement.

### An idle event expires

The reaper removes an event when its last activity is older than the TTL. The
default is 24 hours.

An event therefore dies with no restart. A weekly show that reserves an
identifier on Monday finds it gone on Saturday. Consumers see the same `404`
for both death modes, so no consumer needs to separate them.

### No list route and no delete route

This service cannot tell a client which events it owns. A client that needs a
list must keep its own.

`Delete` in a control surface therefore means "forget locally". This service
discards its own record at the next restart.

### The token is returned one time

This service stores a SHA-256 hash of the broadcaster token and compares it in
constant time. It cannot return the token again.

A client that loses the token loses the event. `v4vmm` writes each token to a
file with mode `0600` for this reason, and never puts a token in a database or
a log.

## Requested Work

### Long-lived live items

A weekly show and a permanent station both need an event that survives a
restart of this process and the idle TTL. Today a restart forces new
identifiers and a new round of listener tuning, and a quiet day does the same.

ADR 0001 in this repository proposes the decision, and
`docs/plans/adr-0001-reserved-live-items-phase-plan.md` holds the work.

The requirement, from the 2026-09-06 chain review:

- An option to reserve a live item that survives a process restart.
- Durable storage for the identifier and the token hash of a reserved item.
- A clear difference between a short event, for one show, and a reserved
  event, for a station.

This is a decision for this repository. `v4vmm` ADR 0059 records it as
follow-up work and its event registry already stores what a resume needs.

### Per-transport delay for live payloads

`musicindex-live-publisher` holds each payload for a configured stream delay
before it sends the payload here. The delay exists so a live listener's app
flips the value block near the moment that listener hears the track change,
after the encoder, the icecast queue, and the player buffer.

That delay is a property of live delivery. It is not a property of the audio.
Applying it before the payload reaches this service means every consumer
receives the delayed version, including a snapshot read that a control surface
uses to show what listeners receive.

The question for this repository: should the delay be applied here, on the
socket.io emission only, instead of at the publisher? The publisher would then
send on sight, this service would hold the socket.io emission for the target
delay, and the HTTP snapshot would show the current truth without a delay.

Not decided. It needs a measurement of the real post-icecast delay first,
which nobody has taken. Recorded so the option is not lost.

## References

- `README.md` in this repository for the full API
- `v4vmm`: `docs/architecture/broadcast-chain.md`
- `v4vmm`: `docs/adr/0059-broadcast-control-surface.md`
- `musicindex-live-publisher`: `docs/architecture/broadcast-chain-boundaries.md`
- `musicindex-live-publisher`: `docs/adr/0002-nowplaying-drop-file-contract.md`
