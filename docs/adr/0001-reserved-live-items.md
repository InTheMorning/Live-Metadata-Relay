# ADR 0001: Reserved Live Items

## Status

Accepted - 2026-09-06.

Amended 2026-09-06: the storage format is SQLite, the state file lives in the
systemd `StateDirectory`, and a reserved item count limit is configurable.
The operator answered those three questions after review.

## Context

This service keeps every live item in memory. `RelayState` holds a
`HashMap` behind an `RwLock`, and nothing writes to disk.

An event therefore dies in two ways:

- A restart of the process discards every live item, every token hash, every
  snapshot, and every replay buffer.
- The reaper removes an event after the idle TTL. The default is 24 hours.

Both are correct for a one-time show. A broadcaster creates an event, runs the
show, and never needs the identifier again.

Two other uses do not fit that model:

- A repeating show. The same audience returns each week to the same live value
  block in an RSS feed. A new identifier for each show makes every listener
  find the feed again.
- A permanent station. It broadcasts continuously and needs one identifier that
  does not change.

`v4vmm` ADR 0059 records the same need from the client side. That app now keeps
a local registry of the events it created, because this service can neither
list them nor return a token a second time. The registry cannot fix a dead
event. Only this service can.

The broadcaster token is the harder half. This service returns it once and
keeps a SHA-256 hash. After a restart, the hash is gone, so a token that a
broadcaster stored correctly becomes invalid through no fault of the
broadcaster.

## Decision

Add a second class of live item. The two classes are `ephemeral` and
`reserved`.

### Ephemeral Items Do Not Change

`POST /v1/liveitems` keeps its behavior: public, in memory, idle TTL, and lost
on a restart. Every current client keeps working.

### Reserved Items Are Durable

A reserved item stores its identity on disk:

- `event_id`
- `token_hash`
- `label`, an operator name
- `created_at`

A restart restores those fields, so a stored broadcaster token stays valid.

The storage is a SQLite file. SQLite gives a schema, safe concurrent writes,
and a migration path for the broadcaster identity work that follows. The file
lives in the systemd `StateDirectory`, which is
`/var/lib/musicindex-live-relay/` for the packaged unit. systemd creates that
directory with the correct owner and mode.

The number of reserved items is limited by a configuration value, in the shape
of `max_active_events`. A reserved item is permanent and consumes disk, so it
needs a bound that is separate from the ephemeral traffic limit.

### A Reserved Item Stores No Payload

**The latest snapshot and the replay buffer are never written to disk.** After a
restart, a reserved item exists and serves `{}` until the next publish.

This is a payment rule, not an optimization. A restored snapshot would tell
listener apps to pay the destinations of a track that stopped playing hours
ago. An empty response is correct, and the broadcaster refills it within one
publish interval.

### A Reserved Item Needs A Credential

Reservation needs an admin credential in the request. `POST /v1/liveitems` is
public today, and a public route that consumes disk is an unbounded growth
surface.

The credential is one configured admin token, compared in constant time, in the
shape of the existing broadcaster token check.

This admin token is the operator credential of a wider identity model that a
later ADR defines. The options for that model are recorded in
`docs/research/broadcaster-identity-options.md`. That ADR makes the relay a public service where every create
needs a credential and every quota is counted for each credential. This ADR
does not wait for it, because the store boundary and the no-payload rule do not
depend on identity.

### Reserved Items Get List And Delete

The credential makes both routes safe and useful:

- list the reserved items, with identifier, label, and last activity
- delete one reserved item

Neither route applies to an ephemeral item. Ephemeral items stay invisible and
undeletable, as they are today.

### A Reserved Item Ignores The Idle TTL

The reaper skips reserved items. A station that plays nothing overnight keeps
its identifier.

## Invariants

- Ephemeral behavior does not change.
- No payload, snapshot, or replay buffer reaches disk.
- The stored value for a token is a hash. The token itself is never stored.
- A route that writes to durable storage needs the admin credential.
- The reaper never removes a reserved item.
- A `404` still means the event does not exist, for both classes.
- Token comparison stays constant time, for the admin token and for the
  broadcaster token.

## Alternatives Considered

### Persist Every Live Item

Rejected. It changes the memory-only premise for every client, and it turns a
public route into a disk write. It also invites the restored-snapshot defect,
because the simple version of that change stores the latest payload.

### Persist The Latest Snapshot Too

Rejected, and the reason is worth stating. It looks helpful: a restart would
restore what listeners were receiving. It is a payment defect. Listener apps
would route boosts to the destinations of a track that is no longer playing,
and neither the operator nor the listener would see an error.

### Use An External Store

Rejected for now. A store such as Redis moves the state out of the process and
solves the same problem, at the cost of a second service to deploy, monitor,
and secure. A single file has no such cost. Revisit this when more than one
relay instance must share state.

### Only Increase The TTL

Rejected. A longer TTL does not survive a restart, which is the failure that
matters most. It also holds abandoned events for longer.

## Consequences

Positive:

- A repeating show and a permanent station keep one identifier.
- A stored broadcaster token stays valid across a restart.
- The client registry in `v4vmm` gains a resume path that can succeed.
- List and delete become possible for the items an operator owns.

Negative and risks:

- The service gains a SQLite file to back up, and a deployment gains a state
  directory.
- SQLite is a new dependency for a service that had none for storage.
- An admin credential is a new secret to configure and rotate.
- A token hash now sits on disk. It is a hash, not a token, but it is a longer
  lived artifact than before.
- Two classes of item mean two code paths in create, in the reaper, and in the
  tests.
- A reserved item serves `{}` after a restart until the next publish. Document
  that in the runbook so it does not read as a defect.

## Follow-Up Work

- A rotation route for the broadcaster token of a reserved item. Today a lost
  token still means a new item.
- Metrics for reserved item count and storage size.
- Multi-instance state sharing, if a second relay instance is ever deployed.

## References

- `docs/interoperability.md`
- `docs/plans/adr-0001-reserved-live-items-phase-plan.md`
- `v4vmm`: `docs/adr/0059-broadcast-control-surface.md`
- `musicindex-live-publisher`: `docs/architecture/broadcast-chain-boundaries.md`
