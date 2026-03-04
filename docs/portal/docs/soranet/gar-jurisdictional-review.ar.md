---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79b860348fa0a776450d1233e5976b195f02a70ecaed02c569b5e0eba73e05f
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: مراجعة الاختصاصات GAR (SNNet-9)
sidebar_label: مراجعة الاختصاصات GAR
description: قرارات الاختصاصات الموقعة وdigests Blake2b لدمجها في إعدادات الامتثال الخاصة بـ SoraNet.
---

# مراجعة الاختصاصات GAR (SNNet-9)

تم الانتهاء من مسار الامتثال SNNet-9. تعرض هذه الصفحة قرارات الاختصاصات الموقعة، وdigests
Blake2b-256 التي يجب على المشغلين نسخها إلى كتل `compliance.attestations`، ومواعيد
المراجعة القادمة. احتفظ بملفات PDF الموقعة في أرشيف الحوكمة؛ هذه digests هي البصمات
القياسية للأتمتة وعمليات التدقيق.

| الاختصاص | القرار | المذكرة | digest Blake2b-256 (hex بحروف كبيرة) | المراجعة التالية |
|--------------|----------|------|--------------------------------------|-----------------|
| الولايات المتحدة | مطلوب نقل direct-only (بدون دوائر SoraNet) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| كندا | مطلوب نقل direct-only | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| الاتحاد الاوروبي/المنطقة الاقتصادية الاوروبية | يسمح بنقل SoraNet المجهول مع تطبيق ميزانيات الخصوصية SNNet-8 | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## مقتطف النشر

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

## قائمة تدقيق التدقيق

- نسخ digests الخاصة بالـ attestations بدقة إلى إعدادات الانتاج.
- تطابق `jurisdiction_opt_outs` مع الكتالوج القياسي.
- الاحتفاظ بملفات PDF الموقعة في أرشيف الحوكمة مع digests المتطابقة.
- توثيق نافذة التفعيل والموافقين في سجل GAR.
- جدولة تذكيرات المراجعة القادمة من الجدول اعلاه.

## راجع ايضا

- [GAR Operator Onboarding Brief](gar-operator-onboarding)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
