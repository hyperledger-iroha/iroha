---
title: Nexus Privacy Commitments
sidebar_label: Privacy Commitments
description: Domain-separated Merkle commitments and registry workflow for NX-10 private lanes.
---

# Privacy Commitment & Proof Framework (NX-10)

> **Status:** Merkle commitments available; proof-system commitments deferred
> **Owners:** Cryptography WG · Privacy WG · Nexus Core WG  
> **Related code:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

Nexus lanes can require a transaction to prove membership in a dataset committed
by the lane governance manifest. The first release supports one scheme:
domain-separated SHA-256 Merkle commitments.

The former `SnarkCircuit` descriptor did not invoke a proof system. It only
compared hashes of attacker-visible bytes, so it has been removed from the
runtime, data model, manifest parser, status output, and SDKs. A proof-system
scheme must not be exposed again until admission resolves a verifying key from
on-chain state and calls the corresponding cryptographic verifier.

## Objectives and scope

- Keep commitment identifiers consistent across manifests, admission, and SDKs.
- Verify every lane witness against node-held manifest state.
- Separate raw leaves from internal nodes cryptographically.
- Reject degenerate or sparse authentication paths at the verification
  boundary, including witnesses constructed directly from decoded wire data.

DA fan-out, relay messaging, and settlement routing are separate layers. See
`nexus_cross_lane.md` for those topics.

## Lane commitment model

The registry stores `LanePrivacyCommitment` entries keyed by a
`LaneCommitmentId`:

```rust
use iroha_crypto::privacy::{
    LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment,
};

let commitment = LanePrivacyCommitment::merkle(
    LaneCommitmentId::new(1),
    MerkleCommitment::from_root_bytes(root_bytes, 16),
);
```

`LaneCommitmentId` is a 16-bit identifier local to a lane manifest.
`MerkleCommitment` records the root and the maximum accepted authentication-path
depth.

## Manifest schema

The first-release manifest parser accepts only `scheme: "merkle"`:

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
    }
  ]
}
```

`max_depth` must be greater than zero. Duplicate IDs, malformed roots, missing
Merkle fields, and every other scheme—including `snark`—make the manifest
invalid. The lane-registry bundler and verifier apply the same Merkle-only
validation before publishing or accepting a bundle. Commitment-only and
split-replica lanes fail readiness if their manifest has no privacy
commitments.

## Canonical lane Merkle hashing

Lane privacy reuses `MerkleProof<[u8; 32]>` only as an index-and-path container.
It does not use the generic Merkle verifier or change its semantics.

For a raw 32-byte leaf `L`, calculate:

```text
leaf_hash = SHA-256("iroha:nexus:lane-privacy:merkle:leaf:v1\0" || L)
```

For canonical child hashes `left` and `right`, calculate:

```text
node_hash = SHA-256(
    "iroha:nexus:lane-privacy:merkle:node:v1\0" || left || right
)
```

Each digest is stored using the canonical Iroha `Hash` representation, whose
least-significant marker bit is set. Producers must use
`lane_merkle_leaf_hash` and `lane_merkle_node_hash`, or reproduce those exact
bytes, before publishing a manifest root.

The witness leaf is raw data. Do not pre-hash it. Audit-path entries are
canonical sibling hashes ordered from the leaf toward the root. The low bit of
`leaf_index` selects whether the accumulator is the left or right child at the
first level; successive bits select later levels.

The V1 interoperability vector uses raw leaf `aa` repeated 32 times, sibling
digest `bb` repeated 32 times, and `leaf_index = 0`. Its marked leaf hash is
`7b08f69e5888269358d2f3029831ede108d0f7b464449001bcc5f7a64f498447`
and its marked root is
`175dd23c29dda55ead958e0b1db68811f2108aa9a6f8d2222bec59bd2aed3a09`.

### Required proof shape

Admission rejects a witness when:

- its audit path is empty;
- any audit-path entry is absent or `null`;
- its path is deeper than the manifest's `max_depth`;
- its leaf index is outside the range represented by the path; or
- its domain-separated implied root differs from the manifest root.

These checks run in `MerkleCommitment::verify`, after wire decoding. The
`LanePrivacyProof::merkle_from_raw_path` constructor performs the same
non-empty-path check for early client feedback, but it is not the security
boundary.

Verification example:

```rust
use iroha_crypto::privacy::{MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(raw_leaf, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

## Runtime registry and admission

1. `LaneManifestRegistry` parses and validates the manifest once.
2. `LanePrivacyRegistry` snapshots its per-lane commitments.
3. Admission reads `ProofAttachment.lane_privacy`, resolves the routed lane and
   commitment ID from the registry, and verifies the attached Merkle witness.
4. Only successfully verified IDs enter
   `LaneComplianceContext::verified_privacy_commitments`.
5. A compliance rule using `privacy_commitments_any_of` remains unsatisfied
   unless at least one required ID verified.

Torii status exposes only Merkle commitment metadata: the commitment ID, root,
and maximum depth. SDK decoders reject any other lane-privacy scheme.

## Operational checklist

- Build roots with the versioned lane leaf and node domains above.
- Never publish a root from the generic untagged `MerkleTree` helper.
- Include at least one real sibling in every proof; do not use sparse
  `None`/`null` path entries.
- Keep the manifest root and proof generator on the same canonical Iroha hash
  representation.
- Rotate a commitment ID or root whenever the committed dataset changes.
- Treat a request for a SNARK lane commitment as unsupported until a real
  on-chain verifying-key-backed verifier is implemented and audited.

Focused unit coverage lives in
[`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs), with registry and
admission coverage in `iroha_core::interlane` and
`integration_tests/tests/nexus/privacy_proof_enforcement.rs`.
