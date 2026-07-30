---
title: SoraFS Foundational Prerequisite Signing
summary: Two-phase external-HSM procedure for the payload-free V1 foundational prerequisite envelope.
---

# SoraFS foundational prerequisite signing

`scripts/build_sorafs_foundational_prerequisite.py` is the only repository
builder for
`sorafs.production_readiness.foundational_prerequisites.v1`. It does not accept
a private key, seed, HSM credential, or signing command. The operator supplies
only a trusted Ed25519 public key and public evidence metadata.

The flow has two phases:

1. `prepare` validates the reviewed production deployment, explicit clock,
   freshness window, release sequence and predecessor, and exactly nine
   prerequisite evidence-package manifests supplied as `ID=PATH` in this
   order: `SFM-1`, `SF-1`, `SF-2`, `SF-2c`, `SF-3`, `SF-4`, `SF-5b`, `SF-6`,
   `SF-8a`. Every manifest must carry the same exact ID, deployment, topology
   binding, and a fresh evidence timestamp. It names one archive-relative
   readiness-summary file and its exact SHA-256. `prepare` pins and hashes both
   files, verifies the digest, and runs the summary through the authoritative
   bundled readiness-gate validator selected by its `gate`. The SHA-256 of the
   exact manifest bytes becomes the existing ordered
   `evidence_anchor_sha256` value covered by the external-HSM signature. There
   is no digest-only prerequisite input. It atomically creates a new binary
   signing-payload file. It also opens the exact 17 ready lane
   summaries through no-follow directory descriptors and signs their SHA-256
   digests in canonical aggregate order. `prepare` additionally requires
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
2. The external HSM signs those exact bytes with plain Ed25519. Do not hash,
   wrap, re-encode, or use Ed25519ph. Export exactly 64 raw signature bytes.
   `finalize` revalidates the canonical payload against independently supplied
   deployment and continuity expectations, verifies the detached signature
   against the trusted public key, and atomically creates the final JSON
   envelope. `finalize` independently takes the same
   `--topology-qualification-summary`, resilience summary, and resilience
   signer public key; it does not trust paths, digests, or authentication
   claims copied from the signing request. For a sequence after 1, both phases also require
   `--previous-envelope` and verify its deterministic encoding, SHA-256,
   deployment identity, trusted Ed25519 signature, immediately preceding
   sequence, and earlier timestamp.

Both commands support reviewed `@ARGFILE` input. Start from
`scripts/examples/sorafs_foundational_prerequisite_prepare.args.example` and
`scripts/examples/sorafs_foundational_prerequisite_finalize.args.example`.
Their angle-bracket values are deliberately invalid; copy the files to a
runtime evidence directory and replace every placeholder with the reviewed
release record before use:

```text
python3 scripts/build_sorafs_foundational_prerequisite.py \
  @/runtime/evidence/foundational-prepare.args

# Sign the emitted file as exact bytes in the external HSM, producing a
# 64-byte raw detached signature.

python3 scripts/build_sorafs_foundational_prerequisite.py \
  @/runtime/evidence/foundational-finalize.args
```

Each `--prerequisite ID=PATH` file uses this closed shape. The summary path is
relative to the manifest directory; absolute paths, traversal, and path
substitution are rejected.

```json
{
  "deployment": {
    "deployment_id": "sorafs-mainnet-2026-07",
    "environment": "production"
  },
  "errors": [],
  "evidence_generated_at_unix": 1785370000,
  "prerequisite_id": "SF-2",
  "readiness_summary": {
    "gate": "pdp",
    "path": "summaries/pdp-ready.json",
    "sha256": "<SHA-256-OF-EXACT-PDP-SUMMARY-FILE>"
  },
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
public key. Deterministic replay snapshots topology, resilience, foundation,
and the 17 lanes as 20 inputs while aggregate summary counts remain exactly
17/17. The
aggregate gate remains authoritative:
creating or signing this envelope does not make any readiness lane ready and
does not authorize Taira or Minamoto cutover.
