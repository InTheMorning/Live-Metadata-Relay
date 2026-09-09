# Reserved Live Items Task 005: Guards, Runbook, And Review

Status: Ready - 2026-09-09. Gate task. Do last.

Every criterion in this packet is mechanical. This service has no user
interface, so it has no visual criteria and needs no operator check.

## Goal

Close ADR 0001. Test every invariant, write the operator runbook, write the
review, and reconcile the statuses.

## Files To Inspect

- `docs/adr/0001-reserved-live-items.md`
- `docs/plans/adr-0001-reserved-live-items-phase-plan.md`
- `docs/interoperability.md`
- All four earlier task packets
- `tests/api.rs`
- `README.md`

## Files Likely To Change

- `tests/api.rs`
- `docs/runbooks/reserved-live-items.md` (new)
- `docs/reviews/adr-0001-implementation-review.md` (new)
- `docs/adr/0001-reserved-live-items.md` (status only)
- `docs/plans/adr-0001-reserved-live-items-phase-plan.md` (status only)
- `docs/README.md`
- `docs/interoperability.md`

## Do Not Touch

- Any behavior. This task adds tests, documents, and status lines. If a test
  fails, open a fix task.

## Constraints

- Every invariant in ADR 0001 needs a test or a recorded reason for its
  absence.
- `Implemented` needs a named artifact. The review document is that artifact.
- A status may not claim `Implemented` while a gate is open.
- The runbook is for an operator under pressure. Short sentences, numbered
  steps, one instruction for each step.

## Implementation Steps

1. Write one test for each ADR 0001 invariant:
   - ephemeral behavior is unchanged
   - no payload, snapshot, or replay buffer reaches disk
   - the stored token value is a hash and never the token
   - a durable write needs the admin credential
   - the reaper never removes a reserved item
   - `404` still means the event does not exist, for both classes
   - token comparison is constant time for both tokens
2. Add a test that reads the state file after a publish and asserts it holds no
   payload field. This is the strongest guard for the central safety rule.
3. Write `docs/runbooks/reserved-live-items.md`:
   - reserve an item and store the broadcaster token
   - back up the state file, and what a lost file costs
   - rotate the admin token
   - what a restart looks like: the item lives and serves `{}` until the next
     publish
   - delete a reserved item and tell listeners
   - recover when the state file is corrupt
4. Write `docs/reviews/adr-0001-implementation-review.md` with the reviewed
   artifacts, pass or fail for each invariant, missing tests, drift, and a merge
   recommendation.
5. Answer the three open questions in the plan or move them to a follow-up
   section.
6. Set the ADR and the plan to `Implemented` with the review as the named
   artifact, or record the open gate on the second status line.
7. Update `docs/README.md` with the runbook and the review.
8. Confirm `docs/interoperability.md` matches the shipped behavior, and confirm
   the two neighbor repositories describe the same limits.

## Reconcile The Other Repositories

This feature changes what a `v4vmm` operator can do, so closing it is not a
`splitkit` matter alone.

Before this packet is complete:

- Confirm that the reserve response still holds the four field names that
  `v4vmm` parses into `LiveItemCreateResponse`. Task 002 pins them.
- Record in `v4vmm` at `docs/plans/broadcast-chain-delivery-order.md` that the
  reserved item class exists, so an operator stops losing an event to the
  24-hour idle rule.
- Note in that plan that `v4vmm` has no packet yet for reserving an item from
  the app. This packet does not write one, and it must not leave the gap
  unrecorded.

## Acceptance Criteria

- The reserve response field names match what `v4vmm` parses.
- The `v4vmm` delivery order records the new class and the missing packet.

- Every ADR 0001 invariant has a test or a recorded reason.
- A test proves the state file holds no payload.
- The runbook covers backup, rotation, restart, delete, and corruption
  recovery.
- The review names each artifact it checked.
- The statuses match the evidence.
- `docs/interoperability.md` and the neighbor documents agree.

## Test Commands

- `cargo fmt -- --check`
- `cargo check --quiet`
- `cargo test --quiet`
- `cargo clippy --quiet -- -D warnings`
- `python3 /home/citizen/.claude/plugins/marketplaces/local/plugins/ste100/scripts/ste_lint.py docs/runbooks/reserved-live-items.md docs/reviews/adr-0001-implementation-review.md`

## Expected Final Report Format

1. Files changed
2. Tests run
3. Guards added, one line for each invariant
4. Open gates, if any
5. Merge recommendation

## Escalation Triggers

- An invariant cannot be tested. Record the reason and the manual check that
  replaces it.
- A test fails against shipped code. Open a fix task. Do not change behavior in
  this task.
- A neighbor document contradicts the shipped behavior. Fix the neighbor in its
  own repository and say so in the report.

## Prompt for lower-context coding model

You are implementing one bounded task from a larger plan.

Implement only this task. Do not redesign the architecture. Add no behavior.

Read:
- `docs/adr/0001-reserved-live-items.md`
- `docs/plans/adr-0001-reserved-live-items-phase-plan.md`
- `docs/interoperability.md`, `tests/api.rs`

Goal:
- One test for each invariant, a runbook, a review, and correct status lines.

Constraints:
- Tests, documents, and status lines only.
- `Implemented` needs the review as its named artifact.
- An open gate keeps the status at `Accepted`.

Do not touch:
- any behavior. If a test fails, report it and open a fix task.

Acceptance criteria:
- Every invariant tested or explained.
- A test proves the state file holds no payload.
- Runbook covers backup, rotation, restart, delete, and corruption.

Test commands:
- `cargo fmt -- --check`
- `cargo test --quiet`
- `cargo clippy --quiet -- -D warnings`

At the end, report:
1. files changed
2. tests run
3. guards added
4. open gates
5. merge recommendation
