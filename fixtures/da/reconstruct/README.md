# DA-6 Reconstruction Fixtures

This directory contains the canonical RS recovery fixture referenced by the
`da_reconstruct` harness, native proof generators, and the documentation in
`specs/da/ingest_plan.md`.
It is generated via the ignored test

```
cargo test --locked -p sorafs_car --features da_harness,dev-tools \
  --bin da_reconstruct regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
```

Artifacts:

- `manifest.norito.hex` / `manifest.json` — canonical `DaManifestV1` encoding and
  Norito JSON view. The manifest contains only contiguous data commitments that
  end at `total_size`; its erasure profile governs the external recovery shards.
- `chunk_matrix.json` — ordered data and recovery commitment metadata (index,
  offset, length, digest, parity flag). Recovery offsets are storage addresses,
  not payload ranges, and therefore do not appear in the manifest.
- `chunks/` — `chunk_{index:05}.bin` payload slices plus external parity recovery
  vectors.
- `payload.bin` — contiguous reconstructed payload used by round-trip tests.
- `commitment_bundle.{json,norito.hex}` — sample V1 Merkle commitment bundle
  for docs/tests.
