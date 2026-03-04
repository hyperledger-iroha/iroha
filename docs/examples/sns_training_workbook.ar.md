---
lang: ar
direction: rtl
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-11-15T09:17:29.843566+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/sns_training_workbook.md -->

# قالب كراسة تدريب SNS

استخدم هذه الكراسة كمستند التوزيع المعتمد لكل مجموعة تدريب. استبدل القوالب (`<...>`) قبل التوزيع على الحضور.

## تفاصيل الجلسة
- suffix: `<.sora | .nexus | .dao>`
- cycle: `<YYYY-MM>`
- اللغة: `<ar/es/fr/ja/pt/ru/ur>`
- الميسر: `<name>`

## المختبر 1 - تصدير KPI
1. افتح لوحة KPI في البوابة (`docs/portal/docs/sns/kpi-dashboard.md`).
2. صف حسب suffix `<suffix>` ونطاق الوقت `<window>`.
3. صدّر لقطات PDF + CSV.
4. سجل SHA-256 لملفات JSON/PDF المصدرة هنا: `______________________`.

## المختبر 2 - تمرين manifest
1. اجلب manifest العينة من `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`.
2. تحقق عبر `cargo run --bin sns_manifest_check -- --input <file>`.
3. ولّد هيكل resolver باستخدام `scripts/sns_zonefile_skeleton.py`.
4. الصق ملخص diff:
   ```
   <git diff output>
   ```

## المختبر 3 - محاكاة نزاع
1. استخدم CLI guardian لبدء freeze (case id `<case-id>`).
2. سجل hash النزاع: `______________________`.
3. ارفع سجل الادلة الى `artifacts/sns/training/<suffix>/<cycle>/logs/`.

## المختبر 4 - اتمتة annex
1. صدّر JSON للوحة Grafana وانسخه الى `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`.
2. شغّل:
   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. الصق مسار annex + ناتج SHA-256: `________________________________`.

## ملاحظات feedback
- ما الذي كان غير واضح؟
- اي المختبرات تجاوزت الوقت؟
- اخطاء tooling الملحوظة؟

اعِد الكراسات المكتملة الى الميسر؛ يجب حفظها تحت
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.

</div>
