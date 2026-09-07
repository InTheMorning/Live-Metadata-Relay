# Documentation

## Order Of Work

The packets here belong to a system that spans three repositories. The
cross-repository order lives in `v4vmm`:
`docs/plans/broadcast-chain-delivery-order.md`.

This repository blocks nothing and is blocked by nothing. Its urgency is a
product question: reserved live items matter as soon as a show repeats or a
station runs continuously.

## Architecture

- [Interoperability](interoperability.md) — what consumers depend on, the limits
  they work around, and the requested work

## ADRs

- [ADR 0001: Reserved live items](adr/0001-reserved-live-items.md) — a durable
  event class for stations and repeating shows

## Plans

- [Reserved live items phase plan](plans/adr-0001-reserved-live-items-phase-plan.md)

## Tasks

Packets for the reserved live items plan. Strictly sequential.

- [001 — Event store boundary](tasks/reserved-live-items-task-001-event-store-boundary.md)
- [002 — Reserved class and admin credential](tasks/reserved-live-items-task-002-reserved-class-and-admin-credential.md)
- [003 — Restore on startup and TTL exemption](tasks/reserved-live-items-task-003-restore-and-ttl-exemption.md)
- [004 — List and delete reserved items](tasks/reserved-live-items-task-004-list-and-delete.md)
- [005 — Guards, runbook, and review](tasks/reserved-live-items-task-005-guards-and-review.md)

## Research

- [Broadcaster identity options](research/broadcaster-identity-options.md) —
  seven viable credential models with evidence from the cloned prior art, and
  the availability defect that makes a decision necessary
- [Curiohoster liveValue Socket.IO examples](research/curiohoster-livevalue-socketio-examples.md)
