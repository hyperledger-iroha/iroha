# GOST Backend Hardening Plan

This document tracks the migration work needed to take the TC26 GOST R 34.10-2012
support in `iroha_crypto` from the current `num-bigint` prototype to a
production-grade, constant-time implementation. It mirrors the `GOST Production
Hardening` roadmap line items and will be updated as decisions land.

## Objectives

- Eliminate secret-dependent branches, heap allocations, and timing variation
  from signing and verification paths.
- Adopt a deterministic nonce scheme equivalent to RFC 6979 while using the
  Streebog hash family with an explicit domain separator.
- Ensure scalar sampling is uniform in `[1, q-1]` for all TC26 parameter sets.
- Pin audited dependencies (or vendor code) so the signature stack does not
  depend on release-candidate crates.
- Provide benchmark and timing harnesses that prove the implementation is both
  secure and performant relative to our Ed25519 and Secp256k1 backends.

## Current Status (Oct 2025)

- ✅ **Task G2 (constant-time backend) is merged:** the Montgomery field
  arithmetic + Jacobian ops in `constant_time` now serve every production path
  (sign, verify, derive, keygen). The compat `num-bigint` code only survives in
  regression helpers.
- ✅ **Task G3 (deterministic nonce) is wired through signing:** HMAC-Streebog
  DRBG with an explicit domain tag now derives `k`; callers can provide hedged
  entropy and the default API pulls fresh OS randomness. Tests can disable the
  entropy to assert determinism.
- ✅ **Wycheproof + TC26 fixtures imported:** `crates/iroha_crypto/tests/gost_wycheproof.rs`
  covers all supported parameter sets; we also added regression tests for hedged
  entropy behaviour.
- 🚧 **Still pending:** dudect/criterion harnesses (timing + throughput), the
  Streebog dependency audit (G4), and hardware-acceleration/FFI decisions (G5).

### Benchmark & Timing Harnesses

- Criterion benches in `crates/iroha_crypto/benches/gost_sign.rs` now cover every TC26
  parameter set. Run them with
  `cargo bench -p iroha_crypto --bench gost_sign --features gost`. The harness runs
  Ed25519, Secp256k1, and each TC26 curve with a 60-sample / 6 s window so slower GOST
  variants complete without warnings.
- Latest run on an Apple M2 Pro (big cores @ 3.5 GHz) produced the following sign + verify medians:
  Reference medians live in `crates/iroha_crypto/benches/gost_perf_baseline.json`. Latest run on an Apple M2 Pro (big cores @ 3.5 GHz) produced the following sign + verify medians:

  | Algorithm                      | Time (microseconds) | Relative to Ed25519 |
  |--------------------------------|---------------------|----------------------|
  | Ed25519                        |               69.6  | 1.0x                 |
  | Secp256k1                      |              160.3  | 2.3x                 |
  | GOST 256 ParamSet A            |            1158.3   | 16.6x                |
  | GOST 256 ParamSet B            |            1148.4   | 16.5x                |
  | GOST 256 ParamSet C            |            1151.7   | 16.6x                |
  | GOST 512 ParamSet A            |            9104.6   | 131x                 |
  | GOST 512 ParamSet B            |            8988.5   | 129x                 |
- Dudect-style timing guard `gost_sign_constant_time_under_dudect` (unit test) samples every TC26
  curve and fails when the Welch t-statistic exceeds 5. Run with
  `cargo test -p iroha_crypto --features gost gost_sign_constant_time_under_dudect`
  (~27 s on reference hardware).
- After running the Criterion suite, validate the results against the recorded medians with
  `cargo run -p iroha_crypto --bin gost_perf_check --features gost`. Use `--tolerance` (fractional)
  to relax the 20% default; pass `--summary-only` in CI so the Markdown table lands in
  `$GITHUB_STEP_SUMMARY`. When rebaselining deliberately, run the checker with
  `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` after benchmarking to
  snapshot the new medians.
  The workflow `.github/workflows/gost-perf.yml` automates this sequence in CI.
- For a one-liner locally, run `scripts/gost_bench.sh`. Add `--write-baseline` to that script to
  refresh the checked-in medians after reviewing the results.
- `make gost-bench` wraps the script (pass `GOST_BENCH_ARGS="--tolerance 0.25"` etc.); use
  `make gost-bench-update` to rebaseline (`scripts/update_gost_baseline.sh` is a convenience helper).
  `make gost-dudect` runs the constant-time timing guard in isolation. The full workflow is captured in
  `docs/source/crypto/gost_performance.md`.
- Streebog (`0.11.0-rc.2`) is mirrored under `vendor/streebog` and patched into the workspace so the
  build no longer depends on crates.io for the hash implementation. Update the mirror when RustCrypto
  publishes a stable `0.11.x` release.
- Next steps: integrate dudect measurements alongside the Criterion harness and
  gate CI on acceptable variance once the timing scripts stabilize.

## Operator Guidance (Task G3)

- Build all binaries that need to handle TC26 keys (`irohad`, `iroha_cli`, tooling such as
  `kagami`) with `--features gost` so the algorithm IDs and parsing helpers are available
  end to end.
