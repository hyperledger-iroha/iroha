---
title: Sora Network Indexer & Delegated Routing Plan
summary: SFM-1 local HTTP Routing V1 service, authority model, security bounds, and deployment work.
---

# Sora Network Indexer Plan

> **Status (July 2026):** The local SFM-1 delegated-routing service is
> implemented in Torii. `GET /routing/v1/providers/{cid}` and
> `GET /routing/v1/peers/{peer_id}` expose the vendor-neutral HTTP Routing V1
> content and peer lookup shapes. The earlier provider-discovery endpoints
> remain available for advert ingestion and operator readback. Regional
> deployment, live traffic/SLA evidence, and generated-client publication are
> release operations still to be completed; they are not inferred from local
> tests.

## Authority model

The service does not maintain a second content-ownership database. Every
lookup derives a bounded snapshot from three existing authorities:

1. The committed pin registry must contain the requested content root in an
   `Approved` `PinManifestRecord`, effective at the current committed epoch.
2. A canonical capacity `ReplicationOrderV1` bound to that manifest must have
   a `Completed` ledger lifecycle. `Pending`, `Expired`, future-dated,
   record/payload-mismatched, retired-manifest, and non-canonical orders do not
   publish routes.
3. Each assigned provider must have a current `ProviderAdvertV1` in Torii's
   admission-bound advert cache. Normal ingestion already verifies the advert
   signature and council admission envelope and persists replay high-water
   marks. The routing handler prunes expired or no-longer-admitted adverts
   before projection.

This join prevents a valid advert from claiming content it was never assigned
and prevents a historical assignment from reviving a retired pin. The routing
view is reconstructed from committed state after restart, so it needs no
independent checkpoint. Durable anti-replay state remains in the existing
provider-advert cache, whose loader rejects corrupt, oversized, non-canonical,
unadmitted, and symlink-backed checkpoints.

## HTTP Routing V1 contract

The implementation follows the current
[Delegated Routing V1 HTTP API](https://specs.ipfs.tech/routing/http-routing-v1/)
for the content and peer GET operations:

- `GET /routing/v1/providers/{cid}` accepts a canonical SoraFS CIDv1 encoded
  as lowercase base32, base36, or base58btc multibase. Other layouts,
  including identity-multihash content CIDs, return `422`.
- `GET /routing/v1/peers/{peer_id}` accepts canonical libp2p peer IDs in legacy
  Base58btc or CIDv1 `libp2p-key` base32/base36 form. Responses normalize IDs
  to CIDv1 base32.
- `filter-addrs` supports case-insensitive positive OR filters, `!` negative
  AND filters, and the special `unknown` value. Address filtering changes only
  `Addrs`; a record with no surviving address is omitted.
- `filter-protocols` performs a case-insensitive record match, including the
  special `unknown` value, while preserving the complete `Protocols` array in
  matching records.
- Duplicate parameters, duplicate case-folded terms, contradictory filters,
  unknown parameters, invalid characters, and over-limit queries return
  `422`. Unsupported response media types return `406`.
- Normal responses use `application/json` and the standard `Providers` or
  `Peers` wrapper. Explicit `Accept: application/x-ndjson` returns one peer
  record per line. JSON is capped at 100 records and NDJSON at 1,024 records.
- Every success is deterministically ordered by normalized peer ID, with
  sorted/deduplicated addresses and protocols. Responses include
  `Last-Modified`, `Vary: Accept`, public CORS headers, and positive/negative
  cache TTLs bounded by advert expiry. Errors are `no-store` and never reflect
  request identifiers or filter payloads into logs.

Peer IDs are derived from the admitted advert's Ed25519 public key using the
canonical libp2p public-key protobuf and identity multihash. Reuse of one peer
key by different governed provider IDs is treated as equivocation and fails
closed rather than merging ownership.

## Resource and corruption bounds

The first-release handler rejects authority snapshots above 65,536 manifests
or orders, more than 262,144 aggregate provider assignment references,
replication-order payloads above 1 MiB, adverts with more than 32 endpoints,
path identifiers above 256 bytes, raw queries above 2 KiB, and filter/Accept
fan-out above their fixed limits. Replication payloads are decoded under
Norito allocation/depth limits, structurally validated, re-encoded, and
byte-compared before their assignments enter the index.

The implementation deliberately skips unsafe endpoint strings rather than
turning them into connectable multiaddrs. A still-authorized peer with no safe
address is represented with an empty `Addrs` array, as allowed by HTTP Routing
V1, and is returned only when address filtering permits `unknown`.

## Validation

Focused tests cover:

- malformed, oversized, non-canonical, and identity-multihash content IDs;
- all supported peer-ID encodings, overlong varints, malformed hashes, and
  oversized identities;
- duplicate/encoded-duplicate parameters, filter case bypasses, contradictory
  and oversized filters, and host-value/protocol confusion;
- positive, negative, mixed, and `unknown` address/protocol filtering;
- pending/expired orders, pending/retired/future pins, future completion,
  corrupt/oversized/non-canonical order payloads, exact replay, and identity
  equivocation;
- expired/missing/unassigned adverts, unsafe endpoints, deterministic ordering,
  result caps, JSON/NDJSON negotiation, cache headers, and payload-safe errors.

## Remaining deployment work

Local implementation does not prove production deployment. SFM-1 remains open
for the following operational evidence:

1. Publish regenerated OpenAPI and SDK artifacts containing both routes and
   pass their cross-language fixture guards.
2. Deploy at least two independently operated regional Torii gateways, replay
   the same committed pin/order state, and prove byte-identical sorted lookup
   results under advert rotation and revocation.
3. Capture signed load, latency, cache, error-rate, failover, and stale-advert
   evidence under the production readiness envelope. Exercise malformed-query
   floods and provider-key equivocation without leaking request payloads.
4. Complete external review of the authority join, peer-ID derivation, cache
   policy, CORS exposure, and regional incident/runbook procedures before the
   production gate can become green.
