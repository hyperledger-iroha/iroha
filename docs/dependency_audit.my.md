---
lang: my
direction: ltr
source: docs/dependency_audit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb0a770fac1086462d949dbf17dd5a05f133169e57d50b0d90ddb48ae05f2853
source_last_modified: "2026-01-05T09:28:11.822642+00:00"
translation_last_reviewed: 2026-02-07
---

//! Dependency Audit Summary

Date: 2025-09-01

Scope: Workspace-wide review of all crates declared in Cargo.toml files and resolved in Cargo.lock. Performed with cargo-audit against the RustSec advisory DB plus manual review for crate legitimacy and “main crate” choices for algorithms.

Tools/commands run:
- `cargo tree -d --workspace --locked --offline` – inspected duplicate versions
- `cargo audit` – scanned Cargo.lock for known vulnerabilities and yanked crates

Security advisories found (now 0 vulns; 2 warnings):
- crossbeam-channel — RUSTSEC-2025-0024
  - Fixed: bumped to `0.5.15` in `crates/ivm/Cargo.toml`.

  - Fixed: flipped `pprof` to `prost-codec` in `crates/iroha_torii/Cargo.toml`.

- ring — RUSTSEC-2025-0009
  - Fixed: bumped QUIC/TLS stack (`quinn 0.11`, `rustls 0.23`, `tokio-rustls 0.26`) and updated WS stack to `tungstenite/tokio-tungstenite 0.24`. Forced lock to `ring 0.17.12` via `cargo update -p ring --precise 0.17.12`.

Remaining advisories: none. Remaining warnings: `backoff` (unmaintained), `derivative` (unmaintained).

Legitimacy and “main crate” assessment (spotlight):
- Hashing: `sha2` (RustCrypto), `blake2` (RustCrypto), `tiny-keccak` (widely used) — canonical choices.
- AEAD/Symmetric: `aes-gcm`, `chacha20poly1305`, `aead` traits (RustCrypto) — canonical.
- Signatures/ECC: `ed25519-dalek`, `x25519-dalek` (dalek project), `k256` (RustCrypto), `secp256k1` (libsecp bindings) — all legitimate; prefer a single secp256k1 stack (`k256` for pure Rust or `secp256k1` for libsecp) to reduce surface area.
- BLS12-381/ZK: `blstrs`, `halo2_*` — widely used in production ZK ecosystems; legitimate.
- PQ: `pqcrypto-dilithium`, `pqcrypto-traits` — legit reference crates.
- TLS: `rustls`, `tokio-rustls`, `hyper-rustls` — canonical modern Rust TLS stack.
- Noise: `snow` — canonical implementation.
- Serialization: `parity-scale-codec` is canonical for SCALE. Serde has been removed from production dependencies across the workspace; Norito derives/writers cover every runtime path. Any residual Serde references live in historical documentation, guardrail scripts, or test-only allowlists.
- FFI/libs: `libsodium-sys-stable`, `openssl` — legitimate; prefer Rustls over OpenSSL in production paths (current code already does).

Recommendations:
- Address warnings:
  - Consider replacing `backoff` with `retry`/`futures-retry` or a local exponential backoff helper.
  - Replace `derivative` derives with manual impls or `derive_more` where applicable.
- Medium: unify on either `k256` or `secp256k1` where possible to reduce duplicate implementations (leave both only if genuinely required).
- Medium: review `poseidon-primitives 0.2.0` provenance for ZK usage; if feasible, consider aligning with an Arkworks/Halo2-native Poseidon implementation to minimize parallel ecosystems.

Notes:
- `cargo tree -d` shows expected duplicate major versions (`bitflags` 1/2, multiple `ring`), not by itself a security risk but increases build surface.
- No typosquat-like crates were observed; all names and sources resolve to well-known ecosystem crates or internal workspace members.
- Experimental: added `iroha_crypto` feature `bls-backend-blstrs` to begin migrating BLS to a blstrs‑only backend (removes dependence on arkworks when enabled). Default remains `w3f-bls` to avoid behavior/encoding changes. Alignment plan:
  - Add round-trip fixtures in `crates/iroha_crypto/tests/bls_backend_compat.rs` that derive keys once and assert equality across both backends, covering `SecretKey`, `PublicKey`, and signature aggregation.

Follow-ups (proposed work items):
- Keep the Serde guardrails in CI (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`) so new production usages cannot be introduced.

Testing performed for this audit:
- Ran `cargo audit` with the latest advisory DB; verified the four advisories and their dependency trees.
- Searched for direct dependency declarations of affected crates to pinpoint fix locations.
