---
lang: ur
direction: rtl
source: docs/examples/docs_preview_feedback_form.md
status: complete
translator: manual
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-11-10T19:22:20.036140+00:00"
translation_last_reviewed: 2026-01-30
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/examples/docs_preview_feedback_form.md (Docs preview feedback form) -->

# Docs preview فیڈبیک فارم (W1 ویو)

یہ ٹیمپلیٹ W1 ویو کے reviewers سے فیڈبیک جمع کرنے کے لیے استعمال کریں۔ ہر پارٹنر کے
لیے اس کی ایک کاپی بنائیں، میٹا ڈیٹا فیلڈز کو پُر کریں، اور مکمل شدہ فارم کو اس
پاتھ کے نیچے اسٹور کریں:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`۔

## Reviewer میٹا ڈیٹا

- **Partner ID:** `partner-w1-XX`
- **Request ticket:** `DOCS-SORA-Preview-REQ-PXX`
- **Invite بھیجے جانے کا وقت (UTC):** `YYYY-MM-DD hh:mm`
- **Checksum acknowledge ہونے کا وقت (UTC):** `YYYY-MM-DD hh:mm`
- **Primary focus areas:** (مثلاً _SoraFS orchestrator docs_، _Torii ISO flows_)

## Telemetry اور artefact کنفرمیشنز

| Checklist item | نتیجہ | Evidence |
| --- | --- | --- |
| Checksum verification | ✅ / ⚠️ | لاگ فائل کا path (مثلاً `build/checksums.sha256`) |
| Try it proxy smoke test | ✅ / ⚠️ | `npm run manage:tryit-proxy …` کا حصہ |
| Grafana dashboard review | ✅ / ⚠️ | اسکرین شاٹ/اسکرین شاٹس کا path |
| Portal probe report review | ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

اگر reviewer اضافی SLOs کو چیک کرے، تو ان کے لیے بھی نئی rows شامل کریں۔

## فیڈبیک لاگ

| Area | Severity (info/minor/major/blocker) | Description | تجویز کردہ fix یا سوال | Tracker issue |
| --- | --- | --- | --- | --- |
| | | | | |

آخری کالم میں GitHub issue یا اندرونی ٹکٹ کا حوالہ دیں تاکہ preview ٹریکر، remediation
آئٹمز کو اسی فارم کے ساتھ لنک کر سکے۔

## Survey خلاصہ

1. **Checksum گائیڈنس اور invite پروسیس پر آپ کا اعتماد کس سطح کا ہے؟** (1–5)
2. **کون سی docs سب سے زیادہ/کم مددگار تھیں؟** (مختصر جواب)
3. **Try it proxy یا telemetry dashboards تک رسائی میں کوئی بلاکر پیش آیا؟**
4. **کیا اضافی localisation یا accessibility مواد درکار ہے؟**
5. **GA سے پہلے کوئی اور تبصرہ/فیڈبیک؟**

مختصر جوابات capture کریں اور اگر آپ external survey فارم استعمال کر رہے ہیں تو raw
exports کو بھی اٹیچ کریں۔

## نالج چیک

- Score: `__/10`
- غلط سوالات (اگر ہوں): `[#1, #4, …]`
- Follow-up actions (اگر score < 9/10): کیا remediation کال شیڈول کی گئی ہے؟ ہاں/نہیں

## Sign-off

- Reviewer کا نام اور timestamp:
- Docs/DevRel reviewer کا نام اور timestamp:

دستخط شدہ کاپی کو متعلقہ artefacts کے ساتھ اسٹور کریں تاکہ auditors، ویو کو بغیر
اضافی context کے replay کر سکیں۔

</div>
