---
lang: am
direction: ltr
source: docs/source/crypto/sm_audit_success_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ef9305dc14d477a616923c80445094c692bc6a38d69465f679b54ccd52e92
source_last_modified: "2025-12-29T18:16:35.940844+00:00"
translation_last_reviewed: 2026-02-07
---

% SM2/SM3/SM4 Audit Success Criteria
% Iroha Crypto Working Group
% 2026-01-30

# Purpose

This checklist captures the concrete criteria required for a successful
completion of the SM2/SM3/SM4 external audit. It should be reviewed during
kick-off, revisited at each status checkpoint, and used to confirm exit
conditions before enabling SM signing for production validators.

# Pre-Engagement Readiness

- [ ] Contract signed, including scope, deliverables, confidentiality, and
      remediation support language.
- [ ] Audit team receives repository mirror access, CI artefact bucket, and
      documentation bundle listed in `docs/source/crypto/sm_audit_brief.md`.
- [ ] Points of contact confirmed with backups for each role
      (crypto, IVM, platform ops, security, docs).
- [ ] Internal stakeholders align on target release date and freeze windows.
- [ ] SBOM export (`cargo auditable` + CycloneDX) generated and shared.
- [ ] OpenSSL/Tongsuo build provenance package prepared
      (source tarball hash, build script, reproducibility notes).
- [ ] Latest deterministic test outputs captured:
      `scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm`, and
      Norito round-trip fixtures.
- [ ] Torii `/v1/node/capabilities` advert (via `iroha runtime capabilities`) recorded, verifying the `crypto.sm` manifest fields and acceleration policy snapshot.

# Engagement Execution

- [ ] Kick-off workshop completed with shared understanding of goals,
      timelines, and communication cadence.
- [ ] Weekly status reports received and triaged; risk register updated.
- [ ] Findings communicated within one business day of discovery when severity
      is High or Critical.
- [ ] Audit team validates determinism paths on ≥2 CPU architectures (x86_64,
      aarch64) with matching outputs.
- [ ] Side-channel review includes constant-time proofs or empirical testing
      evidence for both Rust and FFI paths.
- [ ] Compliance and documentation review confirms operator guidance matches
      regulatory obligations.
- [ ] Differential testing against reference implementations (RustCrypto,
      OpenSSL/Tongsuo) executed with auditor oversight.
- [ ] Fuzz harnesses evaluated; new seed corpora provided where gaps exist.

# Remediation & Exit

- [ ] All findings categorised with severity, impact, exploitability, and
      recommended remediation steps.
- [ ] High/Critical issues receive patches or mitigations with auditor-approved
      verification; residual risks documented.
- [ ] Auditor supplies re-test validation evidencing fixed issues (diff, test
      runs, or signed attestation).
- [ ] Final report delivered: executive summary, detailed findings, methodology,
      determinism verdict, compliance verdict.
- [ ] Internal sign-off meeting concludes next steps, release adjustments,
      and documentation updates.
- [ ] `status.md` updated with audit outcome and outstanding remediation
      follow-ups.
- [ ] Post-mortem captured in `docs/source/crypto/sm_program.md` (lessons
      learned, future hardening tasks).
