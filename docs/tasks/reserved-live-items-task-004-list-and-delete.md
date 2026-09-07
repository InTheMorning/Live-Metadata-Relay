# Reserved Live Items Task 004: List And Delete Reserved Items

## Goal

Add two admin routes so an operator can see the reserved items and remove one.
Ephemeral items stay invisible.

## Files To Inspect

- `docs/adr/0001-reserved-live-items.md`
- `src/store.rs`
- `src/lib.rs` (the router and the admin credential check)
- `tests/api.rs`
- `README.md`
- `v4vmm`: `docs/adr/0059-broadcast-control-surface.md`

## Files Likely To Change

- `src/lib.rs`
- `src/store.rs`
- `tests/api.rs`
- `README.md`
- `docs/interoperability.md`

## Do Not Touch

- The ephemeral create route
- The publish path and the wire format
- The reaper

## Constraints

- Both routes need the admin credential. Reuse the check from task 002.
- **Neither route reports an ephemeral item.** Ephemeral items stay invisible
  and undeletable, as they are today. A client that needs a list of its own
  ephemeral items keeps its own registry.
- The list never holds a token or a token hash. It holds the identifier, the
  label, the created time, and the last activity time.
- Delete removes the record from the file and the live state from memory. It
  disconnects current subscribers.
- Delete is permanent and cannot be undone. A later read of that identifier
  answers `404`.
- With no admin token configured, both routes answer `404`.

## Implementation Steps

1. Add `GET /v1/liveitems/reserved` that returns the reserved records.
2. Add `DELETE /v1/liveitems/reserved/{event_id}`.
3. Return `404` when the identifier is not a reserved item, including the case
   where it names an ephemeral item. Do not confirm that an ephemeral item
   exists.
4. On delete, write the file first, then remove the live state, so a crash
   between the two steps cannot leave a record on disk without a way to remove
   it.
5. Close the broadcast channel for the deleted item so subscribers stop.
6. Add tests:
   - list with a correct credential returns only reserved items
   - list holds no token and no hash
   - delete removes the item and a later read answers `404`
   - delete of an ephemeral identifier answers `404` and removes nothing
   - both routes answer `401`, `403`, and `404` for the credential cases
   - a subscriber to a deleted item stops
7. Document both routes, the status codes, and the permanence of delete in
   `README.md`.
8. Note in `docs/interoperability.md` that a client control surface can now list
   and delete reserved items, and that ephemeral items still need a client-side
   registry.

## Acceptance Criteria

- The list holds reserved items only, and no secret.
- Delete is permanent and disconnects subscribers.
- An ephemeral identifier is indistinguishable from an absent one on both
  routes.
- Both routes answer `404` with no admin token configured.
- `README.md` documents both routes.

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

- Closing the broadcast channel needs a change to the subscriber path that
  affects ephemeral items.
- An operator needs a delete that keeps the identifier reserved but clears the
  payload. That is a different route and a different decision.

## Prompt for lower-context coding model

You are implementing one bounded task from a larger plan.

Implement only this task. Do not redesign the architecture.

Read:
- `docs/adr/0001-reserved-live-items.md`
- `src/lib.rs`, `src/store.rs`, `tests/api.rs`

Goal:
- Add admin list and delete routes for reserved items.

Constraints:
- Admin credential on both routes. No token or hash in the list.
- Ephemeral items are never listed and never deleted, and answer `404`.
- Write the file before removing live state.
- No admin token configured means both routes answer `404`.

Do not touch:
- the ephemeral create route, the publish path, the wire format, the reaper

Acceptance criteria:
- List holds reserved items only and no secret.
- Delete is permanent, disconnects subscribers, and a later read answers `404`.

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
