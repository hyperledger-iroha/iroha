---
lang: ar
direction: rtl
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b46ad81721ede2a5c95fc95a445267c4970b4a6ce669c75caadc65e2542b73d7
source_last_modified: "2025-11-05T17:22:30.409223+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/soranet_incentive_parliament_packet/README.md -->

# حزمة برلمان حوافز Relay لـ SoraNet

تلتقط هذه الحزمة artefacts المطلوبة من برلمان Sora لاعتماد مدفوعات relay التلقائية (SNNet-7):

- `reward_config.json` - اعدادات محرك المكافآت القابلة للتسلسل عبر Norito، جاهزة للاستيعاب بواسطة `iroha app sorafs incentives service init`. يطابق `budget_approval_id` الهاش المدرج في محاضر الحوكمة.
- `shadow_daemon.json` - خريطة المستفيدين والسندات المستخدمة من قبل ادوات replay (`shadow-run`) والdaemon في الانتاج.
- `economic_analysis.md` - ملخص الانصاف لمحاكاة shadow 2025-10 -> 2025-11.
- `rollback_plan.md` - دليل تشغيلي لتعطيل المدفوعات التلقائية.
- artefacts داعمة: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## فحوصات النزاهة

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/*       docs/examples/soranet_incentive_shadow_run.json       docs/examples/soranet_incentive_shadow_run.sig
```

قارن digests مع القيم المسجلة في محاضر البرلمان. تحقق من توقيع shadow-run كما هو موضح في
`docs/source/soranet/reports/incentive_shadow_run.md`.

## تحديث الحزمة

1. حدّث `reward_config.json` كلما تغيرت اوزان المكافآت او الدفع الاساسي او هاش الموافقة.
2. اعادة تشغيل محاكاة shadow لمدة 60 يوما، تحديث `economic_analysis.md` بالنتائج الجديدة، وارسال ملف JSON مع التوقيع المنفصل.
3. قدم الحزمة المحدثة الى البرلمان مع صادرات لوحة Observatory عند طلب تجديد الموافقة.

</div>
