Predecoder Golden Vectors
=========================

This directory hosts golden vectors for the IVM opcode pre-decoder.

Generate/refresh fixtures with:

  cargo run --locked -p ivm --features dev-tools --bin ivm_fixture_export -- --write

Check the tracked fixtures without changing them with:

  cargo run --locked -p ivm --features dev-tools --bin ivm_fixture_export -- --check

This produces the following under `mixed/`:

- `code.bin`       — raw canonical 32-bit instruction words
- `decoded.json`   — decoded op list: [{ pc, len, inst, inst_hex }]
- `index.json`     — artifact manifest with metadata
- `artifacts/*.to` — header + code artifacts for selected metadata variants

Cross-implementation consumers can parse `decoded.json` to validate canonical
fixed-width decoding, and can load `artifacts/*.to` to validate header parsing invariants.
