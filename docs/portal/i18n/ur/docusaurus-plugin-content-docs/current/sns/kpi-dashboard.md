---
id: kpi-dashboard
lang: ur
direction: rtl
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
# Sora Name Service KPI ڈیش بورڈ

KPI ڈیش بورڈ stewards، guardians اور ریگولیٹرز کو ایک جگہ دیتا ہے تاکہ وہ ماہانہ annex cadence (SN-8a) سے پہلے اپنانے، خرابی اور آمدنی کے اشارے دیکھ سکیں۔ Grafana تعریف ریپو میں `dashboards/grafana/sns_suffix_analytics.json` پر موجود ہے، اور پورٹل ایک ایمبیڈڈ iframe کے ذریعے انہی پینلز کو دکھاتا ہے تاکہ تجربہ اندرونی Grafana انسٹینس سے میل کھائے۔

## فلٹرز اور ڈیٹا سورسز

- **سفکس فلٹر** – `sns_registrar_status_total{suffix}` کو ڈرائیو کرتا ہے تاکہ `.sora`، `.nexus` اور `.dao` کو الگ الگ دیکھا جا سکے۔
- **بلک ریلیز فلٹر** – `sns_bulk_release_payment_*` میٹرکس کو محدود کرتا ہے تاکہ فنانس ایک مخصوص registrar manifest کو reconcile کر سکے۔
- **میٹرکس** – Torii (`sns_registrar_status_total`, `torii_request_duration_seconds`)، guardian CLI (`guardian_freeze_active`)، `sns_governance_activation_total`، اور bulk-onboarding helper میٹرکس سے حاصل ہوتے ہیں۔

## پینلز

1. **رجسٹریشنز (گزشتہ 24h)** – منتخب سفکس کے لئے کامیاب registrar ایونٹس کی تعداد۔
2. **گورننس ایکٹیویشنز (30d)** – CLI میں ریکارڈ کی گئی charter/addendum motions۔
3. **Registrar تھرو پٹ** – ہر سفکس کے لئے کامیاب registrar ایکشنز کی شرح۔
4. **Registrar ایرر موڈز** – error لیبل والے `sns_registrar_status_total` کاؤنٹرز کی 5 منٹ ریٹ۔
5. **Guardian فریز ونڈوز** – live selectors جہاں `guardian_freeze_active` کھلا فریز ٹکٹ رپورٹ کرتا ہے۔
6. **اثاثہ جات کے لحاظ سے نیٹ ادائیگی یونٹس** – `sns_bulk_release_payment_net_units` کے ذریعے ہر اثاثے کے لئے رپورٹ شدہ totals۔
7. **ہر سفکس کے لئے بلک ریکویسٹس** – سفکس id کے لحاظ سے manifest volumes۔
8. **ہر ریکویسٹ پر نیٹ یونٹس** – release میٹرکس سے اخذ کیا گیا ARPU طرز حساب۔

## ماہانہ KPI ریویو چیک لسٹ

فنانس لیڈ ہر مہینے کے پہلے منگل کو recurring review چلاتا ہے:

1. پورٹل کے **Analytics → SNS KPI** صفحے (یا Grafana ڈیش بورڈ `sns-kpis`) کو کھولیں۔
2. registrar تھرو پٹ اور ریونیو ٹیبلز کا PDF/CSV export حاصل کریں۔
3. SLA breaches کے لئے سفکسز کا موازنہ کریں (error rate spikes، frozen selectors >72 h، ARPU deltas >10%).
4. `docs/source/sns/regulatory/<suffix>/YYYY-MM.md` کے تحت متعلقہ annex entry میں خلاصے + action items درج کریں۔
5. export شدہ dashboard artefacts کو annex commit کے ساتھ منسلک کریں اور council agenda میں لنک کریں۔

اگر ریویو میں SLA breaches سامنے آئیں تو متاثرہ مالک (registrar duty manager، guardian on-call، یا steward program lead) کے لئے PagerDuty incident فائل کریں اور annex log میں remediation کو ٹریک کریں۔
