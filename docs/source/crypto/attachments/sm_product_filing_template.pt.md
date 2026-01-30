---
lang: pt
direction: ltr
source: docs/source/crypto/attachments/sm_product_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7116d28e32d8bd77434edd6767427cc3d2ae0624f4de132b1d0cec3c7d44b86
source_last_modified: "2026-01-03T18:07:57.069144+00:00"
translation_last_reviewed: 2026-01-30
---

% SM2/SM3/SM4 Product Filing (开发备案) Template
% Hyperledger Iroha Compliance Working Group
% 2026-05-06

# Instructions

Use this template when submitting a *product development filing* to a provincial
or municipal State Cryptography Administration (SCA) office before distributing
SM-enabled binaries or source artefacts from within mainland China. Replace the
placeholders with project-specific details, export the completed form as PDF if
required, and attach the artefacts referenced in the checklist.

# 1. Applicant & Product Summary

| Field | Value |
|-------|-------|
| Organisation name | {{ ORGANISATION }} |
| Registered address | {{ ADDRESS }} |
| Legal representative | {{ LEGAL_REP }} |
| Primary contact (name / title / email / phone) | {{ CONTACT }} |
| Product name | Hyperledger Iroha {{ RELEASE_NAME }} |
| Product version / build ID | {{ VERSION }} |
| Filing type | Product development (开发备案) |
| Filing date | {{ YYYY-MM-DD }} |

# 2. Cryptography Usage Overview

- Supported algorithms: `SM2`, `SM3`, `SM4` (provide usage matrix below).
- Usage context:
  | Algorithm | Component | Purpose | Deterministic safeguards |
  |-----------|-----------|---------|--------------------------|
  | SM2 | {{ COMPONENT }} | {{ PURPOSE }} | RFC6979 + canonical r∥s enforcement |
  | SM3 | {{ COMPONENT }} | {{ PURPOSE }} | Deterministic hashing via `Sm3Digest` |
  | SM4 | {{ COMPONENT }} | {{ PURPOSE }} | AEAD (GCM/CCM) with enforced nonce policy |
- Non-SM algorithms in build: {{ OTHER_ALGORITHMS }} (for completeness).

# 3. Development & Supply Chain Controls

- Source code repository: {{ REPOSITORY_URL }}
- Deterministic build instructions:
  1. `git clone {{ REPOSITORY_URL }} && git checkout {{ COMMIT_SHA }}`
  2. `cargo build --workspace --locked --release --features "sm sm-ffi-openssl"` (adjust as needed).
  3. SBOM generated via `cargo auditable` / CycloneDX (`{{ SBOM_PATH }}`).
- Continuous integration environment summary:
  | Item | Value |
  |------|-------|
  | Build OS / version | {{ BUILD_OS }} |
  | Compiler toolchain | {{ TOOLCHAIN }} |
  | OpenSSL / Tongsuo source | {{ OPENSSL_SOURCE }} |
  | Reproducibility checksum | {{ CHECKSUM }} |

# 4. Key Management & Security

- Default enabled SM features: {{ DEFAULTS }} (e.g., verify-only).
- Configuration flags required for signing: {{ CONFIG_FLAGS }}.
- Key custody approach:
  | Item | Details |
  |------|---------|
  | Key generation tool | {{ KEY_TOOL }} |
  | Storage medium | {{ STORAGE_MEDIUM }} |
  | Backup policy | {{ BACKUP_POLICY }} |
  | Access controls | {{ ACCESS_CONTROLS }} |
- Incident response contacts (24/7):
  | Role | Name | Phone | Email |
  |------|------|-------|-------|
  | Crypto lead | {{ NAME }} | {{ PHONE }} | {{ EMAIL }} |
  | Platform ops | {{ NAME }} | {{ PHONE }} | {{ EMAIL }} |
  | Legal liaison | {{ NAME }} | {{ PHONE }} | {{ EMAIL }} |

# 5. Attachments Checklist

- [ ] Source code snapshot (`{{ SOURCE_ARCHIVE }}`) and hash.
- [ ] Deterministic build script / reproducibility notes.
- [ ] SBOM (`{{ SBOM_PATH }}`) and dependency manifest (`Cargo.lock` fingerprint).
- [ ] Deterministic test transcripts (`scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm`).
- [ ] Telemetry dashboard export demonstrating SM observability.
- [ ] Export-control statement (see separate template).
- [ ] Audit reports or third-party assessments (if already completed).

# 6. Applicant Declaration

> I confirm that the above information is accurate, that the disclosed
> cryptographic functionality complies with applicable PRC laws and regulations,
> and that the organisation will maintain the submitted artefacts for at least
> three years.

- Signature (legal representative): ________________________
- Date: ________________________

