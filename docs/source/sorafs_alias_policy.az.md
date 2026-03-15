---
lang: az
direction: ltr
source: docs/source/sorafs_alias_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 075295bc420dce026e0f02b42db5f39eaf4c30002f246d2a5d11f922b605bb9d
source_last_modified: "2025-12-29T18:16:36.133445+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
Owner: Nexus Core Infra TL / Gateway Team
Related roadmap item: SF-4a — Alias proof caching & TTL policy
-->

# SoraFS Alias Proof Caching & Rotation Policy

This document standardises how SoraFS gateways, SDKs, and intermediate caches
handle `Sora-Name`/`Sora-Proof` envelopes. The goal is to propagate alias
updates deterministically across the network while avoiding thundering herds on
Torii or the Pin Registry.

## Scope & Goals

- Provide a single set of TTLs for positive, stale, and negative alias
  lookups so every component rejects stale bindings at the same time.
- Define proof rotation requirements for governance so Merkle paths never
  exceed the permitted staleness window.
- Document the observable headers and telemetry signals that gateways must emit
  to prove compliance.

## Envelope Recap

Gateways staple two headers on every SoraFS response:

- `Sora-Name`: ASCII alias in `namespace/name` form.
- `Sora-Proof`: Base64-encoded Norito payload `AliasProofBundleV1`.

`AliasProofBundleV1` is defined canonically in
[`sorafs_manifest::AliasProofBundleV1`](../../crates/sorafs_manifest/src/pin_registry.rs).
The bundle pairs the alias binding with the registry Merkle root/height,
generated and expiry timestamps, optional Merkle path entries, and governance
signatures. See `AliasProofBundleV1::validate` for the invariants enforced on
those fields.

Torii emits bundles that include a BLAKE3-derived Merkle root alongside an
Ed25519 signature from each active council signer. The signed digest is derived
as `blake3("sorafs:alias:root:v1" || registry_root || registry_height
|| generated_at || expires_at)`, and the Merkle tree hashes each alias binding
with the `sorafs:alias:leaf:v1` and `sorafs:alias:parent:v1` domain separators.
Consumers **must** recompute the Merkle root and verify every signature before
accepting a bundle.

Caches MUST validate both the Merkle path and the `AliasBindingV1` expiry before
serving a response. `expires_at_unix` may never exceed the on-chain
`AliasBindingV1::expiry_epoch` translated into wall-clock seconds by the Torii
registry facade.

## TTL Budget

| Parameter | Value | Applies to | Rationale |
|-----------|-------|------------|-----------|
| `alias_positive_ttl` | 10 minutes | Gateway in-memory cache, SDK local cache | Matches governance SLAs for alias updates; aligns with the open RFC strawman. |
| `alias_refresh_window` | last 2 minutes of positive TTL | Gateways | Gives headroom to refresh before expiry without double-serving stale proofs. |
| `alias_hard_expiry` | 15 minutes | All caches | Absolute ceiling; prevents divergent behaviour if refresh fails. |
| `alias_negative_ttl` | 60 seconds | Gateway + SDK caches | Avoids hammering Torii for missing aliases while allowing fast recovery after registrations. |
| `alias_revocation_ttl` | 5 minutes | Gateway + SDK caches | Allows deterministic propagation of revocations (`410 Gone`) while keeping rollback responsive. |

All values are maximums. Implementations MAY refresh earlier but MUST NOT exceed
the table without a governance override published in `iroha_config`.

## Configuration

The defaults above ship via the configuration stack as
`[sorafs.alias_cache]` in `iroha_config` (user view) and
`torii.sorafs_alias_cache` in the runtime snapshot. Both surfaces expose the
same fields and propagate directly into `AliasCachePolicy` and
`AliasCacheEnforcement` so gateways and SDKs honour identical guardrails.

| Key | Default | Notes |
|-----|---------|-------|
| `positive_ttl` | 600 s (10 min) | Upper bound for serving cached proofs without a refresh. |
| `refresh_window` | 120 s (2 min) | Window before expiry where gateways must synchronously revalidate. |
| `hard_expiry` | 900 s (15 min) | Absolute ceiling; proofs older than this are rejected. |
| `negative_ttl` | 60 s | Cache duration for `404` misses. |
| `revocation_ttl` | 300 s (5 min) | Cache duration for `410 Gone` revocations. |
| `rotation_max_age` | 21_600 s (6 h) | Governance rotation ceiling; proofs older than this trigger alerts even if TTLs are met. |
| `successor_grace` | 300 s (5 min) | Grace window after a successor manifest is approved; gateways may serve the predecessor proof while draining caches, but MUST block once the window elapses. |
| `governance_grace` | 0 s | Extra allowance after Parliament rotations; kept at zero so governance changes do not drift cache behaviour unless explicitly raised. |