- Provision validator keys with `kagami crypto generate --algorithm gost3410-2012-*-paramset-*`.
  Use the `--seed` flag (32 bytes for 256-bit curves, 64 bytes for 512-bit curves) when you
  need deterministic regeneration; otherwise rely on the default OS randomness.
- When wiring the keys into genesis manifests or `iroha_config`, encode them as multihash
  strings (see `docs/genesis.md` for examples such as `gost3410-2012-256-paramset-a:<hex>`).
  Downstream services such as Torii and the CLI pick the algorithm from the multihash prefix.
- Keep `/dev/urandom` (or the platform equivalent) available on validator hosts. The signer
  hedges the deterministic HMAC-Streebog nonce with `scalar_len` bytes from `OsRng`; if the
  OS RNG fails the call aborts, so treated as a host readiness problem. Integration tests can
  reproduce deterministic output by calling the internal `sign_impl(.., extra_entropy=None)`
  helper the Wycheproof suite already uses.

## Backend Evaluation (Task G1)

### Option A — RustCrypto workspace

- **Status:** As of Oct 19, 2025 there is **no RustCrypto-maintained GOST R 34.10-2012**
  signature crate (sometimes referred to informally as “`gostdsa`”). The `signatures`
  workspace exposes DSA/ECDSA/Ed25519, while `hashes` does provide Streebog, but there
  is no TC26 signature implementation to adopt.
- **Implication:** We cannot rely on RustCrypto for an off-the-shelf backend today.
  Shipping production support therefore requires either authoring an in-tree constant-time
  implementation or adopting/vetting third-party code outside the RustCrypto umbrella.
+ **Next steps for this option:** Track upstream discussions; if RustCrypto ever publishes
  a maintained crate we can reassess, but for now this path is blocked.

### Option B — FFI-backed implementation

- **Candidates:** OpenSSL 3.x GOST provider, Botan 3.x (C++), CryptoPro’s C library,
  or `bee2` (Belarusian Academy of Sciences). We need TC26 certification or equivalent
  assurance plus permissive licensing (Apache/MIT/BSD; GPL is unsuitable).
- **Pros:** Mature, production-tested code paths; some have hardware acceleration hooks.
- **Cons:** FFI surface widens the attack surface, increases build complexity, and may
  require dynamic loading to respect licensing. Need to enforce constant-time behaviour
  across the boundary and audit pointer ownership carefully.
- **Questions:**
  1. Neither OpenSSL’s GOST provider nor Botan currently exposes deterministic nonce
     control for GOST 34.10-2012; both rely on secure randomness for `k`.
  2. OpenSSL GOST provider is Apache-2.0 and packaged broadly; Botan is Simplified BSD.
     CryptoPro is proprietary and therefore excluded.
  3. Streebog integration is available in both OpenSSL and Botan, so digest interop is OK.
  4. Deterministic behaviour for consensus would require either wrapping their RNG with a
     deterministic DRBG seed (if configurable) or layering our own nonce derivation, which
     neither exposes directly.

> Captured findings (Oct 2025):
> - OpenSSL GOST provider: production-ready, Apache-2.0, random nonces only.
> - Botan: production-ready, Simplified BSD, random nonces only.
> - CryptoPro: incompatible licensing.

Given the nonce limitation and our desire for deterministic, constant-time behaviour,
**shipping an FFI fallback would still require wrapping or forking to expose nonce control**.
This increases maintenance burden and may still fall short of constant-time guarantees unless
we audit/replicate the internals.

### Decision Artefact

- **Interim conclusion:** With no RustCrypto backend available and FFI options lacking
  deterministic nonce hooks, we need to pursue an **in-tree constant-time implementation**
  (potentially leveraging `crypto-bigint`/`elliptic-curve` building blocks) while keeping
  an FFI fallback as a last resort for tooling.
- **Next actions:**
  - Draft an implementation plan for a constant-time Rust backend (target Task G2).
  - Specify nonce derivation API and deterministic DRBG requirements (Task G3).
  - Keep the OpenSSL/Botan assessment handy should we need a temporary interoperability
    bridge for tooling.
- Once the in-tree plan is drafted, add migration checklist, owners, and acceptance tests
  to this document.

## Immediate Action Items

- ✅ Import Wycheproof/TC26 fixture coverage for all supported parameter sets (`crates/iroha_crypto/tests/gost_wycheproof.rs`)
  and extend the nonce tests to exercise hedged entropy permutations.
- ✅ Stood up dudect timing harnesses plus Criterion benches to quantify the new
  backend (links directly to roadmap items G3/G5). The helper also exports the table for
  audit logs.
- Complete the Streebog dependency audit/vendoring decision (roadmap item G4)
  and document operator guidance for hedged nonce seeding/fallback scenarios.

## In-Tree Constant-Time Backend Plan (Task G2)

> Status (Oct 2025): The steps below are implemented in the `constant_time`
> module. We keep the blueprint here as reference while validation/audit work
> proceeds.

