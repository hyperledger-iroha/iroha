---
title: SoraFS Foundational Prerequisite Signing
summary: Two-phase external-software-signer procedure for the payload-free V1 foundational prerequisite envelope.
---

# SoraFS foundational prerequisite signing

`scripts/build_sorafs_foundational_prerequisite.py` is the only repository
builder for
`sorafs.production_readiness.foundational_prerequisites.v1`. It does not accept
a private key, seed, signer credential, or signing command. The operator supplies
only a trusted Ed25519 public key and public evidence metadata.

The flow has two phases:

1. `prepare` validates the reviewed production deployment, explicit clock,
   freshness window, release sequence and predecessor, and exactly nine
   prerequisite evidence-package manifests supplied as `ID=PATH` in this
   order: `SFM-1`, `SF-1`, `SF-2`, `SF-2c`, `SF-3`, `SF-4`, `SF-5b`, `SF-6`,
   `SF-8a`. Every manifest must carry the same exact ID, deployment, topology
   binding, and a fresh evidence timestamp. Its plural
   `readiness_summaries` array must contain the exact mapped readiness gates in
   the order specified below. Every row names one archive-relative summary
   file and its exact SHA-256. `prepare` pins and hashes the package and every
   referenced summary, verifies each digest, and runs each summary through the
   authoritative bundled readiness-gate validator selected by its `gate`.
   For a package that references multiple summaries,
   `evidence_generated_at_unix` must equal the maximum validated
   `newest_generated_at_unix` across those summaries. The SHA-256 of the exact
   manifest bytes becomes the ordered `evidence_anchor_sha256` value, while
   the exact ordered `{gate, sha256}` rows become that signed prerequisite
   row's `readiness_summary_sha256`. Both are covered by the external-software
   signature. There is no digest-only prerequisite input. It atomically
   creates a new binary signing-payload file. It also independently opens the
   exact 17 `--lane-summary GATE=PATH` inputs through no-follow directory
   descriptors, signs their SHA-256 digests in canonical aggregate order, and
   requires those top-level `lane_summaries` rows to be the exact disjoint
   union of the nine prerequisite mappings. `prepare` additionally requires
   `--topology-qualification-summary`, rehashes its exact bytes, and requires
   all 17 lanes to contain that identical topology binding. The signed body
   covers the qualification-summary digest, exact and canonical manifest
   digests, and deployment context. `prepare` also requires one
   `--resilience-qualification-summary` and a separate reviewed
   `--resilience-qualification-signer-public-key-hex`. It reconstructs and
   authenticates the externally signed 19-requirement receipt, then signs a
   payload-free binding containing the exact summary and receipt digests,
   receipt timestamp, and resilience signer fingerprint beside the topology
   binding. This is an additional bound artifact field, not a tenth
   prerequisite ID or an eighteenth lane. The full aggregate gate rehashes all
   supplied bytes and rejects any missing, stale, unauthenticated, substituted,
   reordered, post-approval, topology-mismatched, or context-mismatched input.
2. The isolated external software signer signs those exact bytes with plain
   Ed25519. Its signed binding fixes the `software` backend, distinct service
   and administrator identities, positive key and policy revisions, a non-zero
   policy digest, and the operator-trusted public-key fingerprint. Do not hash,
   wrap, re-encode, or use Ed25519ph. Export exactly 64 raw signature bytes and
   the matching canonical public receipt under one non-zero operation ID.
   `finalize` revalidates the canonical payload against independently supplied
   deployment and continuity expectations and verifies the detached signature
   against the trusted public key. It then copies the independently pinned
   `sorafs_external_software_signer` verifier plus the exact public Norito
   binding, payload, signature, and receipt into a private temporary directory
   and runs `verify-receipt` with a 30-second bound. Finalization fails unless
   the verifier binary matches the reviewed SHA-256 and returns schema-closed,
   canonical, payload-free validation for the exact operation, software
   backend, distinct service and administrator identities, promotion role and
   domain, Ed25519 key, positive key/policy revisions, policy SHA-256, payload,
   signature, audit commit, live provenance and response attestations. Revoked,
   substituted, stale-head, noisy, malformed, or unverifiable receipts block.
   Only then does `finalize` atomically create the final JSON envelope.
   `finalize` independently takes the same
   `--topology-qualification-summary`, resilience summary, and resilience
   signer public key. It also reopens the independently signed L1 lane-evidence
   inventory using its explicit trusted external-software Ed25519 tuple and
   replays the same exact ordered 17 lane paths; it does not trust paths,
   digests, or authentication claims copied from the signing request. For a
   sequence after 1, both phases also require
   `--previous-envelope` and verify its deterministic encoding, SHA-256,
   deployment identity, trusted Ed25519 signature, immediately preceding
   sequence, and earlier timestamp.

The nine-to-17 mapping is a hard-cut contract:

| Prerequisite | Exact ordered readiness gates |
|--------------|-------------------------------|
| `SFM-1` | `reputation` |
| `SF-1` | `reference_sdk_release` |
| `SF-2` | `pdp` |
| `SF-2c` | `por`, `potr` |
| `SF-3` | `gateway_compliance` |
| `SF-4` | `repair` |
| `SF-5b` | `gateway_load` |
| `SF-6` | `appeal_finance`, `governance_dag`, `hedging_billing`, `orderbook`, `reserve_rent` |
| `SF-8a` | `ai_prescreen`, `moderation_panel`, `pop_credentials`, `transparency` |

These rows are an exact disjoint cover of the canonical 17 readiness gates.
The singular `readiness_summary` form is not accepted. Missing, extra,
duplicated, reordered, or reassigned gates fail; there is no compatibility
path that derives a prerequisite from the top-level lane list alone.

