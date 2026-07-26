---
lang: ar
direction: rtl
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-11-15T09:17:21.371048+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/sns_training_template.md -->

# قالب شرائح تدريب SNS

يعكس هذا المخطط بـ Markdown الشرائح التي يجب على الميسرين تكييفها لمجموعات اللغة. انسخ هذه الاقسام الى Keynote/PowerPoint/Google Slides وخصص النقاط واللقطات والرسومات حسب الحاجة.

## شريحة العنوان
- البرنامج: "Sora Name Service onboarding"
- العنوان الفرعي: حدد suffix + cycle (مثال: `.sora - 2026-03`)
- المتحدثون + الجهات التابعة

## توجيه KPI
- لقطة شاشة او تضمين لـ `docs/portal/docs/sns/kpi-dashboard.md`
- قائمة نقاط تشرح فلاتر suffix وجدول ARPU ومتعقب freeze
- تنبيهات لتصدير PDF/CSV

## دورة حياة manifest
- مخطط: registrar -> Torii -> governance -> DNS/gateway
- خطوات تشير الى `docs/source/sns/registry_schema.md`
- مثال لمقتطف manifest مع شروح

## تدريبات النزاع والتجميد
- مخطط تدفق لتدخل guardian
- قائمة تحقق تشير الى `docs/source/sns/governance_playbook.md`
- مثال لخط زمني لتذكرة freeze

## التقاط annex
- مقتطف امر يظهر `cargo xtask sns-annex ... --portal-entry ...`
- تذكير بارشفة Grafana JSON تحت `artifacts/sns/regulatory/<suffix>/<cycle>/`
- رابط الى `docs/source/sns/reports/.<suffix>/<cycle>.md`

## الخطوات التالية
- رابط ملاحظات التدريب (انظر `docs/examples/sns_training_eval_template.md`)
- معرفات قنوات Slack/Matrix
- تواريخ المعالم القادمة

</div>
