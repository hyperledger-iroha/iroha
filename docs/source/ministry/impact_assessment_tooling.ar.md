---
lang: ar
direction: rtl
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2026-01-03T18:07:57.641039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# أدوات تقييم الأثر (MINFO‑4b)

مرجع خريطة الطريق: **MINFO‑4b — أدوات تقييم الأثر.**  
المالك: مجلس الإدارة/ التحليلات

توثق هذه المذكرة الأمر `cargo xtask ministry-agenda impact` الموجود الآن
يُنتج فرق التجزئة الآلي المطلوب لحزم الاستفتاء. ال
تستهلك الأداة مقترحات مجلس جدول الأعمال التي تم التحقق منها، والتسجيل المكرر، و
قائمة رفض/لقطة سياسة اختيارية حتى يتمكن المراجعون من معرفة أي منها بالضبط
بصمات الأصابع جديدة، والتي تتعارض مع السياسة القائمة، وكم عدد الإدخالات
تساهم كل عائلة تجزئة.

## المدخلات

1. **مقترحات جدول الأعمال.** ملف واحد أو أكثر يتبع ذلك
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   قم بتمريرها بشكل صريح باستخدام `--proposal <path>` أو قم بتوجيه الأمر إلى a
   الدليل عبر `--proposal-dir <dir>` وكل ملف `*.json` ضمن هذا المسار
   تم تضمينه.
2. ** سجل مكرر (اختياري). ** مطابقة ملف JSON
   `docs/examples/ministry/agenda_duplicate_registry.json`. الصراعات هي
   تم الإبلاغ عنه تحت `source = "duplicate_registry"`.
3. **لقطة السياسة (اختيارية).** بيان خفيف يسرد كل
   تم فرض بصمة الإصبع بالفعل بواسطة سياسة GAR/الوزارة. يتوقع المحمل
   المخطط الموضح أدناه (انظر
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   للحصول على عينة كاملة):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

أي إدخال تتطابق بصمة `hash_family:hash_hex` الخاصة به مع هدف الاقتراح هو
تم الإبلاغ عنه ضمن `source = "policy_snapshot"` مع `policy_id` المشار إليه.

## الاستخدام

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

يمكن إلحاق المقترحات الإضافية عبر إشارات `--proposal` المتكررة أو عن طريق
توفير دليل يحتوي على مجموعة استفتاء كاملة:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

يقوم الأمر بطباعة JSON الذي تم إنشاؤه إلى stdout عند حذف `--out`.

## الإخراج

التقرير عبارة عن قطعة أثرية موقعة (قم بتسجيله ضمن حزمة الاستفتاء
`artifacts/ministry/impact/`) بالبنية التالية:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

قم بإرفاق ملف JSON هذا بكل ملف استفتاء إلى جانب الملخص المحايد لذلك
يمكن لأعضاء اللجنة والمحلفين ومراقبي الحوكمة رؤية نصف قطر الانفجار الدقيق
كل اقتراح. الإخراج حتمي (مرتبة حسب عائلة التجزئة) وآمن
تضمينها في CI/دفاتر التشغيل؛ إذا تغير التسجيل المكرر أو لقطة السياسة،
أعد تشغيل الأمر وأرفق المنتج الذي تم تحديثه قبل فتح التصويت.

> **الخطوة التالية:** أدخل تقرير التأثير الذي تم إنشاؤه فيه
> [`cargo xtask ministry-panel packet`](referendum_packet.md) لذلك
> يحتوي الملف `ReferendumPacketV1` على كلٍ من تقسيم عائلة التجزئة وملف
> قائمة التعارضات التفصيلية للمقترح قيد المراجعة.