# ADR 0001 Reserved Live Items Phase Plan

## Status

Accepted - 2026-09-06.

## Goal

Add a durable live item class so a repeating show and a permanent station keep
one identifier and one broadcaster token across a restart.

## Non-Goals

- No change to ephemeral item behavior.
- No payload, snapshot, or replay buffer on disk.
- No user account system. One configured admin credential only.
- No external state store and no multi-instance state sharing.
- No token rotation route. That is follow-up work.
- No change to the wire format that listener apps read.

## Current State

- `RelayState` holds `events: RwLock<HashMap<String, Arc<LiveEventState>>>`.
  Nothing writes to disk.
- `LiveEventState` holds `token_hash`, `last_activity`, `publish_window`, an
  inner lock with `seq`, `latest`, and `replay`, and a broadcast sender.
- `create_event` rate limits, checks `max_active_events`, makes an identifier,
  makes a token, stores the hash, and inserts the item.
- The reaper removes every item whose `last_activity` is older than the TTL.
  The default TTL is 24 hours.
- `AppConfig::from_env` reads bind, max active events, max SSE connections,
  TTL, publish rate, and create rate.

## Target State

- An `EventStore` boundary with an in-memory implementation and a file-backed
  implementation.
- A `reserved` item class with durable identity, an admin credential for
  creation, list, and delete, and an exemption from the reaper.
- Ephemeral items behave exactly as they do today.

## Affected Modules

- `src/lib.rs`
- `src/main.rs`
- `tests/api.rs`
- `README.md`
- `docs/interoperability.md`

New modules, confirmed in each task:

- `src/store.rs` for the event store boundary and the file backend.

## Proposed Sequence

Do one phase in each session. Make sure the phase is correct, then start a new
session.

1. **Event store boundary.**
   Add an `EventStore` trait and an in-memory implementation. Route every
   current read and write through it. No behavior change and no new route.

2. **Reserved class and admin credential.**
   Add the item class, the admin token in configuration, and the reserved
   create route. Write identity to a file. No restore path yet.

3. **Restore on startup and TTL exemption.**
   Load reserved identity at startup. Serve `{}` until the first publish. Skip
   reserved items in the reaper.

4. **List and delete reserved items.**
   Add two admin routes. Ephemeral items stay invisible.

5. **Guards, runbook, and review.**
   Tests for every invariant, a runbook for reservation and backup, a review
   document, and the status reconciliation.

## Risks

| Risk | Mitigation |
|---|---|
| A restored snapshot routes boosts to a track that stopped | Never write a payload to disk. Add a test that a restart serves `{}` |
| The admin token leaks and a stranger reserves items | Constant-time compare, no token in a log, and a documented rotation step |
| The state file is lost and every reserved item dies | A runbook backup step, and a startup log line that names the file |
| A partial write corrupts the state file | Write a temporary file and rename. Reject a corrupt file at startup with a clear error |
| The two classes drift and the reaper removes a reserved item | One test asserts a reserved item survives a TTL that removes an ephemeral one |
| The store boundary slows the publish path | Keep live state in memory. The store holds identity only |

## Test Strategy

- Store unit tests for both backends: put, get, list, delete, and a corrupt
  file.
- A restart test that builds a new state from the same file and asserts the
  token still validates.
- A restart test that asserts the metadata route answers `{}` and not the old
  snapshot.
- A reaper test with a short TTL that removes an ephemeral item and keeps a
  reserved item.
- Admin route tests for a missing credential, a wrong credential, and a correct
  credential.
- Every existing test in `tests/api.rs` passes unchanged after phase 1.

## Rollback Strategy

Phase 1 is a refactor with no behavior change and reverts cleanly.

Phases 2 through 4 are additive. With no admin token configured, the reserved
routes are unavailable and the service behaves as it does today. That is the
rollback: remove the admin token from the environment.

## Resolved Questions

Answered on 2026-09-06:

- The storage format is SQLite.
- The state file lives in the systemd `StateDirectory`, which is
  `/var/lib/musicindex-live-relay/` for the packaged unit.
- A reserved item count limit is configurable, separate from
  `max_active_events`.

## Open Questions

- How does a broadcaster get a credential on a public service? Deferred on
  2026-09-06. The options and the evidence are in
  `docs/research/broadcaster-identity-options.md`. A future ADR 0002 decides
  it.
- Does the admin token from this ADR become the operator credential of that
  model, or does it stay separate?

## References

- `docs/adr/0001-reserved-live-items.md`
- `docs/interoperability.md`
- `v4vmm`: `docs/adr/0059-broadcast-control-surface.md`
