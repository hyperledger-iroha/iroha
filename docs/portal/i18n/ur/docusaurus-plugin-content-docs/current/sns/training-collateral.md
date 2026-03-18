---
lang: ur
direction: rtl
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: training-collateral
title: SNS تربیتی مواد
description: نصاب، لوکلائزیشن ورک فلو، اور SN-8 کے تحت درکار ضمیمہ شواہد کی گرفت۔
---

> `docs/source/sns/training_collateral.md` کی عکاسی کرتا ہے۔ ہر سفکس لانچ سے پہلے رجسٹرار، DNS، guardian اور فنانس ٹیموں کو بریف کرتے وقت اس صفحے کو استعمال کریں۔

## 1. نصاب کا خلاصہ

| ٹریک | مقاصد | پری ریڈز |
|-------|------------|-----------|
| رجسٹرار آپریشنز | منيفسٹس جمع کرنا، KPI ڈیش بورڈز مانیٹر کرنا، غلطیوں کو اسکیلٹ کرنا۔ | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS اور گیٹ وے | ریزولور اسکیلیٹن لاگو کرنا، فریز/رول بیک کی مشق کرنا۔ | `sorafs/gateway-dns-runbook`, direct-mode policy samples. |
| Guardians اور کونسل | تنازعات انجام دینا، گورننس ایڈینڈا اپ ڈیٹ کرنا، ضمیمے لاگ کرنا۔ | `sns/governance-playbook`, steward scorecards. |
| فنانس اور اینالیٹکس | ARPU/bulk میٹرکس حاصل کرنا، ضمیمہ بنڈلز شائع کرنا۔ | `finance/settlement-iso-mapping`, KPI dashboard JSON. |

### ماڈیول فلو

1. **M1 — KPI اورینٹیشن (30 منٹ):** سفکس فلٹرز، ایکسپورٹس، اور فریز کاؤنٹرز کا جائزہ۔ ڈیلیوریبل: SHA-256 ڈائجسٹ کے ساتھ PDF/CSV اسنیپ شاٹس۔
2. **M2 — منيفسٹ لائف سائیکل (45 منٹ):** رجسٹرار منيفسٹس بنانا اور ویلیڈیٹ کرنا، `scripts/sns_zonefile_skeleton.py` سے ریزولور اسکیلیٹن بنانا۔ ڈیلیوریبل: اسکیلیٹن + GAR ثبوت دکھانے والا git diff۔
3. **M3 — تنازعہ ڈرلز (40 منٹ):** guardian فریز + اپیل کی سِمیولیشن، CLI لاگز `artifacts/sns/training/<suffix>/<cycle>/logs/` کے تحت محفوظ کریں۔
4. **M4 — ضمیمہ کیپچر (25 منٹ):** ڈیش بورڈ JSON ایکسپورٹ کریں اور چلائیں:

   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   ڈیلیوریبل: اپ ڈیٹ شدہ ضمیمہ Markdown + ریگولیٹری میمو + پورٹل بلاکس۔

## 2. لوکلائزیشن ورک فلو

- زبانیں: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, `ur`.
- ہر ترجمہ سورس فائل کے ساتھ ہوتا ہے (`docs/source/sns/training_collateral.<lang>.md`). ریفریش کے بعد `status` + `translation_last_reviewed` اپ ڈیٹ کریں۔
- ہر زبان کے assets `artifacts/sns/training/<suffix>/<lang>/<cycle>/` میں رکھے جاتے ہیں (slides/, workbooks/, recordings/, logs/).
- انگریزی سورس ایڈٹ کرنے کے بعد `python3 scripts/sync_docs_i18n.py --lang <code>` چلائیں تاکہ مترجمین نیا hash دیکھ سکیں۔

### ڈیلیوری چیک لسٹ

1. لوکلائزیشن کے بعد ترجمہ stub (`status: complete`) اپ ڈیٹ کریں۔
2. سلائیڈز کو PDF میں ایکسپورٹ کر کے ہر زبان کے `slides/` فولڈر میں اپ لوڈ کریں۔
3. ≤10 منٹ کا KPI walkthrough ریکارڈ کریں؛ زبان کے stub سے لنک کریں۔
4. `sns-training` ٹیگ کے ساتھ گورننس ٹکٹ فائل کریں جس میں سلائیڈ/ورک بک ڈائجسٹ، ریکارڈنگ لنکس، اور ضمیمہ ثبوت شامل ہوں۔

## 3. تربیتی اثاثے

- سلائیڈ آؤٹ لائن: `docs/examples/sns_training_template.md`.
- ورک بک ٹیمپلیٹ: `docs/examples/sns_training_workbook.md` (ہر شریک کے لیے ایک).
- دعوت + یاد دہانی: `docs/examples/sns_training_invite_email.md`.
- ارزیابی فارم: `docs/examples/sns_training_eval_template.md` (جوابات `artifacts/sns/training/<suffix>/<cycle>/feedback/` میں محفوظ ہوں گے).

## 4. شیڈولنگ اور میٹرکس

| سائیکل | ونڈو | میٹرکس | نوٹس |
|-------|--------|---------|-------|
| 2026‑03 | KPI ریویو کے بعد | حاضری %, ضمیمہ ڈائجسٹ لاگ | `.sora` + `.nexus` cohorts |
| 2026‑06 | `.dao` GA سے پہلے | فنانس ریڈینس ≥90 % | پالیسی ریفریش شامل کریں |
| 2026‑09 | توسیع | تنازعہ ڈرل <20 منٹ، ضمیمہ SLA ≤2 دن | SN-7 incentives کے ساتھ ہم آہنگ کریں |

گمنام فیڈبیک `docs/source/sns/reports/sns_training_feedback.md` میں جمع کریں تاکہ آنے والی cohorts لوکلائزیشن اور لیبس بہتر کر سکیں۔

