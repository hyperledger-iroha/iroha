//! Dependency Audit Summary

Date: 2026-08-22

Scope: Workspace-wide review of all crates declared in Cargo.toml files and resolved in Cargo.lock. Performed with cargo-audit against the RustSec advisory DB plus manual review for crate legitimacy and “main crate” choices for algorithms.

Tools/commands run:
- `cargo tree -d --workspace --locked --offline` – inspected duplicate versions
- `cargo audit` – scanned Cargo.lock for known vulnerabilities and yanked crates

Current incident-response status:
- The root lockfile was checked against the Rust Security Response Team's
  2026-08-20 `arrayref` incident list. It retains the legitimate
  `arrayref 0.3.9`, now exact-pinned in `iroha_crypto`, and does not contain the
  named malicious releases or `proc-macro1` family.
- Remediations are applied in the working tree for `quinn-proto 0.11.15`,
  `h2 0.4.16`, `crossbeam-epoch 0.9.20`, `webbrowser 1.2.2`, and
  `zbus_xml 5.2.1`; the vulnerable `quick-xml 0.39.4` resolution and Norito's
  unused `rkyv 0.8` dependency were pruned.
- The lockfile also advances `event-listener` to `5.4.2` and the lock-only
  `lru 0.18` resolution to `0.18.2`, addressing their compatible unsoundness
  fixes.
- A fresh scan against RustSec advisory database commit
  `bf5c0d245a92671908518d7e765914d437954ed6` reports no actionable active-tree
  vulnerability after excluding `RUSTSEC-2026-0235`. The raw lockfile scan
  still reports that advisory for `rkyv 0.7.46`, which is metadata for
  `rust_decimal`'s disabled optional `rkyv` feature; an all-features/all-targets
  inverse feature tree has no path to it. Twelve unmaintained, two conditional
  `lru`, and two yanked-package warnings remain as separately tracked debt.

Historical 2025 advisory remediation:
- crossbeam-channel — RUSTSEC-2025-0024
  - Fixed: bumped to `0.5.15` in `crates/ivm/Cargo.toml`.

  - Fixed: flipped `pprof` to `prost-codec` in `crates/iroha_torii/Cargo.toml`.

- ring — RUSTSEC-2025-0009
  - Fixed: bumped QUIC/TLS stack (`quinn 0.11`, `rustls 0.23`, `tokio-rustls 0.26`) and updated WS stack to `tungstenite/tokio-tungstenite 0.24`. Forced lock to `ring 0.17.12` via `cargo update -p ring --precise 0.17.12`.

At the time of the 2025 audit, remaining advisories were none and the remaining
warnings were `backoff` (unmaintained) and `derivative` (unmaintained).

Legitimacy and “main crate” assessment (spotlight):
- Hashing: `sha2` (RustCrypto), `blake2` (RustCrypto), `tiny-keccak` (widely used) — canonical choices.
- AEAD/Symmetric: `aes-gcm`, `chacha20poly1305`, `aead` traits (RustCrypto) — canonical.
- Signatures/ECC: `ed25519-dalek`, `x25519-dalek` (dalek project), `k256` (RustCrypto), `secp256k1` (libsecp bindings) — all legitimate; prefer a single secp256k1 stack (`k256` for pure Rust or `secp256k1` for libsecp) to reduce surface area.
- BLS12-381/ZK: `blstrs`, `halo2_*` — widely used in production ZK ecosystems; legitimate.
- PQ: `pqcrypto-mldsa`, `pqcrypto-mlkem`, `pqcrypto-traits` — legit reference crates.
- TLS: `rustls`, `tokio-rustls`, `hyper-rustls` — canonical modern Rust TLS stack.
- Noise: `snow` — canonical implementation.
- Serialization: Norito is the canonical workspace codec. Serde has been removed from production dependencies across the workspace; Norito derives/writers cover every runtime path. Any residual Serde references live in historical documentation, guardrail scripts, or test-only allowlists.
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
- BLS signatures use one `w3f-bls` implementation. The experimental
  `bls-backend-blstrs` adapter was removed because it duplicated the public-key
  and secret-key surface while delegating signing, verification, and key
  derivation back to `w3f-bls`. The `blstrs` dependency remains in use by the
  threshold-BLS and timed-OVN protocols for direct curve arithmetic.

Follow-ups (proposed work items):
- Keep the Serde guardrails in CI (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`) so new production usages cannot be introduced.

Historical testing performed for the 2025 audit:
- Ran `cargo audit` with the latest advisory DB; verified the four advisories and their dependency trees.
- Searched for direct dependency declarations of affected crates to pinpoint fix locations.

Current validation status:
- `cargo metadata --locked --format-version 1 --no-deps` passed.
- `cargo iroha-fast -- check -p norito --lib --locked` passed.
- `cargo iroha-fast -- check -p soranet-relay -p sora-vpn-helper --locked`
  passed with `quinn-proto 0.11.15`.
- `cargo iroha-fast -- check -p iroha_torii --lib --locked` compiled the
  patched `h2 0.4.16` dependency, then stopped on unrelated concurrent
  `iroha_core` source errors in `block.rs` and `state.rs`.
- `cargo audit --ignore RUSTSEC-2026-0235` passed against the database commit
  above; the unfiltered result is the inactive optional-feature finding
  documented above.
