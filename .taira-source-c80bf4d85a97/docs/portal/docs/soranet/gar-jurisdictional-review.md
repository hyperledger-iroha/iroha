---
title: GAR Jurisdictional Review (SNNet-9)
sidebar_label: GAR Jurisdictional Review
description: Signed-off jurisdiction decisions and Blake2b digests to wire into SoraNet compliance configs.
---

The SNNet-9 compliance track is now complete. This page lists the signed
jurisdiction decisions, the Blake2b-256 digests operators must copy into their
`compliance.attestations` blocks, and the next review dates. Keep the signed
PDFs in your governance archive; these digests are the canonical fingerprints
for automation and audits.

| Jurisdiction | Decision | Memo | Blake2b-256 digest (uppercase hex) | Next review |
|--------------|----------|------|------------------------------------|-------------|
| United States | Direct-only transport required (no SoraNet circuits) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Canada | Direct-only transport required | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| EU/EEA | Anonymous SoraNet transport allowed with SNNet-8 privacy budgets enforced | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## Deployment snippet

```jsonc
{
  "compliance": {
    "operator_jurisdictions": ["US", "CA", "DE"],
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

## Audit checklist

- Attestation digests copied exactly into production configs.
- `jurisdiction_opt_outs` matches the canonical catalogue.
- Signed PDFs retained in your governance archive with matching digests.
- Activation window and approvers captured in the GAR logbook.
- Next-review reminders scheduled from the table above.

## See also

- [GAR Operator Onboarding Brief](gar-operator-onboarding)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
