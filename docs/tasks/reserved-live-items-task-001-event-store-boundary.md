# Reserved Live Items Task 001: Event Store Boundary

Status: Ready - 2026-09-09. Do first. Every later packet writes through the
boundary this one adds.

Every criterion in this packet is mechanical. This service has no user
interface, so it has no visual criteria and needs no operator check.

## Goal

Add an `EventStore` boundary with an in-memory implementation, and route every
current live item read and write through it. No behavior change.

## Files To Inspect

- `docs/adr/0001-reserved-live-items.md`
- `docs/plans/adr-0001-reserved-live-items-phase-plan.md`
- `src/lib.rs` (`RelayState`, `LiveEventState`, `create_event`, the reaper)
- `tests/api.rs`

## Files Likely To Change

- `src/store.rs` (new)
- `src/lib.rs`

## Do Not Touch

- `README.md` route documentation
- The wire format of any response
- `src/main.rs`
- Any test expectation in `tests/api.rs`

## Constraints

- **This task changes no behavior.** Every existing test must pass without an
  edit. If a test needs an edit, stop and report.
- Live state stays in memory. The store holds identity and the token hash. It
  does not hold the latest snapshot, the replay buffer, or the broadcast
  sender.
- Keep the lock discipline. Do not hold a store lock across an await that sends
  to a subscriber.
- No new dependency in this task.

## Implementation Steps

1. Add `src/store.rs` and declare it in `src/lib.rs`.
2. Define `StoredEvent { event_id, token_hash, created_at, class }` where
   `class` is an enum with one variant `Ephemeral` for now.
3. Define an `EventStore` trait with `insert`, `get`, `remove`, `list_ids`, and
   `retain` for the reaper.
4. Add `MemoryEventStore` that holds the current `HashMap`.
5. Change `RelayState` to hold the store behind the same lock strategy it uses
   now.
6. Route `create_event`, the publish path, the metadata read, the
   `remoteValue` read, the SSE path, and the reaper through the store.
7. Keep `LiveEventState` as the live half. The store returns the identity, and
   `RelayState` keeps the live state beside it.
8. Add store unit tests: insert, get, remove, list, and retain.
9. Run the whole existing test file and confirm no expectation changed.

## Acceptance Criteria

- `cargo test` passes with no edit to `tests/api.rs`.
- No route response changes.
- The store holds no snapshot and no replay buffer.
- `MemoryEventStore` has unit tests for all five operations.

## Test Commands

- `cargo fmt -- --check`
- `cargo check --quiet`
- `cargo test --quiet`
- `cargo clippy --quiet -- -D warnings`

## Expected Final Report Format

1. Files changed
2. Tests run
3. Behavior changed
4. Deviations from task
5. Unresolved concerns

## Escalation Triggers

- An existing test needs an edit. That means behavior changed. Stop.
- The trait cannot express the reaper without holding a lock for too long.

## Prompt for lower-context coding model

You are implementing one bounded task from a larger plan.

Implement only this task. Do not redesign the architecture.

Read:
- `docs/adr/0001-reserved-live-items.md`
- `src/lib.rs`, `tests/api.rs`

Goal:
- Add an `EventStore` trait and `MemoryEventStore`, and route current reads and
  writes through it.

Constraints:
- No behavior change. Every existing test passes unedited.
- The store holds identity and token hash only. No snapshot, no replay buffer.
- No new dependency.

Do not touch:
- route documentation, response shapes, `src/main.rs`, test expectations

Acceptance criteria:
- `cargo test` passes with no test edit.
- Store unit tests cover insert, get, remove, list, and retain.

Test commands:
- `cargo fmt -- --check`
- `cargo test --quiet`
- `cargo clippy --quiet -- -D warnings`

At the end, report:
1. files changed
2. tests run
3. behavior changed
4. deviations from task
5. unresolved concerns
