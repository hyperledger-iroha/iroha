---
lang: kk
direction: ltr
source: docs/source/nexus_privacy_commitments.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 10526b4c4d0abf2937785f13924d6265003acc53bd65fcc8c8e822c580b694b3
source_last_modified: "2026-02-05T09:16:39.297685+00:00"
translation_last_reviewed: 2026-02-07
title: Nexus Privacy Commitments
sidebar_label: Privacy Commitments
description: Canonical commitment schemes, proof hashes, and registry workflow for NX-10 private lanes.
---

# Privacy Commitment & Proof Framework (NX-10)

> **Status:** 🈴 Completed (NX-10)  
> **Owners:** Cryptography WG · Privacy WG · Nexus Core WG  
> **Related code:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

NX-10 introduces a common commitment surface for private lanes. Every dataspace publishes a deterministic descriptor that ties its Merkle roots or zk-SNARK circuits to reproducible hashes. The global Nexus ring can therefore validate cross-lane transfers and confidentiality proofs without bespoke parsers.

## Objectives & Scope

- Canonicalise commitment identifiers so governance manifests and SDKs agree on the numeric slot used during admission.
- Ship reusable verification helpers (`iroha_crypto::privacy`) so runtimes, Torii, and off-chain auditors evaluate proofs consistently.
- Bind zk-SNARK proofs to the canonical verifying-key digest and deterministic public-input encodings. No ad-hoc transcripts.
- Document registry/export workflow so lane bundles and governance evidence include the same hashes that the runtime enforces.

Out of scope for this note: DA fan-out mechanics, relay messaging, and settlement router plumbing. See `nexus_cross_lane.md` for those layers.

## Lane Commitment Model

The registry stores an ordered list of `LanePrivacyCommitment` entries:

```rust
use iroha_crypto::privacy::{
    CommitmentScheme, LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, SnarkCircuit,
    SnarkCircuitId,
};

let id = LaneCommitmentId::new(1);
let merkle = LanePrivacyCommitment::merkle(
    id,
    MerkleCommitment::from_root_bytes(root_bytes, 16),
);

let snark = LanePrivacyCommitment::snark(
    LaneCommitmentId::new(2),
    SnarkCircuit::new(
        SnarkCircuitId::new(42),
        verifying_key_digest,
        statement_hash,
        proof_hash,
    ),
);
```

- **`LaneCommitmentId`** — 16‑bit stable identifier recorded in lane manifests.
- **`CommitmentScheme`** — `Merkle` or `Snark`. Future variants (e.g., bulletproofs) extend the enum.
- **Copy semantics** — all descriptors implement `Copy` so configs can reuse them without heap churn.

The registry rides along the Nexus lane bundle (`scripts/nexus/lane_registry_bundle.py`). When governance approves a new commitment, the bundle updates both the JSON manifest and the Norito overlay consumed by admission.

### Manifest Schema (`privacy_commitments`)

Lane manifests now expose a `privacy_commitments` array. Each entry assigns an ID and scheme and
includes the scheme-specific parameters:

```json
{
  "lane": "cbdc",
  "governance": "council",
  "privacy_commitments": [
    {
      "id": 1,
      "scheme": "merkle",
      "merkle": {
        "root": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "max_depth": 16
      }
    },
    {
      "id": 2,
      "scheme": "snark",
      "snark": {
        "circuit_id": 5,
        "verifying_key_digest": "0x…",
        "statement_hash": "0x…",
        "proof_hash": "0x…"
      }
    }
  ]
}
```

The registry bundler copies the manifest, records the commitment `(id, scheme)` tuples in
`summary.json`, and CI (`lane_registry_verify.py`) re-parses the manifest to ensure the summary
matches the on-disk contents.

## Merkle Commitments

`MerkleCommitment` captures a canonical `HashOf<MerkleTree<[u8;32]>>` and a maximum audit-path depth. Operators export the root directly from their prover or ledger snapshot; no re-hashing inside the registry.

Verification flow:

```rust
use iroha_crypto::privacy::{LanePrivacyCommitment, MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(leaf_bytes, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

- Proofs exceeding `max_depth` emit `PrivacyError::MerkleProofExceedsDepth`.
- Audit paths reuse the `iroha_crypto::MerkleProof` utilities so shielded pools and Nexus private lanes share the same serialization.
- Hosts that ingest external pools convert shielded leaves via `MerkleTree::shielded_leaf_from_commitment` before constructing the witness.

### Operational Checklist

- Publish the `(id, root, depth)` tuple in the lane manifest summary and evidence bundle.
- Attach the Merkle inclusion proof for every cross-lane transfer to the admission log; the helper reproduces the root byte-for-byte, so audits only compare the exported hash.
- Track depth budgets in telemetry (`nexus_privacy_commitments.merkle.depth_used`) so alerts fire before rollouts exceed configured maxima.

## zk-SNARK Commitments

`SnarkCircuit` entries bind four fields:

| Field | Description |
|-------|-------------|
| `circuit_id` | Governance-controlled identifier for the circuit/version pair. |
| `verifying_key_digest` | Blake3 hash of the canonical verifying key (DER or Norito encoding). |
| `statement_hash` | Blake3 hash of the canonical public-input encoding (Norito or Borsh). |
| `proof_hash` | Blake3 hash of `verifying_key_digest || proof_bytes`. |

Runtime helpers:

```rust
use iroha_crypto::privacy::{
    hash_proof, hash_public_inputs, LanePrivacyCommitment, PrivacyWitness, SnarkWitness,
};

