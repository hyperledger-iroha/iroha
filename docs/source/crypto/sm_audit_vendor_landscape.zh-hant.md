---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_audit_vendor_landscape.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f39199767280be0fdd582301cdc3e8929497cf372a96f9f300e718f827000a7
source_last_modified: "2025-12-29T18:16:35.941305+00:00"
translation_last_reviewed: 2026-02-07
---

% SM Audit Vendor Landscape
% Iroha Crypto Working Group
% 2026-02-12

# Overview

The Crypto Working Group needs a standing bench of independent reviewers who
understand both Rust cryptography and the Chinese GM/T (SM2/SM3/SM4) standards.
This note catalogues firms with relevant references and summarises the audit
scope we typically request so request-for-proposal (RFP) cycles stay fast and
consistent.

# Candidate Firms

## Trail of Bits (CN Cryptography Practice)

- Documented engagements: 2023 security review of Ant Group’s Tongsuo
  (SM-enabled OpenSSL distribution) and repeated audits of Rust-based
  blockchains such as Diem/Libra, Sui, and Aptos.
- Strengths: dedicated Rust cryptography team, automated constant-time
  analysis tooling, experience validating deterministic execution and hardware
  dispatch policies.
- Fit for Iroha: can extend the current SM audit SOW or perform independent
  re-tests; comfortable operating with Norito fixtures and IVM syscall
  surfaces.

## NCC Group (APAC Cryptography Services)

- Documented engagements: gm/T (SM) code examinations for regional payment
  networks and HSM vendors; prior Rust reviews for Parity Substrate, Polkadot,
  and Libra components.
- Strengths: large APAC bench with bilingual reporting, ability to combine
  compliance-style process checks with deep code review.
- Fit for Iroha: ideal for second-opinion assessments or governance-driven
  validation alongside Trail of Bits findings.

## SECBIT Labs (Beijing)

- Documented engagements: maintainers of the open-source `libsm` Rust crate
  used by Nervos CKB and CITA; audited Guomi enablement for Nervos, Muta, and
  FISCO BCOS Rust components with bilingual deliverables.
- Strengths: engineers who actively ship SM primitives in Rust, strong
  property-testing capabilities, deep familiarity with domestic compliance
  requirements.
- Fit for Iroha: valuable when we need reviewers who can supply comparative
  test vectors and implementation guidance alongside findings.

## SlowMist Security (Chengdu)

- Documented engagements: Substrate/Polkadot Rust security reviews including
  Guomi forks for Chinese operators; routine assessments of SM2/SM3/SM4 wallet
  and bridge code used by exchanges.
- Strengths: blockchain-focused audit practice, integrated incident response,
  guidance that spans core protocol code and operator tooling.
- Fit for Iroha: helpful for validating SDK parity and operational touchpoints
  in addition to core crates.

## Chaitin Tech (QAX 404 Security Lab)

- Documented engagements: contributors to GmSSL/Tongsuo hardening and SM2/SM3/
  SM4 implementation guidance for domestic financial institutions; established
  Rust audit practice covering TLS stacks and cryptographic libraries.
- Strengths: deep cryptanalysis background, ability to pair formal verification
  artefacts with manual review, long-standing regulator relationships.
- Fit for Iroha: suitable when regulatory sign-off or formal proof artefacts
  need to accompany the standard code review report.

# Typical Audit Scope & Deliverables

- **Specification conformance:** validate SM2 ZA calculation, signature
  canonicalisation, SM3 padding/compression, and SM4 key schedule & IV handling
  against GM/T 0003-2012, GM/T 0004-2012, and GM/T 0002-2012.
- **Determinism and constant-time behaviour:** examine branching, lookup
  tables, and hardware feature gates (e.g., NEON, SM4 instructions) to ensure
  Rust and FFI dispatch remain deterministic across supported hardware.
- **FFI and provider integration:** review OpenSSL/Tongsuo bindings,
  PKCS#11/HSM adapters, and error propagation paths for consensus safety.
- **Test and fixture coverage:** assess fuzz harnesses, Norito round-trips,
  deterministic smoke tests, and recommend differential testing where gaps
  appear.
- **Dependency and supply-chain review:** confirm build provenance, vendor
  patch policies, SBOM accuracy, and reproducible build instructions.
- **Documentation and operations:** validate operator runbooks, compliance
  briefs, configuration defaults, and rollback procedures.
- **Reporting expectations:** executive summary with risk rating, detailed
  findings with code references & remediation guidance, retest plan, and
  attestations covering determinism guarantees.

# Next Steps

- Use this vendor roster during RFQ cycles; adjust the scope checklist above to
  match the active SM milestone before issuing an RFP.
- Record engagement outcomes in `docs/source/crypto/sm_audit_brief.md` and
  surface status updates in `status.md` once contracts are executed.
