# Broadcaster Identity Options

Research note. Recorded on 2026-09-06. No decision. This document exists so a
later deep-dive starts with the evidence instead of the argument.

## The Problem

`POST /v1/liveitems` is public. Every limit in the service is global, and the
service holds no client identity. There are zero uses of `ConnectInfo`,
`remote_addr`, or a forwarded-address header in `src/lib.rs`.

The result:

- `max_creates_per_sec` is 50 for all callers together.
- `max_active_events` is 10,000 for all callers together.

One caller can therefore make 10,000 events in about 210 seconds. Every other
broadcaster then receives `503 max_active_events_reached` until the idle TTL
removes the events 24 hours later. The service records nothing about who did
it.

This is an availability defect for a public deployment. It needs a decision
before the relay carries other people's shows.

## What The Operator Already Decided

Recorded on 2026-09-06. These constrain the options below:

- The relay is an **open public service**. Anyone can broadcast.
- **Every create needs a credential.** Anonymous create ends.
- **Limits are counted for each credential**, not globally.
- **A caller over its quota is rejected alone.** Other broadcasters are not
  affected.
- A credential carries an **opaque key plus quota values**. No personal data.
- New credentials start **conservative and are raised on request**.
- An operator can **revoke a credential and purge its events** in one action.

The open question is only this: **how does a broadcaster get a credential?**

## Prior Art

Read on 2026-09-06 from the cloned repositories.

### CurioHoster delegates identity to Alby

`curiohoster/sk/generateguid.js` is the create-event handler for The Split Kit.
It reads a JWT from a cookie, verifies it with the Alby secret, calls
`https://api.getalby.com/user/value4value`, and takes the returned
`lightning_address` as the user. It stores `{ guid, lightning_address,
eventName }`.

There is no account system for this path. Identity is delegated.

### The Split Box keys users by address

`thesplitbox/server/stores/inMemoryStore.js` stores settings with
`saveSettings(address, settings)` and reads them with `fetchSettings(address)`.
The Lightning address is the user key. The same model as CurioHoster.

### Neither has rate limiting

A search for a rate limit, a throttle, or a limiter in both repositories returns
nothing.

### The subscriber side is a capability URI

`thesplitkit/src/lib/Share/Share.svelte` builds
`<podcast:liveValue uri="..." protocol="socket.io"/>` for the operator to paste
into an RSS feed. An unguessable URI is the whole access control for listeners.
That model works for readers and does not answer the create question.

### Nostr is not used for authentication

`thesplitbox/nostr.js` sends zap receipts, kind 9734. It is a payment artifact,
not a credential.

## One Fact That Changes The Options

An earlier objection said a browser-based flow cannot work, because
`musicindex-live-publisher` is a headless service.

That objection is wrong under `v4vmm` ADR 0059. The publisher never creates an
event. `v4vmm` creates it, and `v4vmm` is a desktop application that can open a
browser. It already depends on the `open` crate. The publisher then sends
payloads with the per-event broadcaster token.

So a browser flow is available to the only component that needs one.

## The Options

### Option A: Alby OAuth, Lightning address as identity

`v4vmm` runs the OAuth flow. The relay verifies the token with Alby, reads the
Lightning address, and uses it as the broadcaster key.

- For: exact prior art, proven in this ecosystem. No account system to build,
  secure, or reset. Identity ties to a real payment account, which makes abuse
  costly. The quota key is a value the ecosystem already shares.
- Against: an Alby account becomes mandatory to broadcast. That excludes an
  operator who runs a self-custodial node and does not use a custodial service.
  The relay gains a hard runtime dependency on a third party. If Alby is down,
  no new event can be created. Alby can also change or withdraw the endpoint.
- Build: OAuth in `v4vmm`, token verification and an Alby call in the relay.

### Option B: Self-service opaque API key

`POST /v1/broadcasters` returns an opaque key. Issuance is limited for each
source address. Quotas attach to the key.

- For: no third-party dependency and no exclusion. Works for a headless caller
  and for a desktop caller. The key is the quota subject with no translation.
  The relay stays self-contained.
- Against: no prior art either way. The issuance route is itself an abuse
  surface, and the per-address limit that protects it is weak behind a proxy
  unless a forwarded-address header is read and trusted, which the service does
  not do today. Nothing ties a key to a real person, so a purge is the only
  remedy.
- Build: an issuance route, a key store, quota counters, and an admin surface.

### Option C: Both, Alby preferred

Alby where the broadcaster has an account, an opaque key otherwise.

- For: widest reach. An Alby identity carries a higher default quota, because
  it costs more to create.
- Against: two authentication paths and two identity shapes in every quota
  calculation, every test, and every admin view. The most code and the most
  ways to be wrong.

### Option D: Nostr key signature on create

The caller signs the create request with a nostr key. The public key is the
identity.

- For: self-issued, so no issuance route and no signup. No secret is stored by
  the relay. The ecosystem is adjacent, and `v4vmm` already reads nostr handles
  from feeds as source facts.
- Against: this is not the prior art. Bell uses nostr for zap receipts only. A
  key pair is free to make, so a public key alone gives no abuse resistance
  without a further reputation signal. `v4vmm` has no operator key today, only
  handles that belong to other people.

### Option E: Lightning address with a proof

The caller states a Lightning address and proves control, for example with an
LNURL-auth style challenge or a signed message.

- For: the same identity shape as CurioHoster and The Split Box, with no
  dependency on one custodial provider. Self-custodial operators are included.
- Against: the most protocol work of any option. The proof mechanism must be
  chosen, implemented, and tested on both sides, and wallet support varies.

### Option F: Manual issuance by the operator

The relay operator makes keys and hands them out.

- For: no abuse surface and almost no code. Correct for a small known set of
  broadcasters.
- Against: contradicts the open public service decision. It is a bottleneck at
  any scale and it puts the operator in the path of every new broadcaster.

### Option G: Proof of feed ownership

A broadcaster proves control of a podcast feed, then receives a key.

- For: the strongest tie to an identity the ecosystem already recognizes, and
  it matches what a live event is for.
- Against: by far the most work. It needs a challenge in the feed, a fetch, and
  a retry policy. It also excludes a broadcaster who has no feed yet, which is
  a normal state before a first show.

## A Note On Doing Nothing

The current state is not a neutral default. A public create route with global
limits means one caller can stop the service for everyone, with no record of
who. That is acceptable only while the deployment is effectively private. The
operator decided that it will not stay private.

## Open Questions For The Deep-Dive

- Does a mandatory Alby account conflict with the values of the audience this
  serves?
- Is a forwarded-address header trustworthy in the planned deployment? Option B
  depends on it.
- What abuse has actually happened on comparable relays? The prior art has no
  rate limiting at all, which suggests either low abuse or unreported abuse.
- Should the reserved item class of ADR 0001 need a higher trust level than an
  ephemeral one, given it consumes durable storage?
- Does the admin token of ADR 0001 become the operator credential of the chosen
  model, or stay separate?

## Route

A future ADR 0002 in this repository. ADR 0001 does not wait for it, because
the store boundary and the no-payload rule do not depend on identity.

## References

- `docs/interoperability.md`
- `docs/adr/0001-reserved-live-items.md`
- `curiohoster/sk/generateguid.js`
- `thesplitbox/server/stores/inMemoryStore.js`
- `thesplitkit/src/lib/Share/Share.svelte`
- `v4vmm`: `docs/adr/0059-broadcast-control-surface.md`
