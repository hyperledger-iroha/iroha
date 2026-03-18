---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79b860348fa0a776450d1233e5976b195f02a70ecaed02c569b5e0eba73e05f
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: GAR Jurisdictional Review (SNNet-9)
sidebar_label: GAR Jurisdictional Review
description: دستخط شدہ jurisdiction فیصلے اور Blake2b digests جو SoraNet compliance configs میں شامل کرنا ضروری ہیں۔
---

# GAR Jurisdictional Review (SNNet-9)

SNNet-9 compliance track اب مکمل ہے۔ یہ صفحہ دستخط شدہ jurisdiction فیصلے، وہ Blake2b-256 digests جنہیں operators کو
`compliance.attestations` blocks میں کاپی کرنا ہے، اور اگلی review تاریخیں درج کرتا ہے۔ دستخط شدہ PDFs کو اپنے governance archive
میں محفوظ رکھیں؛ یہ digests automation اور audits کے لئے canonical fingerprints ہیں۔

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

- Attestation digests کو production configs میں بالکل درست کاپی کیا گیا ہو۔
- `jurisdiction_opt_outs` canonical catalogue سے match ہو۔
- دستخط شدہ PDFs governance archive میں matching digests کے ساتھ محفوظ ہوں۔
- Activation window اور approvers GAR logbook میں درج ہوں۔
- Next-review reminders اوپر والی جدول سے schedule ہوں۔

## See also

- [GAR Operator Onboarding Brief](gar-operator-onboarding)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
