# Sora Nexus Data Availability Commitments Plan (DA-3)

_Drafted: 2026-03-25 — Owners: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 extends the Nexus block format so every lane embeds deterministic records
describing the blobs accepted by DA-2. This note captures the canonical data
structures, block pipeline hooks, light-client proofs, and Torii/RPC surfaces
that must land before validators can rely on DA commitments during admission or
governance checks. All payloads are Norito-encoded; no retired codec or ad-hoc JSON.

## Objectives

- Carry per-blob Merkle commitments (chunk root + manifest hash) inside every
  Nexus block so peers can reconstruct availability state without consulting
  off-ledger storage.
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
    pub proof_scheme: DaProofScheme,          // V1 lane policy (merkle_sha256 only)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- V1 contains no KZG proof-scheme variant, commitment field, setup,
  commitment generation, or proof verification. Unknown future enum
  discriminants fail decoding, and configuration strings other than
  `merkle_sha256` are rejected before a node starts.
- `proof_scheme` is derived from the lane catalog and is
  `merkle_sha256` in V1. KZG can be introduced only by a separately reviewed
  protocol version with an explicit wire-layout change. In particular, hashes
  expanded to 48 bytes are not elliptic-curve commitments.
- `proof_digest` anticipates DA-5 PDP/PoTR integration so the same record
  enumerates the sampling schedule used to keep blobs live. Its nullable slot
  is nevertheless mandatory on the first-release JSON wire: producers emit an
  explicit value or `null`, never an omitted field. Commitment records and the
  header-hashed policy/commitment bundles reject unknown JSON fields.

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

For every non-empty V1 bundle, `da_commitments_hash` is a domain-separated
commitment to the tree descriptor `(version, leaf_count, merkle_root)`, not the
canonical Norito hash of the full bundle. The descriptor is covered by the
signed block header, while the full bundle remains an authenticated
`SignedBlockWire` sidecar. Empty bundles leave the field as `None`.

Implementation note: `BlockPayload` and the transparent `BlockBuilder` now expose
`da_commitments` setters/getters (see `BlockBuilder::set_da_commitments` and
`SignedBlock::set_da_commitments`), so hosts can attach a pre-built bundle
before sealing a block. All helper constructors default the field to `None`
until Torii threads real bundles through.

### 1.3 Wire encoding

- `SignedBlockWire::canonical_wire()` appends the Norito header for
  `DaCommitmentBundle` immediately after the existing transaction list. The
  version byte is `0x01`.
- Block admission rejects bundles whose `version` is unknown, matching the
  Norito policy described in `norito.md`.
- Builders derive the header descriptor from the exact sidecar before signing.
  Admission independently reconstructs it from the decoded sidecar and rejects
  a missing or mismatched body.

## 2. Block Production Flow

1. Torii DA ingest persists signed receipts and commitment records into the
   DA spool (`da-receipt-*.norito` / `da-commitment-*.norito`). The durable
   receipt log seeds cursors on restart so replayed receipts are still ordered
   deterministically. Receipt recovery streams the directory one artifact at a
   time, retains only one compact high-water record per bounded `(lane, epoch)`
   window, and proves sequence coverage with a constant-size summary. Historical
   duplicate acknowledgements load their deterministic receipt path directly;
   they are never kept as an unbounded in-memory receipt map. V1 rejects a
   receipt above 64 KiB, a manifest above 2 MiB, or a PDP commitment above its
   canonical 16 KiB limit before decoding the artifact.
2. Block assembly loads receipts from the spool, drops stale/already-sealed
   entries using the committed cursor snapshot, and enforces contiguity per
   `(lane, epoch)`. If a reachable receipt lacks a matching commitment or the
   manifest hash diverges the proposal aborts instead of silently omitting it.
3. Right before sealing, the builder slices the commitment bundle to the
   receipt-driven set, sorts by `(lane_id, epoch, sequence)`, constructs its
   versioned `(version, leaf_count, merkle_root)` descriptor, and writes that
   commitment to `da_commitments_hash`.
4. The full bundle is stored in the WSV and emitted alongside the block inside
   `SignedBlockWire`; committed bundles advance the receipt cursors (hydrated
   from Kura on restart) and prune stale spool entries to bound disk growth.

Block assembly and canonical `SignedBlockWire` proposal ingestion re-validate
each commitment against the lane catalog: V1 admits only Merkle records, requires a non-zero
`chunk_root`, and rejects unknown lanes. Because the data model has no KZG
variant or field, neither Torii nor a lifecycle transition can construct or
sign a KZG policy/record; an unknown wire discriminant fails decoding.
`/v1/da/commitments/verify` applies the same V1 policy to historical proofs.

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
| `/v1/da/commitments` | `POST` | `DaCommitmentProofRequest` (optional manifest/lane/epoch/sequence filters and pagination). | Returns located records plus the node's active policy snapshot for discovery. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest`. | Responds with the Merkle proof and the proof-policy sidecar committed by the referenced block. |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Loads the exact Kura block and validates the proof against its canonical committed block header and committed policy sidecar, independent of later lane-policy changes. It does not independently verify block signatures or finality. |

All payloads live under `iroha_data_model::da::commitment`. Torii routers mount
the handlers next to the existing DA ingest endpoints to reuse token/mTLS
policies.

## 4. Inclusion Proofs & Light Clients

- The block producer builds a binary Merkle tree over the serialized
  `DaCommitmentRecord` list. V1 separates the two hash domains:
  leaves hash `b"iroha:da:commitment-merkle:leaf:v1\0" || norito(record)`,
  while internal nodes hash
  `b"iroha:da:commitment-merkle:internal:v1\0" || left || right`.
  Odd nodes are promoted unchanged. `da_commitments_hash` commits the domain,
  V1 bundle version, leaf count, and Merkle root, so a logarithmic proof can
  reconstruct the exact value committed by the block header without the
  complete bundle.
- `DaCommitmentProof` packages the target record plus a vector of `(sibling_hash,
  position)` entries, the leaf count, tree root, and referenced block height.
  A verifier supplies a caller-authenticated canonical block header and the
  policy sidecar whose hash that header commits; the helper verifies neither
  block signatures nor finality and does not consult mutable current Nexus policy.
- Pin-intent `prove` and `verify` use the same authenticated shape rather than
  treating equality with a node's current index as a proof. `DaPinIntentProof`
  binds the intent, block height, bundle index and length, the versioned
  pin-intent tree descriptor from the block header, and a Merkle path. Its leaf
  and internal-node domains are respectively
  `iroha:da:pin-intent-merkle:leaf:v1\0` and
  `iroha:da:pin-intent-merkle:internal:v1\0`.
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

1. **Future KZG protocol** — KZG is not part of V1. A separately reviewed later
   protocol version must specify a real polynomial encoding, trusted/setup
   provenance, commitment and opening algorithms, consensus verification, test
   vectors, hardware-deterministic behavior, and a versioned wire-layout change.
   There is no V1 enable toggle or reserved accepted value.
2. **Sequence gaps** — Do we allow out-of-order lanes? Current plan rejects gaps
   unless governance toggles `allow_sequence_skips` for emergency replay.
3. **Light-client cache** — SDK team requested a lightweight SQLite cache for
   proofs; pending follow-up under DA-8.

Answering these in implementation PRs moves DA-3 from 🈸 (this document) to 🈺
once code work begins.
