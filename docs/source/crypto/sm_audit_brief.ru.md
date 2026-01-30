---
lang: ru
direction: ltr
source: docs/source/crypto/sm_audit_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9cda4648f0af7f89022e9d9f4ea243bc22685d9356927bbf1417c77b2057d872
source_last_modified: "2026-01-03T18:07:57.113286+00:00"
translation_last_reviewed: 2026-01-30
---

% SM2/SM3/SM4 External Audit Brief
% Iroha Crypto Working Group
% 2026-01-30

# Overview

This brief packages the engineering and compliance context required for an
independent review of Iroha’s SM2/SM3/SM4 enablement. It targets audit teams
with Rust cryptography experience and familiarity with Chinese National
Cryptography standards. The expected outcome is a written report covering
implementation risks, conformance gaps, and prioritised remediation guidance
ahead of the SM rollout moving from preview to production.

# Program Snapshot

- **Release scope:** Iroha 2/3 shared codebase, deterministic verification
  paths across nodes and SDKs, signing available behind configuration guard.
- **Current phase:** SM-P3.2 (OpenSSL/Tongsuo backend integration) with Rust
  implementations already shipping for verification and symmetric use-cases.
- **Target decision date:** 2026-04-30 (audit findings inform go/no-go for
  enabling SM signing in validator builds).
- **Key risks tracked:** third-party dependency pedigree, deterministic
  behaviour under mixed hardware, operator compliance readiness.

# Code & Fixture References

- `crates/iroha_crypto/src/sm.rs` — Rust implementations and optional OpenSSL
  bindings (`sm-ffi-openssl` feature).
