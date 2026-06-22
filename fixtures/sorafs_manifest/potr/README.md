# SoraFS PoTR Fixture

This directory contains deterministic proof-of-timed-retrieval fixture payloads
for the SF-11 reference validator.

- `receipt_v1.to` is the canonical Norito `PotrReceiptV1` payload.
- `receipt_v1.json` is a readable summary of the same fixture.

Regenerate both files with:

```sh
cargo run -p sorafs_manifest --bin generate_por_fixtures
```

