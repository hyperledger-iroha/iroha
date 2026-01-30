---
lang: fr
direction: ltr
source: docs/source/crypto/sm_risk_register.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba5f4fdc9221210a793fd0c2120d8cfb68487d7ddcbe67c208976798446ca5db
source_last_modified: "2026-01-03T18:07:57.078074+00:00"
translation_last_reviewed: 2026-01-30
---

//! SM program risk register for SM2/SM3/SM4 enablement.

# SM Program Risk Register

Last updated: 2025-03-12.

This register expands on the summary in `sm_program.md`, pairing each risk with
ownership, monitoring triggers, and the current mitigation state. The Crypto WG
and Core Platform leads review this register at the weekly SM cadence; changes
are reflected both here and in the public roadmap.

## Risk Summary

| ID | Risk | Category | Probability | Impact | Severity | Owner | Mitigation | Status | Triggers |
|----|------|----------|-------------|--------|----------|-------|------------|--------|----------|
| R1 | External audit for RustCrypto SM crates not executed before validator signing GA | Supply chain | Medium | High | High | Crypto WG | Contract Trail of Bits/NCC Group, keep verify-only posture until report accepted | Mitigation in progress | Audit SOW unsigned by 2025-04-15 or audit report delayed past 2025-06-01 |
| R2 | Deterministic nonce regressions across SDKs | Implementation | Medium | High | High | SDK Program Leads | Share fixtures across SDK CI, enforce canonical r∥s encoding, add cross-SDK tamper tests | Monitoring | Fixture drift detected in CI or SDK release without SM fixtures |
| R3 | ISA-specific bugs in intrinsics (NEON/SIMD) | Performance | Low | Medium | Medium | Performance WG | Gate intrinsics behind feature flags, require CI coverage on ARM, maintain scalar fallback | Mitigation in progress | NEON benches fail or hardware regression uncovered in SM perf matrix |
| R4 | Compliance ambiguity delaying SM adoption | Governance | Medium | Medium | Medium | Docs & Legal Liaison | Publish compliance brief, operator checklist, liaison with legal counsel prior to GA | Mitigation in progress | Legal review outstanding after 2025-05-01 or missing checklist updates |
| R5 | FFI backend drift with provider updates | Integration | Medium | Medium | Medium | Platform Ops | Pin provider versions, add parity tests, keep OpenSSL/Tongsuo preview opt-in | Monitoring | Package update merged without parity run or preview enabled outside pilot scope |

## Review Cadence

- Weekly Crypto WG sync (standing agenda item).
- Monthly joint review with Platform Ops and Docs to confirm compliance posture.
- Pre-release checkpoint: risk register freeze and attestation bundled with GA
  artefacts.

## Sign-off

| Role | Representative | Date | Notes |
|------|----------------|------|-------|
| Crypto WG Lead | (signature on file) | 2025-03-12 | Approved for publication and shared with WG backlog. |
| Core Platform Lead | (signature on file) | 2025-03-12 | Accepted mitigations and monitoring cadence. |

For historic approvals and meeting minutes, see `docs/source/crypto/sm_program.md`
(`Communication Plan`) and the SM agenda archive linked from the Crypto WG
workspace.
