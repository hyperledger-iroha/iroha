---
lang: ur
direction: rtl
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-11-15T09:17:21.371048+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/sns_training_template.md کا اردو ترجمہ -->

# SNS ٹریننگ سلائیڈ ٹیمپلیٹ

یہ Markdown آؤٹ لائن اُن سلائیڈز کی عکاسی کرتی ہے جنہیں فیسلیٹیٹرز کو اپنی زبان کی کوہوٹ کے مطابق ڈھالنا چاہیے۔ ان حصوں کو Keynote/PowerPoint/Google Slides میں کاپی کریں اور بلٹس، اسکرین شاٹس، اور ڈایاگرامز کو ضرورت کے مطابق مقامی بنائیں۔

## ٹائٹل سلائیڈ
- پروگرام: "Sora Name Service onboarding"
- سب ٹائٹل: suffix + cycle بیان کریں (مثال: `.sora - 2026-03`)
- پریزنٹرز + وابستگیاں

## KPI اورینٹیشن
- `docs/portal/docs/sns/kpi-dashboard.md` کا اسکرین شاٹ یا ایمبیڈ
- suffix فلٹرز، ARPU ٹیبل، اور freeze ٹریکر کی وضاحت کرنے والی بلٹ لسٹ
- PDF/CSV ایکسپورٹ کے لئے کال آؤٹس

## manifest لائف سائیکل
- ڈایاگرام: registrar -> Torii -> governance -> DNS/gateway
- `docs/source/sns/registry_schema.md` کے حوالہ جات والے مراحل
- annotations کے ساتھ manifest excerpt کی مثال

## dispute اور freeze ڈرلز
- guardian مداخلت کیلئے فلو ڈایاگرام
- `docs/source/sns/governance_playbook.md` کے حوالہ سے چیک لسٹ
- freeze ٹکٹ کی ٹائم لائن کی مثال

## annex کیپچر
- `cargo xtask sns-annex ... --portal-entry ...` دکھانے والا کمانڈ snippet
- Grafana JSON کو `artifacts/sns/regulatory/<suffix>/<cycle>/` کے تحت آرکائیو کرنے کی یاد دہانی
- `docs/source/sns/reports/.<suffix>/<cycle>.md` لنک

## اگلے اقدامات
- ٹریننگ فیڈبیک لنک (دیکھیں `docs/examples/sns_training_eval_template.md`)
- Slack/Matrix چینل ہینڈلز
- آنے والی مائل اسٹون تاریخیں

</div>