Operators can tighten the policy by lowering these values; raising them requires
a roadmap update and council approval to keep alias propagation deterministic.
`sorafs_manifest::alias_cache::AliasCachePolicy` exposes the entire surface so
gateways and SDKs can apply the same TTLs and grace windows when evaluating
`PinRegistry` successor pointers and governance records.

## Positive Cache Flow

1. Parse `AliasProofBundleV1` and compute `age = now() - generated_at_unix`.
2. If `age < alias_positive_ttl - alias_refresh_window`, serve the response and
   schedule an asynchronous refresh.
3. If `age` enters the refresh window, synchronously revalidate against Torii or
   the Pin Registry before responding. If the refresh succeeds, replace the
   cache entry; otherwise respond with `503 alias_proof_stale` and emit telemetry.
4. If `age >= alias_positive_ttl`, gateways MUST block the response until a
   refreshed proof is fetched. If refresh continues to fail and
   `age >= alias_hard_expiry`, respond with `412 Precondition Failed` and
   include a diagnostic `Sora-Proof-Status: expired` header.

SDKs follow the same thresholds but may fall back to an optimistic
`conditional-fetch` flow: they can request a blinded manifest hash from Torii
while waiting for the new proof, then retry the gateway once fresh proof is
available.

## Successor & Governance Enforcement

- When the registry approves a successor manifest, gateways only serve the
  predecessor proof until `serve_until` (RFC3339), derived from
  `successor_grace`. During that window responses advertise
  `Sora-Proof-Status: refresh-successor` and list `ApprovedSuccessorGrace` in
  the `cache_reasons` array. Once the deadline elapses, the predecessor is
  rejected (`successor-refused`) until a refreshed bundle is supplied.
- If `approved_at` (or the numeric `approved_at_unix`) lies in the future,
  gateways return `ApprovedSuccessorPending` and hold requests until that
  timestamp. SDKs must
  treat this identically to the grace window and pre-fetch the successor bundle.
- Governance freezes, revocations, or rotations surface either an immediate
  `governance-refused` (default `governance_grace = 0`) or a temporary
  `refresh-governance` status when a non-zero grace is configured. Clients
  should gate any writes or notarisation attempts tied to the alias until a
  fresh proof lands.
- JSON representations returned by alias endpoints carry
  `cache_decision.reasons`, `cache_evaluation.serve_until` (RFC3339), and the
  numeric `cache_evaluation.serve_until_unix` so SDKs can
  detect whether they sit inside a grace window and plan refreshes
  deterministically.
- The lineage (`successor_of`, approved successor digests) and governance
  references come directly from the Pin Registry snapshot; clients must fetch
  the same metadata to mirror the gateway's successor/governance decisions.

### Headers & Telemetry

- Gateways include `Cache-Control: max-age=600, stale-while-revalidate=120`.
- `Warning: 199 - "alias proof refresh in-flight"` MUST appear when serving a
  response from the refresh window.
- `Sora-Proof-Status` advertises the cache evaluation: `fresh` / `refresh`
  remain servable, `refresh-successor` / `refresh-governance` denote grace
  windows, `expired` rejects with 503, `hard-expired` rejects with 412, and
  `successor-refused` / `governance-refused` signal hard failures. `*-rotate`
  suffixes surface rotation deadlines so SDKs can pre-emptively refresh.
- Emit `torii_sorafs_alias_cache_refresh_total{result="success|error"}` counters
  with failure reasons (`timeout`, `registry_unavailable`, `proof_invalid`).
- Surface `torii_sorafs_alias_cache_age_seconds` histogram for observability.

## Negative Cache Flow

- `404 Not Found` responses (alias never registered) MAY be cached for up to
  `alias_negative_ttl`. Gateways include `Cache-Control: max-age=60` and SHOULD
  set `Retry-After: 60` to hint clients.
