---
lang: hy
direction: ltr
source: docs/source/soranet/gar_jurisdictional_review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b161744ce32089fdd0b4a664e79ae7924e9d02f1736c28c34105ea9e5a229d0c
source_last_modified: "2025-12-29T18:16:36.187633+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet GAR Jurisdictional Review (SNNet-9)
summary: Signed-off per-jurisdiction decisions and evidence digests to close the SNNet-9 compliance gate.
---

# GAR Jurisdictional Review

The SNNet-9 roadmap item required a jurisdiction-by-jurisdiction legal review
with auditable artefacts plus operator-facing guidance. This document records
the completed review, links the signed memos, and provides the attestation
digests operators must reference in their `compliance.attestations` blocks.

> This review is informational and does not replace dedicated legal advice for
> specific deployments. Operators remain responsible for running their own
> counsel reviews and mirroring the digests below in their activation manifests.

## Evidence set

All jurisdiction memos live under `governance/compliance/attestations/` with
Blake2b-256 digests recorded here for reproducibility. The canonical opt-out
catalog remains in `governance/compliance/soranet_opt_outs.json`.

| Jurisdiction | Decision | Memo | Blake2b-256 digest (uppercase hex) | Next review |
|--------------|----------|------|------------------------------------|-------------|
| United States | Direct-only transport required (no SoraNet circuits) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Canada | Direct-only transport required | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| EU/EEA | Anonymous SoraNet transport allowed with SNNet-8 privacy budgets enforced | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## Deployment instructions

1. Merge the canonical opt-outs with your own footprint. For most operators the
   `jurisdiction_opt_outs` list should remain `["US", "CA"]`.
2. Add the attestation digests to your config; this keeps rollouts auditable
   without shipping the signed PDFs:

   ```jsonc
   {
     "compliance": {
       "operator_jurisdictions": ["US", "CA", "JP", "DE"],
       "jurisdiction_opt_outs": ["US", "CA"],
       "blinded_cid_opt_outs": [
         "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828",
         "7F8B1E9D04878F1AEAB553C1DB0A3E3A2AB689F75FE6BE17469F85A4D201B4AC"
       ],
       "attestations": [
         {
           "jurisdiction": "US",
           "document_uri": "norito://gar/attestations/us-2027-q2.pdf",
           "digest_hex": "1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7",
           "issued_at_ms": 1805313600000,
           "expires_at_ms": 1822248000000
         },
         {
           "jurisdiction": "CA",
           "document_uri": "norito://gar/attestations/ca-2027-q2.pdf",
           "digest_hex": "52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063",
           "issued_at_ms": 1805313600000,
           "expires_at_ms": 1822248000000
         },
         {
           "jurisdiction": "EU",
           "document_uri": "norito://gar/attestations/eu-2027-q2.pdf",
           "digest_hex": "30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15",
           "issued_at_ms": 1805313600000
         }
       ]
     }
   }
   ```

3. Record the activation in your GAR logbook (git tag, rollout window, approver
   list) and keep the signed PDFs alongside the Markdown companions cited above.
4. Attach this review to your compliance runbooks and operator onboarding kits
   so downstream teams can cite the same digests.

## Audit checklist

- ✅ Attestation digests copied into production config.
- ✅ `jurisdiction_opt_outs` matches `governance/compliance/soranet_opt_outs.json`.
- ✅ Signed PDFs stored in the governance archive with the same digests.
- ✅ Activation captured in the GAR logbook with timestamps and approvers.
- ✅ Next-review dates scheduled and noted in the operator calendar.

## Contacts

- Governance Council: `governance@soranet.org`
- Legal/Compliance desk: `legal@soranet.org`
- Data Protection Officer (EU/EEA): `dpo@soranet.org`

For rollout guidance see the Operator Onboarding Brief and the GAR Compliance
Playbook.