let witness = SnarkWitness {
    public_inputs: encoded_inputs,
    proof: proof_bytes,
};

LanePrivacyCommitment::snark(id, circuit)
    .verify(PrivacyWitness::Snark(witness))?;
```

- `hash_public_inputs` and `hash_proof` live in the same module; SDKs must call them when generating manifests or proofs to avoid format drift.
- Any mismatch between the recorded statement hash and the presented public inputs yields `PrivacyError::SnarkStatementMismatch`.
- Proof bytes must already be in their canonical compressed form (Groth16, Plonk, etc.). The helper only verifies the hash binding; full curve verification stays in the prover/verifier service.

### Evidence Workflow

1. Export the verifying key digest and proof hash into the lane bundle manifest.
2. Attach the raw proof artefact to the governance evidence package so auditors can recompute `hash_proof`.
3. Publish the canonical public-input encoding (hex or Norito) alongside the statement hash for deterministic replays.

## Proof Ingestion Pipeline

1. **Lane manifests** declare which `LaneCommitmentId` governs each contract or programmable-money bucket.
2. **Admission** consumes `ProofAttachment.lane_privacy` entries, verifies the attached witnesses for the routed lane via `LanePrivacyRegistry::verify`, and feeds the validated commitment ids into lane compliance (`privacy_commitments_any_of`) so programmable-money allow/deny rules enforce proof presence before queueing.
3. **Telemetry** increments `nexus_privacy_commitments.{merkle,snark}` counters with the `LaneId`, `commitment_id`, and outcome (`ok`, `depth_mismatch`, etc.).
4. **Governance evidence** uses the same helper to regenerate acceptance reports (`ci/check_nexus_lane_registry_bundle.sh` hooks into bundle verification once SNARK metadata lands).

### Operator Visibility

Torii’s `/v2/sumeragi/status` endpoint now exposes the `lane_governance[].privacy_commitments`
array so operators and SDKs can diff the live registry against the published manifests without
re-reading the bundle. The snapshot is built inside
`crates/iroha_core/src/sumeragi/status.rs`, exported by Torii’s REST/JSON handlers
(`crates/iroha_torii/src/routing.rs`), and decoded by every client (`javascript/iroha_js/src/toriiClient.js`,
`python/iroha_python/src/iroha_python/client.py`, `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`),
mirroring the manifest schema for both Merkle roots and SNARK digests.

Commitment-only or split-replica lanes now fail admission if their manifest omits the
`privacy_commitments` section, ensuring programmable-money flows cannot start until deterministic
proof anchors ship with the bundle.

## Runtime Registry & Admission

- `LanePrivacyRegistry` (`crates/iroha_core/src/interlane/mod.rs`) snapshots the manifest loader’s
  `LaneManifestStatus` structures and stores per-lane commitment maps keyed by `LaneCommitmentId`.
  The transaction queue and `State` install this registry alongside every manifest reload
  (`Queue::install_lane_manifests`, `State::install_lane_manifests`), so admission and consensus
  validation always have access to the canonical commitments.
- `LaneComplianceContext` now carries an optional `Arc<LanePrivacyRegistry>` reference
  (`crates/iroha_core/src/compliance/mod.rs`). When `LaneComplianceEngine` evaluates a transaction,
  programmable-money flows can inspect the same per-lane commitments exposed through Torii before
  queueing the payload.
- Admission and core validation keep an `Arc<LanePrivacyRegistry>` next to the governance manifest
  handle (`crates/iroha_core/src/queue.rs`, `crates/iroha_core/src/state.rs`), guaranteeing that
  programmable-money modules and future interlane hosts read a consistent view of the privacy
  descriptors even as manifests rotate.

## Runtime Enforcement

Lane privacy proofs now ride along transaction attachments via `ProofAttachment.lane_privacy`.
Torii admission and `iroha_core` validation verify each attachment against the routed lane’s
registry using `LanePrivacyRegistry::verify`, record the proven commitment ids, and feed them into
the compliance engine. Any rule that specifies `privacy_commitments_any_of` now evaluates to `false`
unless a matching attachment verifies successfully, so programmable-money lanes cannot be queued or
committed without the required witness. Unit coverage lives in `interlane::tests` and the lane
compliance tests to keep the attachment path and policy guardrails stable.

For immediate experimentation, see the unit tests in [`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs) which demonstrate both success and failure cases for Merkle and zk-SNARK commitments.
