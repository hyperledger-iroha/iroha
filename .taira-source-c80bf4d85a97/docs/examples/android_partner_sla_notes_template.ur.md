---
lang: ur
direction: rtl
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f1d3e5d2b42f7e6f9c2de4f2be56b6994b4b88b109f70edc7e6f04ec0f3465ac
source_last_modified: "2025-11-12T08:32:28.349523+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/android_partner_sla_notes_template.md کا اردو ترجمہ -->

# Android پارٹنر SLA ڈسکوری نوٹس - ٹیمپلیٹ

ہر AND8 SLA discovery سیشن کے لئے یہ ٹیمپلیٹ استعمال کریں۔ مکمل شدہ کاپی کو
`docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md` کے تحت محفوظ کریں
اور معاون artefacts (questionnaire responses، acknowledgements، attachments) اسی ڈائریکٹری میں
منسلک کریں۔

```
Partner: <Name>                      Date: <YYYY-MM-DD>  Time: <UTC>
Primary contact(s): <names, roles, email>
Android attendees: <Program Lead / Partner Eng / Support Eng / Compliance>
Meeting link / ticket: <URL or ID>
```

## 1. ایجنڈا اور سیاق

- سیشن کا مقصد (pilot scope، release window، telemetry توقعات)۔
- کال سے پہلے شیئر کیے گئے حوالہ docs (support playbook، release calendar،
  telemetry dashboards)۔

## 2. ورک لوڈ اوور ویو

| موضوع | نوٹس |
|-------|-------|
| ہدف ورک لوڈز / چینز | |
| متوقع ٹرانزیکشن والیوم | |
| اہم بزنس ونڈوز / blackout ادوار | |
| ریگولیٹری regimes (GDPR، MAS، FISC وغیرہ) | |
| مطلوبہ زبانیں / localisation | |

## 3. SLA گفتگو

| SLA کلاس | پارٹنر کی توقع | baseline سے فرق؟ | مطلوبہ کارروائی |
|----------|---------------|------------------|-----------------|
| Critical fix (48 h) | | Yes/No | |
| High-severity (5 business days) | | Yes/No | |
| Maintenance (30 days) | | Yes/No | |
| Cutover notice (60 days) | | Yes/No | |
| Incident communications cadence | | Yes/No | |

پارٹنر کی درخواست کردہ اضافی SLA شقیں دستاویز کریں (مثال کے طور پر dedicated phone bridge،
اضافی telemetry exports)۔

## 4. Telemetry اور ایکسس کی ضروریات

- Grafana / Prometheus ایکسس ضروریات:
- Logs/trace export تقاضے:
- Offline evidence یا dossier توقعات:

## 5. Compliance اور قانونی نوٹس

- Jurisdictional notification تقاضے (statute + timing)۔
- Incident updates کے لئے قانونی contacts درکار ہیں۔
- Data residency پابندیاں / storage تقاضے۔

## 6. فیصلے اور ایکشن آئٹمز

| آئٹم | Owner | Due | نوٹس |
|------|-------|-----|------|
| | | | |

## 7. Acknowledgement

- پارٹنر نے baseline SLA کو تسلیم کیا؟ (Y/N)
- Follow-up acknowledgement طریقہ (email / ticket / signature):
- تصدیقی email یا meeting minutes اس ڈائریکٹری میں بند کرنے سے پہلے منسلک کریں۔

</div>
