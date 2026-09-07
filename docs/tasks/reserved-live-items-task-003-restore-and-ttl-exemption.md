# Reserved Live Items Task 003: Restore On Startup And TTL Exemption

## Goal

Load reserved identity at startup so a stored broadcaster token stays valid.
Skip reserved items in the reaper.

## Files To Inspect

- `docs/adr/0001-reserved-live-items.md`
- `src/store.rs`
- `src/lib.rs` (`RelayState::new`, the reaper, the TTL configuration)
- `src/main.rs`
- `tests/api.rs`

## Files Likely To Change

- `src/store.rs`
- `src/lib.rs`
- `src/main.rs`
- `tests/api.rs`
- `README.md`
- `docs/interoperability.md`

## Do Not Touch

- The wire format
- The ephemeral create route
- The admin credential check from task 002

## Constraints

- **A restored item serves `{}` until the next publish.** Never restore a
  payload, a sequence number that implies a payload, or a replay buffer. A
  restored snapshot would tell listener apps to pay the destinations of a track
  that stopped. This is the central safety rule of the ADR.
- The sequence number restarts at zero for a restored item. Say so in
  `README.md`, because an SSE client that keeps a `Last-Event-ID` sees the
  reset.
- A corrupt database is a startup error with a clear message. Do not start with
  a silent empty set, and do not delete or recreate the database.
- The reaper skips reserved items. It keeps its current behavior for ephemeral
  items.
- Startup logs the state file path and the count of restored items. It logs no
  identifier and no hash.

## Implementation Steps

1. Add `load` to the SQLite store. Read the reserved rows and return them.
   Return a typed error for a corrupt or unreadable database.
2. A missing database file is normal on a first run. Create the schema and
   yield an empty set.
3. In `RelayState::new`, build a live state for each restored record with the
   stored token hash, an empty `latest`, an empty replay buffer, `seq` at zero,
   and `last_activity` at the current time.
4. Change the reaper to retain an item when its class is `Reserved`, and to
   apply the TTL to an ephemeral item as before.
5. Fail startup on a corrupt file, with the path in the message.
6. Add tests:
   - a token that validates against a store built from the same file
   - a metadata read after a restore answers `{}` and not the old payload
   - a `remoteValue` read after a restore answers `{}`
   - a short TTL removes an ephemeral item and keeps a reserved item
   - a corrupt file fails startup and leaves the file unchanged
7. Document the restore behavior, the `{}` answer, and the sequence reset in
   `README.md`.
8. Update `docs/interoperability.md`: a reserved item survives a restart and the
   idle TTL, and it serves `{}` until the next publish.

## Acceptance Criteria

- A stored broadcaster token still validates after a restart.
- A restored item serves `{}`, never the previous payload.
- The reaper keeps reserved items and still removes ephemeral ones.
- A corrupt file fails startup with the path in the message.
- `README.md` and `docs/interoperability.md` describe the new behavior.

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

- A client depends on a sequence number that does not reset. Report it before
  you add persistence for the sequence.
- The reaper cannot read the class without a lock change that affects the
  publish path.

## Prompt for lower-context coding model

You are implementing one bounded task from a larger plan.

Implement only this task. Do not redesign the architecture.

Read:
- `docs/adr/0001-reserved-live-items.md`
- `src/store.rs`, `src/lib.rs`, `src/main.rs`, `tests/api.rs`

Goal:
- Restore reserved identity at startup and exempt reserved items from the
  reaper.

Constraints:
- Never restore a payload. A restored item serves `{}` until the next publish.
- The sequence number restarts at zero. Document it.
- A corrupt file fails startup and is not overwritten.
- Log the file path and the count, never an identifier or a hash.

Do not touch:
- the wire format, the ephemeral create route, the credential check

Acceptance criteria:
- Token validates after a restart, metadata answers `{}`, reaper keeps reserved
  items, corrupt file fails startup.

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
