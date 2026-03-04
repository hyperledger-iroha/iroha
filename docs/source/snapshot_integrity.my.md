---
lang: my
direction: ltr
source: docs/source/snapshot_integrity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7886c43b5b8a5afcbdf9a39e4e89959c172d7265fdb34f9d955a95cf57d2f82a
source_last_modified: "2026-01-05T09:28:12.065907+00:00"
translation_last_reviewed: 2026-02-07
---

# Snapshot Integrity

Iroha nodes now emit signed, Merkle-attested snapshots and refuse to restore
from sidecars that fail integrity checks. Snapshot creation writes four files
under `snapshot.store_dir`:

- `snapshot.data` — Norito JSON dump of the world state
- `snapshot.sha256` — SHA-256 digest of `snapshot.data`
- `snapshot.sig` — signature of the digest using the node identity key
- `snapshot.merkle.json` — Norito JSON metadata with `chunk_size_bytes`,
  `total_len_bytes`, `root_hex`, and per-chunk SHA-256 leaf hashes
- Snapshot writers use temp files, fsync payloads, then rename and fsync the
  snapshot directory so the bundle is crash-safe. Snapshot readers will accept
  the `.tmp` bundle when the final files are missing and promote the temp files
  after successful validation.

## Configuration

- `snapshot.merkle_chunk_size_bytes` controls the chunk size used to derive the
  Merkle tree (default: 1 MiB). Change it only when generating a fresh snapshot,
  since restores validate that metadata and runtime expectations match.
- Snapshot writing is enabled when `snapshot.mode = "read_write"`. Signature
  sidecars always use the node identity key.

## Restore and gating

- `irohad` now fails startup if any sidecar is missing or mismatched. Restore
  verifies the digest, the signature over the digest, the Merkle root over the
  chunked payload, chunk size/length parity, leaf counts, and metadata
  self-consistency, then confirms the snapshot `chain_id` matches the
  configured chain before deserializing the world state. Only the `NotFound`
  case falls back to a fresh genesis state.
- Error surfaces map to dedicated `TryReadError` variants (`ChecksumMissing`,
  `SignatureMissing`, `MerkleMissing`, `MerkleMismatch`,
  `MerkleChunkSizeMismatch`, `MerkleLengthMismatch`, `MerkleProofInvalid`,
  `ChainIdMismatch`), keeping CI/startup logs explicit about the failure mode.

## Chunk proofs and provenance

- `snapshot.merkle.json` carries per-chunk digests so chunk proofs can be
  reconstructed (`SnapshotMerkleMetadata::verify_chunk`) for streaming/remote
  restore flows. The recorded `root_hex` is suitable for on-chain provenance or
  governance packets when operators need to pin a specific snapshot payload.
