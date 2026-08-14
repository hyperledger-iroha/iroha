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

Regenerate the artifacts through a private external stage with an explicitly
authorized signing-key file:

```bash
sf1_stage="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sf1.XXXXXX")"
sf1_stage="$(cd -- "$sf1_stage" && pwd -P)"
chmod 700 "$sf1_stage"
cargo run --locked -p sorafs_chunker --features dev-tools --bin export_vectors -- \
  --write --staging-root "$sf1_stage" \
  --signing-key-file /absolute/private/council-signing-key.hex
```

The generator has no unsigned mode. It validates the complete staged tree and
signature before publishing any checked-in path, and a changed manifest digest
requires explicit signing authority. Use `--check` with a fresh private stage
for a read-only replay. The canonical signature set comes from Parliament
events fetched by the tooling helpers described in
`specs/sorafs/signing_ceremony.md`.
