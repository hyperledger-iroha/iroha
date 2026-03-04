---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/summary.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-summary
title: W2 فيڈبیک اور اسٹیٹس خلاصہ
sidebar_label: W2 خلاصہ
description: community preview wave (W2) کے لئے live digest۔
---

| آئٹم | تفصیل |
| --- | --- |
| לרי | W2 - מבקרי קהילה |
| دعوتی ونڈو | 2025-06-15 -> 2025-06-29 |
| آرٹیفیکٹ ٹیگ | `preview-2025-06-15` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W2` |
| شرکا | comm-vol-01...comm-vol-08 |

## نمایاں نکات

1. **Governance اور tooling** - community intake پالیسی 2025-05-20 کو متفقہ طور پر منظور ہوئی؛ motivation/timezone فیلڈز کے ساتھ اپ ڈیٹ request template `docs/examples/docs_preview_request_template.md` میں موجود ہے۔
2. **Preflight evidence** - Try it proxy change `OPS-TRYIT-188` 2025-06-09 کو چلایا گیا، Grafana dashboards کیپچر کیے گئے، اور `preview-2025-06-15` کے descriptor/checksum/probe outputs `artifacts/docs_preview/W2/` میں archive کیے گئے۔
3. **Invite wave** - آٹھ community reviewers کو 2025-06-15 کو مدعو کیا گیا، acknowledgements tracker invite table میں لاگ ہوئے؛ سب نے browsing سے پہلے checksum verification مکمل کیا۔
4. **Feedback** - `docs-preview/w2 #1` (tooltip wording) اور `#2` (localization sidebar order) 2025-06-18 کو فائل ہوئے اور 2025-06-21 تک حل ہو گئے (Docs-core-04/05)؛ لہر کے دوران کوئی incidents نہیں ہوئے۔

## ایکشن آئٹمز

| תעודה מזהה | وضاحت | مالک | اسٹیٹس |
| --- | --- | --- | --- |
| W2-A1 | `docs-preview/w2 #1` (tooltip wording) حل کرنا۔ | Docs-core-04 | ✅ מקל (21-06-2025). |
| W2-A2 | `docs-preview/w2 #2` (localization sidebar) حل کرنا۔ | Docs-core-05 | ✅ מקל (21-06-2025). |
| W2-A3 | exit evidence archive کرنا + roadmap/status اپ ڈیٹ کرنا۔ | Docs/DevRel lead | ✅ מחמל (2025-06-29). |

## اختتامی خلاصہ (2025-06-29)

- تمام آٹھ community reviewers نے تکمیل کی تصدیق کی اور preview access واپس لے لیا گیا؛ acknowledgements tracker invite log میں ریکارڈ ہوئے۔
- آخری telemetry snapshots (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) green رہے؛ logs اور Try it proxy transcripts `DOCS-SORA-Preview-W2` کے ساتھ منسلک ہیں۔
- evidence bundle (descriptor, checksum log, probe output, link report, Grafana screenshots, invite acknowledgements) `artifacts/docs_preview/W2/preview-2025-06-15/` میں archive ہوا۔
- מעקב אחר יציאת יומן נקודת ביקורת W2. מפת דרכים W3 תכנון תכנון W3