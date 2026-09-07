# musicindex-live-relay Agent Guidelines

This repository holds `musicindex-live-relay`, a small Rust service that relays
live metadata to listener apps over Socket.IO, server-sent events, and HTTP.

Read `docs/interoperability.md` before a change to a route, a body form, or the
state model. Three other repositories depend on those choices.

## Build / Lint / Test Commands

```bash
cargo build --release
cargo build
cargo test                       # All tests
cargo test --test api            # The HTTP integration tests
cargo fmt -- --check
cargo fmt
cargo clippy -- -D warnings
```

## Conventions

- Rust edition 2024. `cargo fmt` defaults. No `rustfmt.toml`.
- Types and enums use PascalCase. Functions, variables, and modules use
  snake_case. Constants use SCREAMING_SNAKE_CASE.
- Group imports by std, external, then `crate::`.
- Use `tracing` for logging, not `println!`.
- Configuration comes from environment variables through `AppConfig::from_env`.
  Add a default constant for each new value.
- Document every public type and every route handler.

## Foundational Mandates

### 1. The Wire Format Is A Public Contract

- Listener apps read `remoteValue`. The payload shape they receive is a public
  contract. Do not change a field name or a nesting level without an ADR.
- **The relay separates the wrapped body from the direct body by exact key
  match.** A body with exactly the keys `event_id` and `metadata` is wrapped.
  Any other object is a direct live value payload. A change to that rule breaks
  `musicindex-live-publisher`, which has a test that asserts against it.
- A direct payload passes through without interpretation. Do not add a field to
  it and do not reorder it.

### 2. ADR-First

- Do not implement an architectural change before an ADR in `docs/adr/` records
  it.
- A status is one of `Proposed`, `Accepted`, `Implemented`, or
  `Superseded by ADR NNNN`, each with a date.
- `Implemented` needs a named artifact: a review, a checklist with no open
  gates, or a completed task series.
- Amend an ADR in place only when the amendment does not reverse a decision,
  and add a dated sentence that says what changed.

### 3. Documentation Discipline

- Documentation lives under `docs/`, in `adr/`, `plans/`, `tasks/`, `reviews/`,
  `runbooks/`, `architecture/`, and `research/`. Keep the repository root
  small.
- Write documentation prose in ASD-STE100 Simplified Technical English. Short
  sentences, active voice, no semicolons, and one instruction for each
  sentence.
- Update `docs/README.md` when a document is added.
- `README.md` is the API reference for client authors. Keep every route,
  status code, and body example correct. A wrong example here becomes a defect
  in another repository.

### 4. Payment Safety

This service carries payment routing to listener apps.

- **A stale snapshot sends listener boosts to a track that is not playing.**
  Never serve a snapshot that the current broadcaster did not publish. This is
  the reason a restart must not restore a stored payload.
- A publish needs a valid broadcaster token. Compare token hashes in constant
  time with `subtle`. Never compare with `==`.
- Store a SHA-256 hash of a token. Never store the token.
- A `404` for an unknown event is load-bearing. A control surface uses it to
  detect a dead event. Do not change it to a `200` with an empty body.

### 5. Resource Limits

- Every limit is configurable and has a default: active events, SSE
  connections, event TTL, publish rate, and create rate.
- `POST /v1/liveitems` is public. Any new route that consumes durable storage
  must need a credential, or it is an unbounded growth surface.
- The replay buffer holds a fixed number of updates for each event. Keep the
  bound.

### 6. State Model Honesty

- Today the service keeps state in memory only, and an event dies on a process
  restart or after the idle TTL.
- Consumers depend on those two death modes and work around them. A change to
  either one is an interoperability change. Record it in
  `docs/interoperability.md` in the same commit.

### 7. Test Strategy

- `tests/api.rs` holds the HTTP integration tests. Every route has one.
- Every status code named in `README.md` has a test.
- Rate limits, the TTL reaper, and token comparison each have a test.
- Do not depend on wall-clock sleep for a TTL test. Inject the time source or a
  short TTL, as the existing tests do.

## Neighbors

| Repository | Relation |
|---|---|
| `musicindex-live-publisher` | The broadcaster. Sends the direct live value payload. |
| `v4vmm` | Creates live items, keeps the event registry and the tokens, reads snapshots. Sends no payloads. |

This service has no dependency on either one and must keep none.
