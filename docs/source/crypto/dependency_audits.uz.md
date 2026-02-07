---
lang: uz
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
---

# Crypto Dependency Audits

## Streebog (`streebog` crate)

- **Version in tree:** `0.11.0-rc.2` vendored under `vendor/streebog` (used when the `gost` feature is enabled).
- **Consumer:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + message hashing).
- **Status:** Release-candidate only. No non-RC crate currently offers the required API surface,
  so we mirror the crate in-tree for auditability while we track upstream for a final release.
- **Review checkpoints:**
  - Verified hash output against the Wycheproof suite and TC26 fixtures via
    `cargo test -p iroha_crypto --features gost` (see `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    exercises Ed25519/Secp256k1 alongside every TC26 curve with the current dependency.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    compares the fresher measurements against the checked-in medians (use `--summary-only` in CI, add
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` when rebaselining).
  - `scripts/gost_bench.sh` wraps the bench + check flow; pass `--write-baseline` to update the JSON.
    See `docs/source/crypto/gost_performance.md` for the end-to-end workflow.
- **Mitigations:** `streebog` is only ever invoked through deterministic wrappers that zeroise keys;
  the signer hedges nonces with OS entropy to avoid catastrophic RNG failure.
- **Next actions:** Follow RustCrypto’s streebog `0.11.x` release; once the tag lands, treat the
  upgrade as a standard dependency bump (verify checksum, review the diff, record provenance, and
  drop the vendored mirror).
