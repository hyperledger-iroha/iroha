# Snapshot Integrity

Iroha nodes now emit signed, Merkle-attested snapshots and refuse to restore
from sidecars that fail integrity checks. Snapshot creation writes four files
under `snapshot.store_dir`:

- `snapshot.data` — Norito JSON dump of the world state
- `snapshot.sha256` — SHA-256 digest of `snapshot.data`
- `snapshot.sig` — signature of the digest using the node identity key
- `snapshot.merkle.json` — Norito JSON metadata with `chunk_size_bytes`,
  `total_len_bytes`, `root_hex`, and per-chunk SHA-256 leaf hashes

## Configuration

- `snapshot.merkle_chunk_size_bytes` controls the chunk size used to derive the
  Merkle tree (default: 1 MiB). Change it only when generating a fresh snapshot,
  since restores validate that metadata and runtime expectations match.
- Snapshot writing is enabled when `snapshot.mode = "read_write"`. Signature
  sidecars always use the node identity key.

## Restore and gating

- `irohad` now fails startup if any sidecar is missing or mismatched. Restore
  verifies the digest, the signature over the digest, the Merkle root over the
  chunked payload, chunk size/length parity, and metadata self-consistency,
  then confirms the snapshot `chain_id` matches the configured chain before
  deserializing the world state. Only the `NotFound` case falls back to a fresh
  genesis state.
- Error surfaces map to dedicated `TryReadError` variants (`ChecksumMissing`,
  `SignatureMissing`, `MerkleMissing`, `MerkleMismatch`,
  `MerkleChunkSizeMismatch`, `MerkleLengthMismatch`, `MerkleProofInvalid`,
  `ChainIdMismatch`), keeping CI/startup logs explicit about the failure mode.

## Chunk proofs and provenance

- `snapshot.merkle.json` carries per-chunk digests so chunk proofs can be
  reconstructed (`SnapshotMerkleMetadata::verify_chunk`) for streaming/remote
  restore flows. The recorded `root_hex` is suitable for on-chain provenance or
  governance packets when operators need to pin a specific snapshot payload.
