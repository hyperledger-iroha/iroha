---
lang: dz
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2025-12-29T18:16:35.946614+00:00"
translation_last_reviewed: 2026-02-07
---

//! Notes for the RustCrypto SM integration spike.

# RustCrypto SM Spike Notes

## Objective
Validate that introducing RustCrypto’s `sm2`, `sm3`, and `sm4` crates (plus `rfc6979`, `ccm`, `gcm`) as optional dependencies compiles cleanly in the `iroha_crypto` crate and yields acceptable build times before wiring the feature flag into the wider workspace.

## Proposed Dependency Map

| Crate | Suggested Version | Features | Notes |
|-------|-------------------|----------|-------|
| `sm2` | `0.13` (RustCrypto/signatures) | `std` | Depends on `elliptic-curve`; verify MSRV matches workspace. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/hashes) | default | API parallels `sha2`, integrates with existing `digest` traits. |
| `sm4` | `0.5.1` (RustCrypto/block-ciphers) | default | Works with cipher traits; AEAD wrappers deferred to later spike. |
| `rfc6979` | `0.4` | default | Reuse for deterministic nonce derivation. |

*Versions reflect current releases as of 2024-12; confirm with `cargo search` before landing.*

## Manifest Changes (draft)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

Follow-up: pin `elliptic-curve` to match versions already in `iroha_crypto` (currently `0.13.8`).

## Spike Checklist
- [x] Add optional dependencies and feature to `crates/iroha_crypto/Cargo.toml`.
- [x] Create `signature::sm` module behind `cfg(feature = "sm")` with placeholder structs to confirm wiring.
- [x] Run `cargo check -p iroha_crypto --features sm` to confirm compile; record build time and new dependency count (`cargo tree --features sm`).
- [x] Confirm the std-only posture with `cargo check -p iroha_crypto --features sm --locked`; `no_std` builds are no longer supported.
- [x] File results (timings, dependency tree delta) in `docs/source/crypto/sm_program.md`.

## Observations To Capture
- Additional compile time vs. baseline.
- Binary size impact (if measurable) with `cargo builtinsize`.
- Any MSRV or feature conflicts (e.g., with `elliptic-curve` minor versions).
- Warnings emitted (unsafe code, const-fn gating) that may require upstream patches.

## Pending Items
- Await Crypto WG approval before inflating workspace dependency graph.
- Confirm whether to vendor crates for review or rely on crates.io (mirrors may be required).
- Coordinate `Cargo.lock` refresh per `sm_lock_refresh_plan.md` before marking checklist complete.
- Use `scripts/sm_lock_refresh.sh` once approval is granted to regenerate the lockfile and dependency tree.

## 2025-01-19 Spike Log
- Added optional dependencies (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1`, `rfc6979 0.4`) and `sm` feature flag in `iroha_crypto`.
- Stubbed `signature::sm` module to exercise hashing/block cipher APIs during compilation.
- `cargo check -p iroha_crypto --features sm --locked` now resolves dependency graph but aborts with `Cargo.lock` update requirement; repository policy forbids lockfile edits, so the compile run remains pending until we coordinate an allowed lock refresh.

## 2026-02-12 Spike Log
- Resolved the previous lockfile blocker—the dependencies are already captured—so `cargo check -p iroha_crypto --features sm --locked` succeeds (cold build 7.9 s on dev Mac; incremental re-run 0.23 s).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` passes in 1.0 s, confirming the optional feature compiles in `std`-only configurations (no `no_std` path remains).
- Dependency delta with the `sm` feature enabled introduces 11 crates: `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4`, and `sm4-gcm`. (`rfc6979` was already part of the baseline graph.)
- Build warnings persist for unused NEON policy helpers; leave as-is until the metering smoothing runtime re-enables those code paths.
