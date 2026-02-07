---
lang: hy
direction: ltr
source: docs/source/da/commitments_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea1b16b73a55e3e47dfe9d5bfc77dedce2e8fa9ff964d244856767f14931733
source_last_modified: "2026-01-22T14:45:02.095688+00:00"
translation_last_reviewed: 2026-02-07
---

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
  `iroha_crypto::kzg`. Merkle lanes leave it empty; `kzg_bls12_381` lanes now
  receive a deterministic BLAKE3-XOF commitment derived from the chunk root and
  storage ticket so block hashes stay stable without an external prover.
- `proof_scheme` is derived from the lane catalog; Merkle lanes reject stray KZG
  payloads while `kzg_bls12_381` lanes require non-zero KZG commitments.
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

1. Torii DA ingest persists signed receipts and commitment records into the
   DA spool (`da-receipt-*.norito` / `da-commitment-*.norito`). The durable
   receipt log seeds cursors on restart so replayed receipts are still ordered
   deterministically.
2. Block assembly loads receipts from the spool, drops stale/already-sealed
   entries using the committed cursor snapshot, and enforces contiguity per
   `(lane, epoch)`. If a reachable receipt lacks a matching commitment or the
   manifest hash diverges the proposal aborts instead of silently omitting it.
3. Right before sealing, the builder slices the commitment bundle to the
   receipt-driven set, sorts by `(lane_id, epoch, sequence)`, encodes the
   bundle with the Norito codec, and updates `da_commitments_hash`.
4. The full bundle is stored in the WSV and emitted alongside the block inside
   `SignedBlockWire`; committed bundles advance the receipt cursors (hydrated
   from Kura on restart) and prune stale spool entries to bound disk growth.

Block assembly and `BlockCreated` ingestion re-validate each commitment against
the lane catalog: Merkle lanes reject stray KZG commitments, KZG lanes require a
non-zero KZG commitment and non-zero `chunk_root`, and unknown lanes are
dropped. Torii’s `/v1/da/commitments/verify` endpoint mirrors the same guard,
and ingest now threads the deterministic KZG commitment into every
`kzg_bls12_381` record so policy-compliant bundles reach block assembly.

The manifest fixtures described in the DA-2 ingest plan double as the source of
truth for the commitment bundler. The Torii test
`manifest_fixtures_cover_all_blob_classes` regenerates manifests for every
`BlobClass` variant and refuses to compile until new classes gain fixtures,
ensuring the encoded manifest hash inside each `DaCommitmentRecord` matches the
golden Norito/JSON pair.【crates/iroha_torii/src/da/tests.rs:2902】

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
   bundle bytes and Merkle proofs. Each bundle references the manifest bytes
   from `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}`, so
   regenerating `cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture`
   keeps the Norito story consistent before `ci/check_da_commitments.sh` refreshes the commitment
   proofs.【fixtures/da/ingest/README.md:1】
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