- `crates/ivm/tests/sm_syscalls.rs` — IVM syscall coverage for hashing,
  verification, and symmetric modes.
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` — Norito payload
  round-trips for SM artefacts.
- `docs/source/crypto/sm_program.md` — programme history, dependency audit, and
  rollout guardrails.
- `docs/source/crypto/sm_operator_rollout.md` — operator-facing enablement and
  rollback procedures.
- `docs/source/crypto/sm_compliance_brief.md` — regulatory summary and export
  considerations.
- `scripts/sm_openssl_smoke.sh` / `crates/iroha_crypto/tests/sm_openssl_smoke.rs`
  — deterministic smoke harness for OpenSSL-backed flows.
- `fuzz/sm_*` corpora — RustCrypto-based fuzz seeds covering SM3/SM4 primitives.

# Requested Audit Scope

1. **Specification conformance**
   - Validate SM2 signature verification, ZA calculation, and canonical
     encoding behaviour.
   - Confirm SM3/SM4 primitives follow GM/T 0002-2012 and GM/T 0007-2012,
     including counter mode invariants and IV handling.
2. **Determinism & constant-time guarantees**
   - Review branching, table lookups, and hardware dispatch so node execution
     remains deterministic across CPU families.
   - Evaluate constant-time claims for private-key operations and confirm the
     OpenSSL/Tongsuo paths retain constant-time semantics.
3. **Side-channel and fault analysis**
   - Inspect for timing, cache, and power side-channel risks in both Rust and
     FFI-backed code paths.
   - Assess fault-handling and error propagation for signature verification and
     authenticated encryption failures.
4. **Build, dependency, and supply-chain review**
   - Confirm reproducible builds and provenance of OpenSSL/Tongsuo artefacts.
   - Review dependency tree licensing and audit coverage.
5. **Testing & verification harness critique**
   - Evaluate deterministic smoke tests, fuzz harnesses, and Norito fixtures.
   - Recommend additional coverage (e.g., differential testing, property-based
     proofs) if gaps remain.
6. **Compliance & operator guidance validation**
   - Cross-check shipped documentation against legal requirements and expected
     operator controls.

# Deliverables & Logistics

- **Kick-off:** 2026-02-24 (virtual, 90 minutes).
- **Interviews:** Crypto WG, IVM maintainers, platform ops (as needed).
- **Artefact access:** read-only repository mirror, CI pipeline logs, fixture
  outputs, and dependency SBOMs (CycloneDX).
- **Interim updates:** weekly written status + risk callouts.
- **Final deliverables (due 2026-04-15):**
  - Executive summary with risk rating.
  - Detailed findings (per issue: impact, likelihood, code references,
    remediation guidance).
  - Re-test/verification plan.
  - Statement on determinism, constant-time posture, and compliance alignment.

## Engagement Status

| Vendor | Status | Kick-off | Field Window | Notes |
|--------|--------|----------|--------------|-------|
| Trail of Bits (CN practice) | Statement of work executed 2026-02-21 | 2026-02-24 | 2026-02-24 – 2026-03-22 | Delivery due 2026-04-15; Hui Zhang leading engagement with Alexey M. as engineering counterpart. Weekly status call Wednesdays 09:00 UTC. |
| NCC Group (APAC) | Contingency slot reserved | N/A (on hold) | Provisional 2026-05-06 – 2026-05-31 | Activation only if high-risk findings require second pass; readiness confirmed by Priya N. (Security) and NCC Group engagement desk 2026-02-22. |

# Attachments Included in Outreach Package

- `docs/source/crypto/sm_program.md`
- `docs/source/crypto/sm_operator_rollout.md`
- `docs/source/crypto/sm_compliance_brief.md`
- `docs/source/crypto/sm_lock_refresh_plan.md`
- `docs/source/crypto/sm_rust_vector_check.md`
- `docs/source/crypto/attachments/sm_iroha_crypto_tree.txt` — `cargo tree -p iroha_crypto --no-default-features --features "sm sm-ffi-openssl"` snapshot.
- `docs/source/crypto/attachments/sm_iroha_crypto_metadata.json` — `cargo metadata` export for the `iroha_crypto` crate (locked dependency graph).
- `docs/source/crypto/attachments/sm_openssl_smoke.log` — latest `scripts/sm_openssl_smoke.sh` run (skips SM2/SM4 paths when provider support is missing).
- `docs/source/crypto/attachments/sm_openssl_provenance.md` — local toolkit provenance (pkg-config/OpenSSL version notes).
- Fuzz corpus manifest (`fuzz/sm_corpus_manifest.json`).

> **Environment caveat:** The current development snapshot uses the vendored OpenSSL 3.x toolchain (`openssl` crate `vendored` feature) but macOS lacks SM3/SM4 CPU intrinsics and the default provider does not expose SM4-GCM, so the OpenSSL smoke harness still skips SM4 coverage and Annex Example SM2 parsing. A workspace dependency cycle (`sorafs_manifest ↔ sorafs_car`) also forces the helper script to skip the run after emitting the `cargo check` failure. Re-run the bundle inside the Linux release build environment (OpenSSL/Tongsuo with SM4 enabled and without the cycle) to capture full parity before the external audit.

# Candidate audit partners & scope

| Firm | Relevant experience | Typical scope & deliverables | Notes |
|------|---------------------|------------------------------|-------|
| Trail of Bits (CN cryptography practice) | Rust code reviews (`ring`, zkVMs), prior GM/T assessments for mobile payment stacks. | Spec conformance diff (GM/T 0002/3/4), constant-time review of Rust + OpenSSL paths, differential fuzzing, supply-chain review, remediation roadmap. | Already engaged; table retained for completeness when planning future refresh cycles. |
| NCC Group APAC | Hardware/SOC + Rust cryptography red teams, published reviews of RustCrypto primitives and payment HSM bridges. | Holistic assessment of Rust + JNI/FFI bindings, deterministic policy validation, perf/telemetry gate review, operator playbook walkthrough. | Reserved as contingency; can also provide bilingual reporting for Chinese regulators. |
| Kudelski Security (Blockchain & crypto team) | Audits of Halo2, Mina, zkSync, custom signature schemes implemented in Rust. | Focus on elliptic-curve correctness, transcript integrity, threat modelling for hardware acceleration, and CI/rollout evidence. | Useful for second opinions on hardware acceleration (SM-5a) and FASTPQ-to-SM interactions. |
| Least Authority | Cryptographic protocol audits for Rust-based blockchains (Filecoin, Polkadot), reproducible builds consulting. | Deterministic build verification, Norito codec verification, compliance evidence cross-check, operator communication review. | Well-suited for transparency/audit-report deliverables when regulators request independent verification beyond code review. |

All engagements request the same artefact bundle enumerated above plus the following optional add-ons depending on the firm:

- **Spec conformance & deterministic behaviour:** Line-by-line verification of SM2 ZA derivation, SM3 padding, SM4 round functions, and the `sm_accel` runtime dispatch gate to ensure acceleration never alters semantics.
- **Side-channel and FFI review:** Inspection of constant-time claims, unsafe code blocks, and OpenSSL/Tongsuo bridging layers, including diff testing against the Rust path.
- **CI / supply-chain validation:** Reproduction of the `sm_interop_matrix`, `sm_openssl_smoke`, and `sm_perf` harnesses together with SBOM/SLSA attestations so audit findings can be tied directly to release evidence.
- **Operator-facing collateral:** Cross-check of `sm_operator_rollout.md`, compliance filing templates, and telemetry dashboards to confirm that mitigations promised in documentation are technically enforceable.

When scoping future audits, reuse this table to align vendor strengths with the specific roadmap milestone (e.g., favour Kudelski for hardware/perf heavy releases, Trail of Bits for language/runtime correctness, and Least Authority for reproducible build assurances).

# Points of Contact

- **Technical owner:** Crypto WG lead (Alexey M., `alexey@iroha.tech`)
- **Program manager:** Platform Operations coordinator (Sarah K.,
  `sarah@iroha.tech`)
- **Security liaison:** Security Engineering (Priya N., `security@iroha.tech`)
- **Documentation liaison:** Docs/DevRel lead (Jamila R.,
  `docs@iroha.tech`)
