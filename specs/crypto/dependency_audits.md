# Crypto Dependency Audits

## Streebog (`streebog` crate)

- **Version in tree:** `0.11.0` from crates.io, pinned by checksum in `Cargo.lock` (used when the
  `gost` feature is enabled). The workspace carries no vendored mirror or `[patch]` override.
- **Consumer:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + message hashing).
- **Status:** Stable upstream release. The former in-tree `0.11.0-rc.2` mirror was removed once
  the stable `0.11.0` release shipped the required API surface.
- **Review checkpoints:**
  - Verified hash output against the Wycheproof suite and TC26 fixtures via
    `cargo test -p iroha_crypto --features gost` (see `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    exercises Ed25519/Secp256k1 alongside every TC26 curve with the current dependency.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost,dev-tools`
    compares the fresher measurements against the checked-in medians (use `--summary-only` in CI, add
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` when rebaselining).
  - `scripts/gost_bench.sh` wraps the bench + check flow; pass `--write-baseline` to update the JSON.
    See `specs/crypto/gost_performance.md` for the end-to-end workflow.
- **Mitigations:** `streebog` is only ever invoked through deterministic wrappers that zeroise keys;
  the signer hedges nonces with OS entropy to avoid catastrophic RNG failure.
- **Next actions:** Treat later `0.11.x` releases as standard dependency bumps: verify the
  checksum, review the upstream diff, and record provenance.
