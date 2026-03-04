---
lang: ur
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

# امپیکٹ اسسمنٹ ٹولنگ (MINFO - 4B)

روڈ میپ حوالہ: ** MINFO - 4B - امپیکٹ اسسمنٹ ٹولنگ۔ **  
مالک: گورننس کونسل / تجزیات

اس نوٹ میں `cargo xtask ministry-agenda impact` کمانڈ کی دستاویز کی گئی ہے
ریفرنڈم پیکٹوں کے ل required مطلوبہ خودکار ہیش فیملی فرق پیدا کرتا ہے۔
ٹول درست ایجنڈا کونسل کی تجاویز ، ڈپلیکیٹ رجسٹری ، اور استعمال کرتا ہے
ایک اختیاری ڈینیلسٹ/پالیسی اسنیپ شاٹ تاکہ جائزہ لینے والے بالکل وہی دیکھ سکیں جو
فنگر پرنٹس نئے ہیں ، جو موجودہ پالیسی سے ٹکرا جاتے ہیں ، اور کتنی اندراجات ہیں
ہر ہیش خاندان میں حصہ ڈالتا ہے۔

## ان پٹ

1. ** ایجنڈا کی تجاویز۔ ** ایک یا زیادہ فائلیں جو پیروی کرتی ہیں
   [`docs/source/ministry/agenda_council_proposal.md`] (agenda_council_proposal.md)۔
   ان کو واضح طور پر `--proposal <path>` کے ساتھ پاس کریں یا کمانڈ کو ایک پر اشارہ کریں
   `--proposal-dir <dir>` کے ذریعے ڈائرکٹری اور اس راستے کے تحت ہر `*.json` فائل
   شامل ہے۔
2. ** ڈپلیکیٹ رجسٹری (اختیاری)۔ ** JSON فائل مماثل
   `docs/examples/ministry/agenda_duplicate_registry.json`۔ تنازعات ہیں
   `source = "duplicate_registry"` کے تحت رپورٹ کیا گیا۔
3. ** پالیسی اسنیپ شاٹ (اختیاری)۔
   فنگر پرنٹ پہلے ہی GAR/وزارت پالیسی کے ذریعہ نافذ کیا گیا ہے۔ لوڈر کی توقع ہے
   اسکیما ذیل میں دکھایا گیا ہے (دیکھیں
   [`docs/examples/ministry/policy_snapshot_example.json`] (../../examples/ministry/policy_snapshot_example.json)
   ایک مکمل نمونے کے لئے):

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

کوئی بھی اندراج جس کا `hash_family:hash_hex` فنگر پرنٹ کسی تجویز کا ہدف سے میل کھاتا ہے
`source = "policy_snapshot"` کے تحت حوالہ `policy_id` کے ساتھ رپورٹ کیا گیا ہے۔

## استعمال

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

اضافی تجاویز کو دہرائے ہوئے `--proposal` جھنڈوں کے ذریعے یا بذریعہ شامل کیا جاسکتا ہے
ایک ڈائریکٹری کی فراہمی جس میں ایک پورا ریفرنڈم بیچ ہوتا ہے:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

جب `--out` کو خارج کیا جاتا ہے تو کمانڈ تیار کردہ JSON کو STDOUT پر پرنٹ کرتا ہے۔

## آؤٹ پٹ

رپورٹ ایک دستخط شدہ آرٹ فیکٹ ہے (اسے ریفرنڈم پیکٹ کے تحت ریکارڈ کریں
`artifacts/ministry/impact/` ڈائرکٹری) مندرجہ ذیل ڈھانچے کے ساتھ:

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

اس JSON کو غیر جانبدار خلاصہ کے ساتھ ساتھ ہر ریفرنڈم ڈوزیئر سے منسلک کریں
پینلسٹ ، جورز ، اور گورننس مبصرین کے عین مطابق دھماکے کا رداس دیکھ سکتے ہیں
ہر تجویز آؤٹ پٹ عین مطابق ہے (ہیش فیملی کے ذریعہ ترتیب دیا گیا ہے) اور اس کے لئے محفوظ ہے
CI/رن بکس میں شامل کریں ؛ اگر ڈپلیکیٹ رجسٹری یا پالیسی اسنیپ شاٹ میں تبدیلی آتی ہے ،
کمانڈ دوبارہ چلائیں اور ووٹ کے کھلنے سے پہلے ریفریشڈ آرٹ فیکٹ منسلک کریں۔

> ** اگلا مرحلہ: ** پیدا شدہ اثر کی رپورٹ کو کھانا کھلانا
> [`cargo xtask ministry-panel packet`] (referendum_packet.md) تو
> `ReferendumPacketV1` ڈوسیئر میں ہیش فیملی خرابی اور دونوں پر مشتمل ہے
> جائزہ کے تحت تجویز کے لئے تنازعہ کی تفصیلی فہرست۔