- `410 Gone` responses (alias revoked) MUST be cached for
  `alias_revocation_ttl`, include `Cache-Control: max-age=<alias_revocation_ttl>`,
  and SHOULD emit `Retry-After: <alias_revocation_ttl>` so clients know when to
  revalidate. During this window, gateways continue returning `410` even if
  stale caches attempt to serve the old manifest. Clients SHOULD treat the alias
  as unavailable until the window elapses and a new proof surfaces.
- Negative caches MUST be purged immediately if Torii signals a registry change
  via SSE/WebSocket notifications or when the client successfully retrieves a
  newer proof bundle.

## Successor & Governance Grace Periods

- When a successor manifest is approved, gateways honour the
  `successor_grace` window before refusing the predecessor proof. This prevents
  brief `412` storms while caches reload the new bundle but guarantees that
  stale proofs disappear deterministically after five minutes.
- SDKs replicate the same grace window and SHOULD expose a `successor_grace`
  field in diagnostics so operators can confirm convergence during upgrades.
- Parliament rotations do **not** introduce additional slack. The
  `governance_grace` field remains zero by default, so newly seated signers must
  publish updated proofs within the standard rotation cadence. Raising this
  value requires an explicit governance motion and corresponding documentation
  update.

## Pin Registry Alignment

- The `/v1/sorafs/pin` and `/v1/sorafs/pin/{digest}` endpoints hydrate each
  alias response with `cache_evaluation.successor` (lineage head, approval
  status, anomalies) and `cache_evaluation.governance` (revocation/freeze/rotation flags) sourced
  from the pin registry snapshot.
- Aliases referencing manifests that are absent from the registry immediately
  refuse with `cache_decision: "refuse"` and log the `ManifestMissing` reason so
  operators can remediate the underlying data drift.
- Successor traversal is capped at 64 hops; exceeding the bound emits a
  `LineageDepthExceeded` anomaly and Torii refuses to serve the stale proof to
  avoid unbounded scans or adversarial chains.
- Governance envelopes attached to the manifest metadata (`sorafs_governance_refs`)
  trigger the corresponding reason codes (`GovernanceRevoked`, `GovernanceFrozen`,
  `GovernanceRotated`) and propagate the effective timestamp via
  `cache_evaluation.governance.effective_at` (RFC3339) alongside the numeric
  `cache_evaluation.governance.effective_at_unix` field for diagnostics.

## Proof Rotation Cadence

- Governance signers MUST emit a refreshed proof bundle within two minutes of
  any alias mutation (new manifest, manifest successor, alias revocation).
- Even if no changes occur, proofs MUST be rotated every six hours to keep
  merkle roots aligned with the latest registry height. Gateways reject proofs
  whose `generated_at_unix` is more than six hours in the past even if TTL
  windows were respected.
- `expires_at_unix` SHOULD be set to `generated_at_unix + 6h`, capped by the
  registry `expiry_epoch`. Clients MUST treat the sooner of those two bounds as
  canonical.
- Torii publishes a `sorafs.alias_proof_rotations_total` counter and emits an
  alert if no rotation occurs within five hours.

## SDK Expectations

- SDK local caches mirror the TTL table above.
- SDKs MUST surface an error when receiving `Sora-Proof-Status: expired` and
  SHOULD expose an API to force-refresh from Torii.
- When operating offline, SDKs MAY continue serving cached manifests for up to
  the `alias_hard_expiry` window, but they MUST gate any write operations or
  notarised outputs until the proof is refreshed.

## Infra & Configuration

- Default values will be wired through `iroha_config::sorafs::AliasCachePolicy`.
  Operators may lower (not raise) the TTLs; stricter values must be reflected in
  documentation and rollouts.
- Gateways expose health checks that fail when the average alias proof age
  exceeds eight minutes or when the most recent refresh failed more than three
  consecutive times.
- CI MUST include a `sorafs alias cache smoke` job that provisions the gateway
  with synthetic proofs and verifies the TTL transitions.

## Cache Busting Signals

Gateways emit alias refresh notifications over both Server-Sent Events (SSE) and
WebSocket feeds so downstream caches can evict stale entries without polling.
Both transports carry the same JSON payload:

