---
lang: he
direction: rtl
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79b860348fa0a776450d1233e5976b195f02a70ecaed02c569b5e0eba73e05f
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: סקירת תחום שיפוט GAR (SNNet-9)
sidebar_label: סקירת תחום שיפוט GAR
description: החלטות תחום שיפוט חתומות ו-digests של Blake2b לשילוב בהגדרות ה-compliance של SoraNet.
---

# סקירת תחום שיפוט GAR (SNNet-9)

מסלול ה-compliance SNNet-9 הושלם. עמוד זה מפרט את החלטות תחום השיפוט החתומות,
digests מסוג Blake2b-256 שעל המפעילים להעתיק לבלוקים `compliance.attestations`, ואת מועדי
הסקירה הבאים. שמרו את ה-PDFs החתומים בארכיון הממשל; ה-digests הללו הם טביעות
האצבע הקנוניות לאוטומציה ולאודיטים.

| תחום שיפוט | החלטה | Memo | Digest Blake2b-256 (hex באותיות גדולות) | סקירה הבאה |
|--------------|----------|------|----------------------------------------|------------|
| United States | נדרש transport direct-only (ללא מעגלי SoraNet) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Canada | נדרש transport direct-only | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| EU/EEA | transport אנונימי של SoraNet מותר עם החלת SNNet-8 privacy budgets | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## קטע פריסה

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

## צ'קליסט אודיט

- Digests של attestations הועתקו בדיוק להגדרות production.
- `jurisdiction_opt_outs` תואם לקטלוג הקנוני.
- PDFs חתומים נשמרו בארכיון הממשל עם digests תואמים.
- חלון ההפעלה והמאשרים תועדו ביומן GAR.
- תזכורות לסקירה הבאה נקבעו מתוך הטבלה למעלה.

## ראו גם

- [GAR Operator Onboarding Brief](gar-operator-onboarding)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
