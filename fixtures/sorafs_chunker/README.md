This directory hosts the canonical SoraFS chunker fixtures referenced by
cross-language implementations (Rust/Go/TypeScript) and integration pipelines.

- `sf1_profile_v1.json` — Norito-friendly JSON snapshot of the SF1 profile,
  including PRNG parameters, chunk boundaries, and BLAKE3 digests.
- `sf1_profile_v1.rs` — Rust constants mirroring the JSON payload for unit test
  embeds or offline verification.
- `sf1_profile_v1.ts` — TypeScript fixture for SDK parity checks.
- `sf1_profile_v1.go` — Go fixture for gateway/storage agents.
- `manifest_blake3.json` — BLAKE3 digest summary for the generated artifacts.
- `manifest_signatures.json` — Sora Parliament approval envelope for the
  `manifest_blake3.json` payload. The Infrastructure Panel publishes the
  canonical digest on-chain; this file mirrors the latest on-chain commitment for
  local regression tests.

Regenerate the artifacts with:

```
cargo run -p sorafs_chunker --bin export_vectors
```

Local runs typically pass `--allow-unsigned`; the canonical signature set comes
from Parliament events fetched by the tooling helpers described in
`docs/source/sorafs/signing_ceremony.md`. After an approval lands on-chain,
rerun the generator to refresh the language fixtures and confirm that the
BLAKE3 digest matches the Nexus record.