```json
{
  "event": "alias-proof-updated",
  "alias": "docs/main",
  "manifest_cid": "bafy…",
  "registry_root": "aa…",
  "registry_height": 18446744073709551618,
  "generated_at_unix": 1_700_001_200,
  "expires_at_unix": 1_700_001_800,
  "bundle_digest": "3bf43e4c…",
  "reason": "renewal"
}
```

- `event` is always `alias-proof-updated` for parity with other governance
  signals. Future reasons (e.g., `revocation`, `rollback`) reuse the payload.
- `manifest_cid` exposes the Norito canonical bytes encoded with multibase base64
  (`m` alphabet) to keep payloads transport-friendly.
- `bundle_digest` is the BLAKE3-256 digest of the Norito-encoded
  `AliasProofBundleV1`, allowing SDKs to short-circuit if they already cached
  that bundle.
- `reason` classifies the trigger:
  - `renewal` — periodic rotation within the standard TTL budget.
  - `mutation` — alias bound to a new manifest.
  - `revocation` — alias removed; clients should drop associated manifests.

SSE delivery uses the `/torii/v1/alias/stream` endpoint with the following
conventions:

- Each message carries `id: <registry_height>-<alias>` so clients can resume
  using the standard `Last-Event-ID` header.
- The HTTP response includes `Cache-Control: no-store` and
  `Content-Type: text/event-stream; charset=utf-8`.
- Gateways send a keep-alive comment every 20 seconds to preserve intermediaries.

WebSocket delivery reuses the `/torii/v1/alias/ws` route. Frames are UTF-8 JSON
documents identical to the SSE payloads and include a `version` field when
breaking changes are introduced. The initial `HELLO` frame echoes the current
registry height so late joiners can decide whether they must hydrate via Torii
queries before accepting live updates.

Downstream components react as follows:

- Gateways purge in-memory entries for the `alias` and immediately fetch fresh
  bundles if the `bundle_digest` is unknown.
- SDK caches skip eviction when `bundle_digest` matches the locally stored hash,
  preventing redundant refetches in scenarios where multiple gateways broadcast
  identical updates.
- Observability pipelines increment
  `torii_sorafs_alias_cache_bust_total{reason}` counters based on the `reason`
  field to track real-world churn.

## Alias Cache Smoke Job

Release engineering maintains a Buildkite job `ci/sorafs_alias_cache_smoke.yml`
that validates end-to-end cache behaviour before promoting gateway builds:

1. **Fixture generation.** Run
   `cargo run -p sorafs_manifest --example gen_alias_proof_fixture` to produce
   two `AliasProofBundleV1` fixtures:
   - `fixtures/alias/docs_main_v1.to` (initial binding).
   - `fixtures/alias/docs_main_v2.to` (rotated binding with a later registry
     height).
   The job stores their BLAKE3 digests to compare against telemetry.
2. **Gateway bootstrap.** Launch a disposable Torii instance with the fixtures
   preloaded via the `--alias-fixture-dir` flag. The job captures the SSE stream
   and WebSocket feed using `scripts/sorafs/alias_stream_tap.py` (headless
   Python client).
3. **TTL verification.**
   - Assert that the first HTTP fetch returns `Sora-Proof` corresponding to `v1`
     and that the SSE/WebSocket tap receives a single `mutation` event.
   - Sleep `alias_refresh_window` seconds, then inject `v2` by calling
     `torii alias rotate --fixture fixtures/alias/docs_main_v2.to`.
   - Confirm caches avoid serving stale proof: the smoke client must observe an
     HTTP `412` until the refreshed bundle is fetched and should log an
     `alias_proof_refresh_total{result="success"}` increment.
4. **Negative path checks.** Issue `torii alias revoke docs/main` and verify the
   next HTTP response is `410 Gone`, the SSE feed reports `reason: "revocation"`,
   and SDK cache subscribers purge state.
5. **Report & artifacts.** The job uploads:
   - SSE/WebSocket transcripts for regression comparisons.
   - A summary JSON containing observed TTLs, digests, and return codes.
   - Grafana snapshot links proving that telemetry counters advanced.

Failures page the SoraFS gateway on-call and block release promotion until the
job passes with the regenerated fixtures.

## References

- SF-4 — On-chain Pin Registry
- SF-5 — SoraFS Gateway Service
- `docs/source/sorafs_architecture_rfc.md`
- `docs/source/sorafs/migration_roadmap.md`
