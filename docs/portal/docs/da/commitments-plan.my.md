---
lang: my
direction: ltr
source: docs/portal/docs/da/commitments-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fd1985901145d0dbcc587d953b0b1a3b5210132c3f915ffd36ec81fbe0692b7
source_last_modified: "2026-01-22T14:45:01.276618+00:00"
translation_last_reviewed: 2026-02-07
---

title: Data Availability Commitments Plan
sidebar_label: Commitments Plan
description: Block, RPC, and proof plumbing for embedding DA commitments in Nexus.
---

:::note Canonical Source
:::

# Sora Nexus Data Availability Commitments Plan (DA-3)

_Drafted: 2026-03-25 — Owners: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 extends the Nexus block format so every lane embeds deterministic records
describing the blobs accepted by DA-2. This note captures the canonical data
structures, block pipeline hooks, light-client proofs, and Torii/RPC surfaces
that must land before validators can rely on DA commitments during admission or
governance checks. All payloads are Norito-encoded; no SCALE or ad-hoc JSON.

## Objectives

- Carry per-blob commitments (chunk root + manifest hash + optional KZG
  commitment) inside every Nexus block so peers can reconstruct availability
  state without consulting off-ledger storage.
- Provide deterministic membership proofs so light clients can verify that a
  manifest hash was finalised in a given block.
- Expose Torii queries (`/v1/da/commitments/*`) and proofs that let relays,
  SDKs, and governance automation audit availability without replaying every
  block.
- Keep the existing `SignedBlockWire` envelope canonical by threading the new
  structures through the Norito metadata header and block hash derivation.

## Scope Overview

1. **Data model additions** in `iroha_data_model::da::commitment` plus block
   header changes in `iroha_data_model::block`.
2. **Executor hooks** so `iroha_core` ingests DA receipts emitted by Torii
   (`crates/iroha_core/src/queue.rs` and `crates/iroha_core/src/block.rs`).
3. **Persistence/indexes** so the WSV can answer commitment queries quickly
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii RPC additions** for list/query/prove endpoints under
   `/v1/da/commitments`.
5. **Integration tests + fixtures** validating the wire layout and proof flow in
   `integration_tests/tests/da/commitments.rs`.

## 1. Data Model Additions

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` reuses the existing 48-byte point used under
  `iroha_crypto::kzg`. When absent we fall back to Merkle proofs only.
- `proof_scheme` is derived from the lane catalog; Merkle lanes reject KZG
  payloads while `kzg_bls12_381` lanes require non-zero KZG commitments. Torii
  currently only produces Merkle commitments and rejects KZG-configured lanes.
- `KzgCommitment` reuses the existing 48-byte point used under
  `iroha_crypto::kzg`. When absent on Merkle lanes we fall back to Merkle proofs
  only.
- `proof_digest` anticipates DA-5 PDP/PoTR integration so the same record
  enumerates the sampling schedule used to keep blobs live.

### 1.2 Block header extension

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

The bundle hash feeds into both the block hash and `SignedBlockWire` metadata.
overhead.

Implementation note: `BlockPayload` and the transparent `BlockBuilder` now expose
`da_commitments` setters/getters (see `BlockBuilder::set_da_commitments` and
`SignedBlock::set_da_commitments`), so hosts can attach a pre-built bundle
before sealing a block. All helper constructors default the field to `None`
until Torii threads real bundles through.

### 1.3 Wire encoding

- `SignedBlockWire::canonical_wire()` appends the Norito header for
  `DaCommitmentBundle` immediately after the existing transaction list. The
  version byte is `0x01`.
- `SignedBlockWire::decode_wire()` rejects bundles whose `version` is unknown,
  matching the Norito policy described in `norito.md`.
- Hash derivation updates exist only in `block::Hasher`; light clients decoding
  the existing wire format automatically gain the new field because the Norito
  header advertises its presence.

## 2. Block Production Flow

1. Torii DA ingest finalises a `DaIngestReceipt` and publishes it on the
   internal queue (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` collects all receipts whose `lane_id` matches the block under
   construction, deduplicating by `(lane_id, client_blob_id, manifest_hash)`.
3. Right before sealing, the block builder sorts commitments by `(lane_id,
   epoch, sequence)` to keep the hash deterministic, encodes the bundle with the
   Norito codec, and updates `da_commitments_hash`.
