---
title: "Pin Registry Manifest Validation Plan"
---

# Pin Registry Manifest Validation Status (SF-4)

This page records the completed SF-4 manifest-validation wiring for SoraFS pin
registration. The shared validation path now lives in `sorafs_manifest`, is used
by Torii submission handling, and is enforced by the on-chain pin registry entry
points before state or fee side effects.

## Implemented Goals

1. Host-side submission paths verify manifest structure, chunking profile, pin
   policy, and governance envelopes before accepting proposals.
2. Torii and gateway-facing services reuse the same validation routines so hosts
   and clients see deterministic acceptance and refusal labels.
3. Unit and integration tests cover positive registration, validator error cases,
   governance-policy rejections, and no-side-effect failures.

## Current Architecture

```mermaid
flowchart LR
    cli["sorafs_cli / SDK clients"] --> torii["Torii /v1/sorafs/pin/register"]
    torii --> validator["sorafs_manifest::validation"]
    torii --> manifest["required canonical ManifestV1 from manifest_payload"]
    validator --> registry["Pin Registry ISI"]
    registry --> pending["pending record when council approval is required"]
    pending --> approval["permissioned ApprovePinManifest + verified envelope"]
    approval --> state["approved manifest / aliases / replication orders"]
    registry --> state
```

## Shipped Components

- `sorafs_manifest::validation` provides shared chunker, pin-policy, and
  `ManifestV1` validation helpers.
- Full-manifest admission is bounded to 512 KiB of canonical Norito and rejects
  inline/unregistered chunker profiles, substituted profile geometry, inert
  root/CAR commitments, zero retention, duplicate aliases or metadata keys,
  oversized proof/metadata/signature collections, and invalid or replay-shaped
  Ed25519 council signatures. Signatures are verified over the manifest with
  its governance-signature list cleared, eliminating the self-reference.
- `manifest_pin_policy_constraints_from_config` maps governance configuration
  into `sorafs_manifest::PinPolicyConstraints`.
- `/v1/sorafs/pin/register` requires `manifest_payload`, rejects non-canonical or
  oversized Norito before transaction construction, and returns stable
  `sorafs_pin_*` application-validation labels. Summary commitments supplied by
  callers are not an authority boundary.
- `RegisterPinManifest` carries those exact canonical bytes. Consensus decodes
  them under explicit element, field, allocation, and depth budgets; revalidates
  the complete `ManifestV1`; and derives the manifest digest, embedded
  chunk-plan SHA3-256 digest, 36-byte content root CID, chunker handle, content length, and pin policy before any state or
  fee side effect. Consensus—not Torii—also enforces the
  `require_council_signatures` boundary: governed submissions stay pending and
  cannot publish aliases or issue replication orders.
- The first `ApprovePinManifest` transition requires the actual bounded council
  envelope, verifies every strong Ed25519 signature over the registered manifest
  digest, rejects digest-only approval, and requires the approval epoch to fall
  between submission and retention expiry. A same-epoch replay may only reuse
  the already-recorded envelope digest; it cannot replace approval history.
  Only after all fallible validation and automatic-order construction succeeds
  are reserved aliases and replication orders published.
- Public pin registration remains permissionless, but attaching or reserving an
  alias requires `CanBindSorafsAlias`. The proof must be canonical bounded
  Norito, commit to the exact canonical content root CID and submission/retention epochs, contain at most a
  64-level Merkle path and 64 distinct sorted council signers, and fit within the
  1 MiB proof ceiling.
- Retirement epochs cannot predate submission or approval, and retirement
  reasons are bounded, canonical, control-free text.
- Tests cover chunker/profile checks, council-signature policy,
  replica floors/ceilings, retention ceilings, storage-class allowlists,
  on-chain registration acceptance, and governance-policy rejections.

## Completion Matrix

| Area | Status | Evidence |
|------|--------|----------|
| Shared validator | Done | `validate_chunker_handle`, `validate_pin_policy`, and `validate_manifest` live in `sorafs_manifest::validation`. |
| Policy wiring | Done | Governance config is mapped into `PinPolicyConstraints`; DTO and full-manifest paths use the same limits. |
| Torii integration | Done | `/v1/sorafs/pin/register` requires the canonical manifest payload and emits stable `sorafs_pin_*` error labels. |
| Contract enforcement | Done | `RegisterPinManifest` resource-bounds, canonicalizes, validates, and derives every persisted manifest commitment before state mutation; governed submissions remain pending until a permissioned, envelope-backed approval. |
| Tests | Done | Validator and integration tests cover policy, chunker geometry substitution, direct-instruction bypass, council-envelope canonicalization/resource attacks, and side-effect guarantees. |
| Docs | Done | Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap docs describe the shared validation path. |

## Operational Notes

- Manifest validation rejects unknown registered chunker profile IDs instead of
  inferring layout from inline parameters. The first release has one canonical
  Norito layout; alternate legacy flag layouts are rejected at Torii ingress.
- Council-signature requirements are driven by governance configuration. Torii
  always requires `manifest_payload` so malformed full manifests can be rejected
  early, regardless of whether approval is automatic or governed. This host-side check is not the authority
  boundary: consensus keeps the record pending until `ApprovePinManifest`
  verifies the supplied council envelope under an account carrying
  `CanApproveSorafsPin`.
- A pending manifest reserves its digest and alias claim against conflicts but
  does not publish the alias or create a replication order. Alias claims can
  only be supplied by an authority carrying `CanBindSorafsAlias`. The approval
  epoch becomes the issuance epoch for any deferred automatic replication
  order.
- Error labels are part of the operator contract. Keep Torii, CLI, OpenAPI, and
  tests aligned whenever adding validation cases.
- Large-manifest performance should be measured in release rehearsals; cache only
  deterministic digest results and never bypass validation.

## Remaining Rollout Evidence

1. Archive release-candidate logs for positive registration and governed-policy
   rejection through Torii and on-chain execution.
2. Attach OpenAPI/CLI examples that demonstrate the stable `sorafs_pin_*` labels
   for common failures.
3. Record any production performance baseline for large manifests in the
   migration ledger before widening operator usage.
