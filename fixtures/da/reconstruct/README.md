# DA-6 Reconstruction Fixtures

This directory contains the canonical RS parity fixture referenced by the
`da_reconstruct` harness and the documentation in `specs/da/ingest_plan.md`.
It is generated via the ignored test

```
cargo test -p sorafs_car --features 'dev-tools,da_harness' --bin da_reconstruct regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
```

Artifacts:

- `manifest.norito.hex` / `manifest.json` — canonical `DaManifestV1` encoding and
  Norito JSON view.
- `chunk_matrix.json` — producer-ordered commitment metadata (each stripe's
  data followed by global parity; index, offset, length, digest, parity flag).
- `chunks/` — `chunk_{index:05}.bin` payload slices (data + parity).
- `payload.bin` — contiguous reconstructed payload used by round-trip tests.
- `commitment_bundle.{json,norito.hex}` — sample V1 Merkle commitment bundle
  for docs/tests.
