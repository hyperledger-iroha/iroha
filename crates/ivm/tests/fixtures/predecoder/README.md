Predecoder Golden Vectors
=========================

This directory hosts golden vectors for the IVM opcode pre-decoder.

Generate/refresh fixtures with:

  cargo run -p ivm --bin ivm_predecoder_export

This produces the following under `mixed/`:

- `code.bin`       — raw instruction bytes (mixed 16/32-bit)
- `decoded.json`   — decoded op list: [{ pc, len, inst, inst_hex }]
- `index.json`     — artifact manifest with metadata
- `artifacts/*.to` — header + code artifacts for selected metadata variants

Cross-implementation consumers can parse `decoded.json` to validate canonical
decoding, and can load `artifacts/*.to` to validate header parsing invariants.

