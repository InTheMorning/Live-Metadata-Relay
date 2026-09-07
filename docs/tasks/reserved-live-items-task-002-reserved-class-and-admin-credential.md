# Reserved Live Items Task 002: Reserved Class And Admin Credential

## Goal

Add the reserved item class, an admin credential in configuration, and a route
that reserves a durable live item. Write identity to a file.

## Files To Inspect

- `docs/adr/0001-reserved-live-items.md`
- `src/store.rs`
- `src/lib.rs` (`AppConfig`, `create_event`, `hash_token`, the token check)
- `tests/api.rs`
- `README.md`

## Files Likely To Change

- `src/store.rs`
- `src/lib.rs`
- `tests/api.rs`
- `README.md`
- `docs/interoperability.md`

## Do Not Touch

- The behavior of `POST /v1/liveitems`
- The publish path and the wire format
- The reaper. Task 003 changes it

## Constraints

- **`POST /v1/liveitems` does not change.** A public route must not write to
  disk.
- The reserve route needs the admin credential. Compare it in constant time
  with `subtle`, in the shape of the broadcaster token check. Never compare
  with `==`.
- With no admin token configured, the reserve route answers `404`. The feature
  is off by default.
- **No payload reaches disk.** The file holds identity and token hash only.
- SQLite owns its own durability. Do not write the database through a
  temporary file and a rename. Use a transaction for each change.
- Never log the admin token and never log a broadcaster token.
- A reserved item returns its broadcaster token once, exactly as an ephemeral
  item does.

## Implementation Steps

1. Add `Reserved` to the class enum in `src/store.rs`, and add a `label` field
   to `StoredEvent`.
2. Add `SqliteEventStore` that keeps the reserved records in a SQLite file and
   holds ephemeral records in memory. The schema holds `event_id`,
   `token_hash`, `label`, `created_at`, and a class column. Add a schema
   version table so the identity work that follows can migrate it.
3. Add configuration values: an admin token, a state file path, and a maximum
   reserved item count. Read all three from the environment with the existing
   helper. The admin token and the state path are optional. The state path
   default is the systemd `StateDirectory`, which is
   `/var/lib/musicindex-live-relay/` for the packaged unit.
4. Add `POST /v1/liveitems/reserved`:
   - require the admin credential in an `Authorization` header
   - accept an optional `label` in the body
   - make an identifier and a token, store the hash, write the file
   - return the same response shape as the ephemeral create, plus the label
5. Return `401` for a missing credential, `403` for a wrong credential, and
   `404` when no admin token is configured.
6. Add tests: reserve with a correct credential, a wrong credential, a missing
   credential, no configured admin token, a duplicate label, and a publish to a
   reserved item with its broadcaster token.
7. Add a test that the state file holds no payload field.
8. Document the route, the two configuration values, and the status codes in
   `README.md`.
9. Add the new death-mode information to `docs/interoperability.md`: a reserved
   item survives a restart, and an ephemeral item does not.

## Acceptance Criteria

- The ephemeral create route behaves exactly as before.
- The reserve route is unavailable with no admin token configured.
- The credential check is constant time.
- The state file holds identity and hash only.
- `README.md` documents the route and the status codes.

## Test Commands

- `cargo fmt -- --check`
- `cargo check --quiet`
- `cargo test --quiet`
- `cargo clippy --quiet -- -D warnings`

## Expected Final Report Format

1. Files changed
2. Tests run
3. Behavior changed
4. Schema created
5. Deviations from task
6. Unresolved concerns

## Escalation Triggers

- SQLite needs a crate choice that the plan did not name. Report the crate and
  why.
- The response shape cannot carry a label without a change that affects an
  existing client.

## Prompt for lower-context coding model

You are implementing one bounded task from a larger plan.

Implement only this task. Do not redesign the architecture.

Read:
- `docs/adr/0001-reserved-live-items.md`
- `src/store.rs`, `src/lib.rs`, `tests/api.rs`

Goal:
- Add the reserved class, an admin credential, and `POST /v1/liveitems/reserved`
  with a durable identity file.

Constraints:
- Do not change `POST /v1/liveitems`. A public route must not write to disk.
- Constant-time credential compare. Never log a token.
- No payload in the database. Identity and token hash only.
- SQLite with a schema version table. One transaction for each change.
- No admin token configured means the route answers `404`.

Do not touch:
- the publish path, the wire format, the reaper

Acceptance criteria:
- Ephemeral behavior unchanged, credential cases tested, file holds no payload.
- `README.md` documents the route and the status codes.

Test commands:
- `cargo fmt -- --check`
- `cargo test --quiet`
- `cargo clippy --quiet -- -D warnings`

At the end, report:
1. files changed
2. tests run
3. behavior changed
4. schema created
5. deviations from task
6. unresolved concerns