4. The full bundle is stored in the WSV and emitted alongside the block inside
   `SignedBlockWire`.

If block creation fails the receipts remain in the queue so the next block
attempt can pick them up; the builder records the last included `sequence` per
lane to avoid replay attacks.

## 3. RPC & Query Surface

Torii exposes three endpoints:

| Route | Method | Payload | Notes |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (range filter by lane/epoch/sequence, pagination) | Returns `DaCommitmentPage` with total count, commitments, and block hash. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (lane + manifest hash or `(epoch, sequence)` tuple). | Responds with `DaCommitmentProof` (record + Merkle path + block hash). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Stateless helper that replays the block hash calculation and validates inclusion; used by SDKs that cannot link directly to `iroha_crypto`. |

All payloads live under `iroha_data_model::da::commitment`. Torii routers mount
the handlers next to the existing DA ingest endpoints to reuse token/mTLS
policies.

## 4. Inclusion Proofs & Light Clients

- The block producer builds a binary Merkle tree over the serialized
  `DaCommitmentRecord` list. The root feeds `da_commitments_hash`.
- `DaCommitmentProof` packages the target record plus a vector of `(sibling_hash,
  position)` entries so verifiers can reconstruct the root. Proofs also include
  the block hash and signed header so light clients can verify finality.
- CLI helpers (`iroha_cli app da prove-commitment`) wrap the proof request/verify
  cycle and surface Norito/hex outputs for operators.

## 5. Storage & Indexing

WSV stores commitments in a dedicated column family keyed by `manifest_hash`.
Secondary indexes cover `(lane_id, epoch)` and `(lane_id, sequence)` so queries
avoid scanning full bundles. Each record tracks the block height that sealed it,
allowing catch-up nodes to rebuild the index quickly from the block log.

## 6. Telemetry & Observability

- `torii_da_commitments_total` increments whenever a block seals at least one
  record.
- `torii_da_commitment_queue_depth` tracks receipts waiting to be bundled (per
  lane).
- Grafana dashboard `dashboards/grafana/da_commitments.json` visualises block
  inclusion, queue depth, and proof throughput so DA-3 release gates can audit
  behaviour.

## 7. Testing Strategy

1. **Unit tests** for `DaCommitmentBundle` encoding/decoding and block hash
   derivation updates.
2. **Golden fixtures** under `fixtures/da/commitments/` capturing canonical
   bundle bytes and Merkle proofs.
3. **Integration tests** booting two validators, ingesting sample blobs, and
   asserting that both nodes agree on the bundle contents and query/proof
   responses.
4. **Light-client tests** in `integration_tests/tests/da/commitments.rs`
   (Rust) that call `/prove` and verify the proof without talking to Torii.
5. **CLI smoke** script `scripts/da/check_commitments.sh` to keep operator
   tooling reproducible.

## 8. Rollout Plan

| Phase | Description | Exit Criteria |
|-------|-------------|---------------|
| P0 — Data model merge | Land `DaCommitmentRecord`, block header updates, and Norito codecs. | `cargo test -p iroha_data_model` green with new fixtures. |
| P1 — Core/WSV wiring | Thread queue + block builder logic, persist indexes, and expose RPC handlers. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` pass with bundle proof assertions. |
| P2 — Operator tooling | Ship CLI helpers, Grafana dashboard, and proof verification doc updates. | `iroha_cli app da prove-commitment` works against devnet; dashboard displays live data. |
| P3 — Governance gate | Enable block validator requiring DA commitments on the lanes flagged in `iroha_config::nexus`. | Status entry + roadmap update mark DA-3 as 🈴. |

## Open Questions

1. **KZG vs Merkle defaults** — Should small blobs always skip KZG commitments to
   reduce block size? Proposal: keep `kzg_commitment` optional and gate via
   `iroha_config::da.enable_kzg`.
2. **Sequence gaps** — Do we allow out-of-order lanes? Current plan rejects gaps
   unless governance toggles `allow_sequence_skips` for emergency replay.
3. **Light-client cache** — SDK team requested a lightweight SQLite cache for
   proofs; pending follow-up under DA-8.

Answering these in implementation PRs moves DA-3 from 🈸 (this document) to 🈺
once code work begins.