Both commands support reviewed `@ARGFILE` input. Start from
`scripts/examples/sorafs_foundational_prerequisite_prepare.args.example` and
`scripts/examples/sorafs_foundational_prerequisite_finalize.args.example`.
Their angle-bracket values are deliberately invalid; copy the files to a
runtime evidence directory and replace every placeholder with the reviewed
release record before use:

```text
python3 scripts/build_sorafs_foundational_prerequisite.py \
  @/runtime/evidence/foundational-prepare.args

# Sign the emitted file as exact bytes in the isolated external software signer.
sorafs_external_software_signer sign \
  --binding /runtime/signer/promotion.binding.norito \
  --request-socket /run/sorafs-promotion-signer/request.sock \
  --administrator-socket /run/sorafs-promotion-signer/admin.sock \
  --operation-id <NONZERO-LOWERCASE-32-BYTE-OPERATION-ID> \
  --payload /runtime/evidence/foundational-signing-payload.bin \
  --signature-out /runtime/evidence/foundational-signature.bin \
  --receipt-out /runtime/evidence/promotion-signature-receipt.json

python3 scripts/build_sorafs_foundational_prerequisite.py \
  @/runtime/evidence/foundational-finalize.args
```

Each `--prerequisite ID=PATH` file uses this closed shape. Summary paths are
relative to the manifest directory; absolute paths, traversal, and path
substitution are rejected. This `SF-2c` example shows the plural form and its
required order.

```json
{
  "deployment": {
    "deployment_id": "sorafs-mainnet-2026-07",
    "environment": "production"
  },
  "errors": [],
  "evidence_generated_at_unix": 1785370000,
  "prerequisite_id": "SF-2c",
  "readiness_summaries": [
    {
      "gate": "por",
      "path": "summaries/por-ready.json",
      "sha256": "<SHA-256-OF-EXACT-POR-SUMMARY-FILE>"
    },
    {
      "gate": "potr",
      "path": "summaries/potr-ready.json",
      "sha256": "<SHA-256-OF-EXACT-POTR-SUMMARY-FILE>"
    }
  ],
  "schema": "sorafs.production_readiness.foundational_prerequisite_evidence_package.v1",
  "status": "verified",
  "topology_qualification": {
    "canonical_manifest_sha256": "<CANONICAL-TOPOLOGY-MANIFEST-SHA256>",
    "deployment_id": "sorafs-mainnet-2026-07",
    "environment": "production",
    "manifest_sha256": "<EXACT-TOPOLOGY-MANIFEST-SHA256>",
    "qualification_summary_sha256": "<QUALIFICATION-SUMMARY-SHA256>"
  }
}
```

The corresponding signed prerequisite row is:

```json
{
  "evidence_anchor_sha256": "<SHA-256-OF-EXACT-SF-2C-PACKAGE>",
  "evidence_generated_at_unix": 1785370000,
  "id": "SF-2c",
  "readiness_summary_sha256": [
    {
      "gate": "por",
      "sha256": "<SHA-256-OF-EXACT-POR-SUMMARY-FILE>"
    },
    {
      "gate": "potr",
      "sha256": "<SHA-256-OF-EXACT-POTR-SUMMARY-FILE>"
    }
  ],
  "status": "verified"
}
```

The aggregate checker does not trust the builder's result. It validates the
signed per-prerequisite mapping, cross-binds its grouped digest rows to the
signed top-level `lane_summaries`, then rehashes the independently supplied 17
aggregate inputs and requires every gate digest to match. A valid signature
over an old singular package, a digest-only wrapper, or a mismapped lane cannot
bypass these checks. The payload-free aggregate reports
`signer_qualification=software-key-qualified` only after this signed software
binding is valid; `hsm-qualified` is not an admitted value. Tests may exercise
the path with fixture content, but
fixtures have no production evidentiary standing and must never be submitted
as promotion evidence.

Outputs are new, mode `0600` files. Existing destinations, symlinks, symlinked
parents, hardlinked or group/world-writable inputs, unsafe path spellings,
malformed canonical JSON, stale/future timestamps, duplicate/reordered IDs or
anchors, manifest/summary schema mismatches, summary digest or relative-path
swaps, missing/reordered lane summaries, lane schema/status mismatches, zero
values, fingerprint mismatches, parent-path swaps, and continuity mismatches
fail closed. Inputs and output parents are pinned through
no-follow directory descriptors; a path-identity change during a read or
atomic publication is rejected. The builder never writes a synthetic envelope
and no production envelope belongs in the repository.

For release sequence 1, the predecessor is 32 zero bytes and
`--previous-envelope` is forbidden. Later sequences use the lowercase SHA-256
of the exact preceding finalized envelope file and pass that same file through
`--previous-envelope`; the builder requires its signed release sequence to be
exactly one lower. Keep that digest, the current sequence, and the trusted
public key outside the envelope invocation plan used for approval.

Pass the finalized envelope to
`scripts/run_sorafs_production_readiness.py` as
`--foundational-prerequisite-summary`, with the same trusted public key,
release sequence, predecessor digest, deployment ID, environment, explicit
clock, freshness window, exact `--topology-qualification-summary`, exact
`--resilience-qualification-summary`, and its separately reviewed signer
public key. Deterministic replay snapshots the topology summary and envelope,
resilience summary, signed lane inventory, foundational envelope, and the 17
lanes as exactly 22 inputs. The inventory is an additional bound input, so
aggregate summary counts remain exactly 17/17. The
aggregate gate remains authoritative:
creating or signing this envelope does not make any readiness lane ready and
does not authorize Taira or Minamoto cutover. The repository currently
contains no genuine reviewed production lane summaries, prerequisite
packages, externally signed foundational envelope, or ready aggregate; local
fixtures and negative tests are not substitutes for that missing evidence.
