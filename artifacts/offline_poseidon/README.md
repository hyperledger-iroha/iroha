# Offline Poseidon Snapshot

This directory captures the canonical Poseidon2 parameters used by the offline
receipt tree (OA13) and the future aggregate-proof circuits (OA14).

## Files

| File | Description |
|------|-------------|
| `constants.ron` | Auditable snapshot of the field modulus, round constants, and MDS matrix. Mirrors `crates/fastpq_isi/src/poseidon.rs` and SHOULD be updated only when the canonical Poseidon profile changes. |
| `vectors.json` | Golden receipt-leaf fixtures (tx id, counter, hashed fields) plus the resulting Poseidon roots. SDKs/operators can cross-check their builders against these cases. |

## Regenerating the snapshot

```bash
cargo xtask offline-poseidon-fixtures
```

Use optional flags to override the default output paths or domain tag:

```
cargo xtask offline-poseidon-fixtures \
  --constants /tmp/constants.ron \
  --vectors /tmp/vectors.json \
  --tag custom.domain.tag
```

By default the command also mirrors the generated files into the SDK fixtures at
`IrohaSwift/Fixtures/offline_poseidon/` and
`java/iroha_android/src/test/resources/offline_poseidon/` so wallet teams can run
their fuzzers without shelling out to Rust. Pass `--no-sdk-mirror` if you only
need the standalone artifacts.

Internally this command mirrors the canonical FASTPQ Poseidon parameters from
`fastpq_isi`, formats them as the auditable RON snapshot, and recomputes the leaf
fixtures directly from the data-model helpers so hashes align with `compute_receipts_root`.

To extend `vectors.json`, create deterministic receipts/leaves in `crates/iroha_data_model`
tests (see `OfflineReceiptLeaf`) and append their hashes/root to the fixture file. Future
automation will move this flow into `cargo xtask offline-bundle-fixtures`, but the format is
stable and human-editable today.
