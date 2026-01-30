---
lang: he
direction: rtl
source: docs/source/crypto/attachments/sm_export_statement_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6742d2b87b8fbbc1493c5ae2704147b0f8d5d23af78004c2c9a112fe881efb11
source_last_modified: "2026-01-03T18:07:57.071790+00:00"
translation_last_reviewed: 2026-01-30
---

% SM2/SM3/SM4 Export-Control Statement Template
% Hyperledger Iroha Compliance Working Group
% 2026-05-06

# Usage

Embed this statement in release notes, manifests, or legal correspondence when
distributing SM-enabled artefacts. Update the placeholders to match the release,
jurisdiction, and applicable license exceptions. Retain a signed copy with the
release checklist.

# Statement

> **Product:** Hyperledger Iroha {{ RELEASE_VERSION }} (`{{ ARTEFACT_ID }}`)
>
> **Algorithms Included:** SM2 digital signature, SM3 hashing, SM4 symmetric
> encryption (GCM/CCM)
>
> **Export Classification:** United States EAR Category 5, Part 2 (5D002.c.1);
> European Union Regulation 2021/821 Annex 1, 5D002.
>
> **License Exception(s):** {{ LICENSE_EXCEPTION }} (e.g., ENC §740.17(b)(2),
> TSU §740.13 for source distribution).
>
> **Distribution Scope:** {{ DISTRIBUTION_SCOPE }} (e.g., “Global, excluding
> embargoed territories listed in 15 CFR 746”).
>
> **Operator Obligations:** Recipients must comply with applicable export,
> import, and usage regulations. Deployments within the People’s Republic of
> China require product and usage filings with the State Cryptography
> Administration and adherence to mainland data residency requirements.
>
> **Contact:** {{ LEGAL_CONTACT_NAME }} — {{ LEGAL_CONTACT_EMAIL }} /
> {{ LEGAL_CONTACT_PHONE }}
>
> This statement accompanies the cryptography compliance checklist and filing
> templates provided in `docs/source/crypto/sm_compliance_brief.md`. Retain this
> document and associated filings for a minimum of three years.

# Signature

- Authorised representative: ________________________
- Title: ________________________
- Date: ________________________