1. **Field Arithmetic**
   - Use `crypto_bigint::Uint<256>` / `Uint<512>` with Montgomery reduction for the
     respective prime fields (`p`) and subgroup orders (`q`).
   - Precompute modulus constants and Montgomery parameters at compile time; expose
     wrappers for addition, subtraction, multiplication, squaring, and inversion
     implemented via Fermat exponentiation with fixed-window ladders.
   - Ensure all operations are constant-time (no data-dependent branches or early
     returns); rely on `subtle::Choice` for conditional selection.

2. **Curve Representation**
   - Represent points in Jacobian projective coordinates to eliminate per-operation
     inversions.
   - Encode generators/parameters as compile-time constants validated against existing
     tests; provide conversion helpers to/from the little-endian affine form used by the
     public API.
   - Implement unified addition/doubling formulas to avoid exceptional cases.

3. **Scalar Multiplication**
   - Implement a constant-time double-and-add ladder for arbitrary scalars (windowing
     remains a future optimisation).
   - Provide specialised paths:
     - `mul_base` detects the fixed generator and routes through the constant-time
       backend without re-deriving curve parameters.
     - `mul_add` performs `u1 * G + u2 * Q` with a joint ladder so verification doesn’t
       materialise intermediate points.

4. **Signature Algorithms**
   - Port signing and verification to the new projective arithmetic.
   - Reuse existing parsing helpers but rewrite them to validate coordinates using the
     constant-time checks.
   - Replace the current big-integer loop in `sign`/`verify` with constant-time calls.

5. **Deterministic Nonce (Task G3 Interface)**
   - Design a trait `DeterministicNonce` returning `Scalar` values; provide a Streebog +
     HMAC-DRBG implementation seeded with private key, message hash, and optional RNG
     entropy for hedging.
   - Ensure the scalar sampling uses the same rejection logic and returns canonical bytes.

6. **Testing & Validation**
   - Adapt existing unit tests to exercise both the compat and new backends during the
     transition.
   - Add Wycheproof vectors for TC26 curves (if available) and integration tests covering
     keygen/sign/verify.
   - Integrate dudect timing harnesses to detect data-dependent leakage.

7. **Rollout Strategy**
   - Implement the constant-time backend behind a feature flag (`gost-ct`), default it on
     once tests pass, and keep the compat bigint path under a temporary fallback feature.
   - Remove the compat path after parity tests and audits are complete.

8. **Implementation Checklist**
   - **File Layout**
     - `crates/iroha_crypto/src/gost/mod.rs`: expose the compat backend today; add a
       `#[cfg(feature = "gost-ct")]` branch that re-exports the constant-time modules.
     - `crates/iroha_crypto/src/gost/ct/mod.rs`: new entry point housing shared types,
       trait impls, and feature gating for downstream crates.
     - `crates/iroha_crypto/src/gost/ct/scalar.rs`: finite-field arithmetic implemented
       with constant-time Montgomery reduction (backed by 64-bit limbs).
     - `crates/iroha_crypto/src/gost/ct/point.rs`: curve arithmetic (add/double,
       scalar multiplication, subgroup checks) using the abstractions in `scalar.rs`.
     - `crates/iroha_crypto/src/gost/ct/nonce.rs`: host the `DeterministicNonce`
       trait and the Streebog-based HMAC-DRBG sampler shared across signing paths.
     - `crates/iroha_crypto/src/gost/ct/sign.rs` and `verify.rs`: constant-time
       signing/verification entry points, reused by the Torii and executor stacks.
     - `crates/iroha_crypto/tests/gost_ct.rs`: end-to-end regression vectors and
       cross-checks against the bigint backend.
   - **Module Skeletons**
     - `DeterministicNonce` trait with `fn next(&mut self, key: &SecretKey, msg: &[u8]) -> Scalar`
       plus an `HmacDrbg` builder that records personalization inputs.
     - `Scalar` type implementing `Zeroize`, `Display`, `ConstantTimeEq`, conversion
       helpers, and `TryFrom<[u8; 32]>` with strict range rejection.
     - `Point` type exposing `identity`, `is_infinity`, and constant-time ladder-based
       multiplication used by signing and verification.
     - Public `sign_detached`/`verify_detached` APIs that dispatch to the new backend
       behind the `gost-ct` feature while preserving the existing public surface.
   - **Owner Assignments**
     - *Crypto Backend*: @crypto-squad drives the `ct` module implementation and
       sampling primitives.
     - *Testing Harness*: @qa-verification maintains Wycheproof ingestion, dudect
       benches, and integration with CI.
     - *Rollout & Configuration*: @torii-runtime steers feature flag defaults,
       config plumbing, and coordinates the deprecation of the compat path.

## Open Questions

- Preferred approach if RustCrypto progress stalls (e.g., fallback to an
  internal fork versus adopting a vendor library).
- Requirements for hardware acceleration hooks once the constant-time backend is
  in place (SIMD, NEON, etc.).

> If additional background or data is needed, prepare LLM prompt drafts for
> @mtakemiya as noted in the roadmap coordination reminder.
